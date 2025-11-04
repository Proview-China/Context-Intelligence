use std::env;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose};
use futures_util::StreamExt;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::Deserialize;
use sysinfo::{Networks, System};
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout, Instant};
use walkdir::WalkDir;

const DEEPSEEK_ENDPOINT: &str = "https://api.deepseek.com/chat/completions";
const DEFAULT_KEY_FILE: &str = "deepseek_api_key.secret";
const PER_TASK_MEMORY_KB: u64 = 64 * 1024; // 64MB 估算
const PER_TASK_BANDWIDTH_BYTES: u64 = 512 * 1024; // 512KB/s 估算

#[derive(Debug)]
pub struct PretacklerConfig {
    pub input: PathBuf,
    pub version: String,
    pub prompt_path: PathBuf,
    pub model: String,
    pub temperature: f32,
    pub top_k: u32,
    pub concurrency_ceil: Option<usize>,
    pub rate_limit_rps: Option<f64>,
    pub rate_limit_bytes_per_sec: Option<u64>,
    pub connect_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub stream_idle_timeout_secs: u64,
    pub skip_large_file_size_mb: Option<u64>,
    pub skip_exts: Vec<String>,
    pub verbose: bool,
    pub inject_fault: Option<String>,
    pub long_file_bytes_threshold: u64,
    pub long_file_lines_threshold: u64,
    pub long_channel_enabled: bool,
    pub long_channel_timeout_multiplier: f32,
    pub long_channel_request_timeout_secs: Option<u64>,
    pub long_channel_idle_timeout_secs: Option<u64>,
    pub long_channel_adaptive_idle_enabled: bool,
}

#[derive(Debug, Clone)]
struct FileMetadata {
    language: &'static str,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Option<StreamDelta>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

pub const DEFAULT_PROMPT_FILE: &str = "prompt_template.md";
pub const DEFAULT_MODEL: &str = "deepseek-chat";

#[derive(Clone, Debug)]
enum FaultKind { Status429, Status500, Idle }

fn parse_fault(s: Option<&str>) -> Option<FaultKind> {
    match s.map(|v| v.to_ascii_lowercase()) {
        Some(ref v) if v == "429" => Some(FaultKind::Status429),
        Some(ref v) if v == "5xx" || v == "500" => Some(FaultKind::Status500),
        Some(ref v) if v == "idle" => Some(FaultKind::Idle),
        _ => None,
    }
}

pub async fn run(config: PretacklerConfig) -> Result<()> {
    let PretacklerConfig {
        input,
        version,
        prompt_path,
        model,
        temperature,
        top_k,
        concurrency_ceil,
        rate_limit_rps,
        rate_limit_bytes_per_sec,
        connect_timeout_secs,
        request_timeout_secs,
        stream_idle_timeout_secs,
        skip_large_file_size_mb,
        mut skip_exts,
        verbose,
        inject_fault,
        long_file_bytes_threshold,
        long_file_lines_threshold,
        long_channel_enabled,
        long_channel_timeout_multiplier,
        long_channel_request_timeout_secs,
        long_channel_idle_timeout_secs,
        long_channel_adaptive_idle_enabled,
    } = config;

    let api_key = Arc::new(load_api_key().await?);
    let prompt = Arc::new(load_prompt(&prompt_path).await?);
    let client = Arc::new(
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(connect_timeout_secs))
            .timeout(Duration::from_secs(request_timeout_secs))
            .build()
            .context("初始化 HTTP 客户端失败")?,
    );

    // 自适应 idle 统计（仅长通道使用）
    let adapt = if long_channel_adaptive_idle_enabled { Some(Arc::new(LongAdapt::new())) } else { None };

    // 规范化扩展名（小写、去除前导点）
    for ext in &mut skip_exts {
        let e = ext.trim().trim_start_matches('.') .to_ascii_lowercase();
        *ext = e;
    }

    let limiter = if rate_limit_rps.is_some() || rate_limit_bytes_per_sec.is_some() {
        Some(Arc::new(RateLimiter::new(rate_limit_rps, rate_limit_bytes_per_sec)))
    } else {
        None
    };

    if input.is_file() {
        if let Some(reason) = should_skip(&input, skip_large_file_size_mb, &skip_exts).await? {
            println!("{} [skip] {} - {}", ts_now(), input.display(), reason);
            return Ok(());
        }
        let summary_path = build_file_summary_path(&input, &version)?;
        process_file(
            client,
            api_key,
            prompt,
            &model,
            temperature,
            top_k,
            &input,
            &summary_path,
            verbose,
            limiter,
            stream_idle_timeout_secs,
            parse_fault(inject_fault.as_deref()),
            None,
            false,
            adapt.clone(),
        )
        .await?;

        println!("摘要已生成: {}", summary_path.display());
        println!(
            "PreTackler 完成：文件 1 个，目录 0 个，输出位置 {}",
            summary_path.display()
        );
        return Ok(());
    }

    if input.is_dir() {
        let report = process_directory(
            PretacklerRuntime {
                client,
                api_key,
                prompt,
                limiter,
                fault: parse_fault(inject_fault.as_deref()),
                adapt,
            },
            &input,
            &version,
            &model,
            temperature,
            top_k,
            concurrency_ceil,
            skip_large_file_size_mb,
            skip_exts,
            verbose,
            stream_idle_timeout_secs,
            long_channel_enabled,
            long_file_bytes_threshold,
            long_file_lines_threshold,
            long_channel_timeout_multiplier,
            long_channel_request_timeout_secs,
            long_channel_idle_timeout_secs,
            request_timeout_secs,
        )
        .await?;
        println!(
            "PreTackler 完成：文件 {} 个，目录 {} 个，输出根目录 {}",
            report.files_processed,
            report.directories_processed,
            report.output_root.display()
        );
        return Ok(());
    }

    bail!("输入路径不是文件或文件夹: {}", input.display());
}

struct PretacklerRuntime {
    client: Arc<reqwest::Client>,
    api_key: Arc<String>,
    prompt: Arc<String>,
    limiter: Option<Arc<RateLimiter>>,
    fault: Option<FaultKind>,
    adapt: Option<Arc<LongAdapt>>, // P2 自适应 idle 统计
}

#[derive(Debug)]
pub struct ProcessingReport {
    pub output_root: PathBuf,
    pub files_processed: usize,
    pub directories_processed: usize,
}

async fn process_directory(
    runtime: PretacklerRuntime,
    input_dir: &Path,
    version: &str,
    model: &str,
    temperature: f32,
    top_k: u32,
    concurrency_ceil: Option<usize>,
    skip_large_file_size_mb: Option<u64>,
    skip_exts: Vec<String>,
    verbose: bool,
    stream_idle_timeout_secs: u64,
    // 长通道策略参数
    long_channel_enabled: bool,
    long_file_bytes_threshold: u64,
    long_file_lines_threshold: u64,
    long_channel_timeout_multiplier: f32,
    long_channel_request_timeout_secs: Option<u64>,
    long_channel_idle_timeout_secs: Option<u64>,
    request_timeout_secs: u64,
) -> Result<ProcessingReport> {
    let PretacklerRuntime {
        client,
        api_key,
        prompt,
        limiter,
        fault,
        adapt,
    } = runtime;

    let output_root = build_output_root(input_dir, version)?;
    fs::create_dir_all(&output_root)
        .await
        .with_context(|| format!("创建输出根目录失败: {}", output_root.display()))?;

    let (dir_rel_paths, file_entries_all) = collect_directory_entries(input_dir)?;

    for rel_dir in &dir_rel_paths {
        let dir_path = if rel_dir.as_os_str().is_empty() {
            output_root.clone()
        } else {
            output_root.join(rel_dir)
        };
        fs::create_dir_all(&dir_path)
            .await
            .with_context(|| format!("创建输出子目录失败: {}", dir_path.display()))?;
    }

    let mut normal_entries: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut long_entries: Vec<(PathBuf, PathBuf)> = Vec::new();
    let total_found = file_entries_all.len();
    for (abs_path, rel_path) in file_entries_all {
        if let Some(reason) = should_skip(&abs_path, skip_large_file_size_mb, &skip_exts).await? {
            println!("{} [skip] {} - {}", ts_now(), abs_path.display(), reason);
            continue;
        }
        let route_long = if long_channel_enabled {
            match is_long_file_by_thresholds(&abs_path, long_file_bytes_threshold, long_file_lines_threshold).await {
                Ok(b) => b,
                Err(e) => { println!("{} [warn] 阈值判断失败 {}: {}，按 normal", ts_now(), abs_path.display(), e); false }
            }
        } else { false };
        if route_long {
            long_entries.push((abs_path, rel_path));
        } else {
            normal_entries.push((abs_path, rel_path));
        }
    }
    let total_entries = normal_entries.len() + long_entries.len();
    if total_entries == 0 {
        println!("目录不包含可处理文件: {}", output_root.display());
        return Ok(ProcessingReport {
            output_root,
            files_processed: 0,
            directories_processed: 0,
        });
    }

    let mut rng = thread_rng();
    normal_entries.shuffle(&mut rng);
    long_entries.shuffle(&mut rng);

    let concurrency_limit = determine_concurrency_limit(concurrency_ceil, total_entries).await;
    println!(
        "{} 计划处理文件: normal {} / long {} / 总 {}/{}，并发任务数: {}",
        ts_now(), normal_entries.len(), long_entries.len(), total_entries, total_found, concurrency_limit
    );
    // 准备两条队列
    // 队列项：(abs, summary, req_timeout_secs, idle_timeout_secs, is_long)
    let (tx_n, rx_n) = mpsc::channel::<(PathBuf, PathBuf, u64, u64, bool)>(normal_entries.len().max(1));
    let (tx_l, rx_l) = mpsc::channel::<(PathBuf, PathBuf, u64, u64, bool)>(long_entries.len().max(1));
    // normal: 使用基础超时
    for (abs_path, rel_path) in &normal_entries {
        let summary_path = build_file_summary_path_in_output(&output_root, rel_path, version)?;
        tx_n.send((abs_path.clone(), summary_path, request_timeout_secs, stream_idle_timeout_secs, false)).await.context("分派 normal 文件任务失败")?;
    }
    // long: 计算长通道的 request/idle 超时（0 表示无限制 → 以极大值代替 request，idle=0 表示不设置超时）
    let long_req = compute_long_timeout(request_timeout_secs, long_channel_request_timeout_secs, long_channel_timeout_multiplier);
    let long_idle = compute_long_timeout(stream_idle_timeout_secs, long_channel_idle_timeout_secs, long_channel_timeout_multiplier);
    for (abs_path, rel_path) in &long_entries {
        let summary_path = build_file_summary_path_in_output(&output_root, rel_path, version)?;
        tx_l.send((abs_path.clone(), summary_path, long_req, long_idle, true)).await.context("分派 long 文件任务失败")?;
    }
    drop(tx_n);
    drop(tx_l);

    let rx_n = Arc::new(Mutex::new(rx_n));
    let rx_l = Arc::new(Mutex::new(rx_l));
    let mut join_set: JoinSet<Result<usize>> = JoinSet::new();
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;
    let started = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicUsize::new(0));

    // P2 公平调度：统一 worker 池 + 轮询两队列，避免饥饿
    let turn = Arc::new(AtomicUsize::new(0));
    for _ in 0..concurrency_limit {
        let client = client.clone();
        let api_key = api_key.clone();
        let prompt = prompt.clone();
        let model = model.to_string();
        let rx_n = Arc::clone(&rx_n);
        let rx_l = Arc::clone(&rx_l);
        let turn = turn.clone();
        let started = started.clone();
        let completed = completed.clone();
        let total = total_entries;

        let limiter = limiter.clone();
        let fault = fault.clone();
        let adapt = adapt.clone();
        join_set.spawn(async move {
            let mut processed = 0usize;
            loop {
                // 轮询公平获取任务
                let prefer_long = turn.fetch_add(1, Ordering::SeqCst) % 2 == 0;
                let mut job = None;
                // 尝试非阻塞获取
                if prefer_long {
                    if let Some(j) = try_take(&rx_l).await { job = Some(j); }
                    else if let Some(j) = try_take(&rx_n).await { job = Some(j); }
                } else {
                    if let Some(j) = try_take(&rx_n).await { job = Some(j); }
                    else if let Some(j) = try_take(&rx_l).await { job = Some(j); }
                }
                // 都没有则阻塞等待优先队列，再尝试另一个
                if job.is_none() {
                    let first = if prefer_long { &rx_l } else { &rx_n };
                    let second = if prefer_long { &rx_n } else { &rx_l };
                    job = take_blocking(first).await;
                    if job.is_none() { job = take_blocking(second).await; }
                }
                let Some((abs_path, summary_path, req_to, idle_to, is_long)) = job else { break };

                let idx = started.fetch_add(1, Ordering::SeqCst) + 1;
                let file_t0 = Instant::now();
                let ch = if is_long { "LONG" } else { "NORMAL" };
                println!("{} [{} / {}] 开始({} req={}s idle={}s) {}", ts_now(), idx, total, ch, req_to, idle_to, abs_path.display());

                let result = process_file(
                    client.clone(),
                    api_key.clone(),
                    prompt.clone(),
                    model.as_str(),
                    temperature,
                    top_k,
                    &abs_path,
                    &summary_path,
                    verbose,
                    limiter.clone(),
                    idle_to,
                    fault.clone(),
                    Some(req_to),
                    is_long,
                    adapt.clone(),
                )
                .await;

                if let Err(err) = result {
                    println!(
                        "{} [{} / {}] 失败 {} 错误: {}",
                        ts_now(), idx, total, abs_path.display(), err
                    );
                    continue;
                }

                let elapsed = file_t0.elapsed();
                let size_bytes = match fs::metadata(&summary_path).await { Ok(m) => m.len(), Err(_) => 0 };
                let speed = if elapsed.as_secs_f64() > 0.0 { size_bytes as f64 / elapsed.as_secs_f64() } else { 0.0 };
                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                println!("{} [{} / {}] 完成({}) {} 用时 {:.2}s 大小 {:.1}KB 速率 {:.1}KB/s", ts_now(), done, total, ch, summary_path.display(), elapsed.as_secs_f64(), size_bytes as f64 / 1024.0, speed / 1024.0);
                processed += 1;
            }

            Ok(processed)
        });
    }

    let mut files_processed = 0usize;
    while let Some(result) = join_set.join_next().await {
        files_processed += result??;
    }

    println!("{} 全部摘要完成，输出根目录: {}", ts_now(), output_root.display());

    Ok(ProcessingReport {
        output_root,
        files_processed,
        directories_processed: 0,
    })
}

async fn process_file(
    client: Arc<reqwest::Client>,
    api_key: Arc<String>,
    prompt: Arc<String>,
    model: &str,
    temperature: f32,
    top_k: u32,
    input_path: &Path,
    summary_path: &Path,
    verbose: bool,
    limiter: Option<Arc<RateLimiter>>,
    stream_idle_timeout_secs: u64,
    fault: Option<FaultKind>,
    request_timeout_override_secs: Option<u64>,
    is_long: bool,
    adapt: Option<Arc<LongAdapt>>,
) -> Result<()> {
    let input_bytes = fs::read(input_path)
        .await
        .with_context(|| format!("读取输入文件失败: {}", input_path.display()))?;

    let file_name = input_path
        .file_name()
        .and_then(|os| os.to_str())
        .unwrap_or("unknown");

    let metadata = detect_file_metadata(input_path);

    let user_message = if input_bytes.is_empty() {
        format!(
            "文件 `{}` 当前字节长度为 0。\n文件所使用的语言: {}\n请严格按照空文件输出规范：\n文件名: {}\n文件所使用的语言: {}\n文件存在的意义: 文件为空,初始化不能读取其意义。",
            file_name, metadata.language, file_name, metadata.language
        )
    } else {
        let base64_payload = general_purpose::STANDARD.encode(&input_bytes);
        format!(
            "文件 `{}` 已按 Base64 编码传输。\n文件所使用的语言: {}\n以下为编码后的字节流：\n\n{}",
            file_name, metadata.language, base64_payload
        )
    };

    process_streaming_request(
        client,
        api_key,
        prompt,
        model,
        temperature,
        top_k,
        &user_message,
        summary_path,
        verbose,
        limiter,
        stream_idle_timeout_secs,
        fault,
        request_timeout_override_secs,
        is_long,
        adapt,
    )
    .await
}

async fn process_streaming_request(
    client: Arc<reqwest::Client>,
    api_key: Arc<String>,
    prompt: Arc<String>,
    model: &str,
    temperature: f32,
    top_k: u32,
    user_message: &str,
    summary_path: &Path,
    verbose: bool,
    limiter: Option<Arc<RateLimiter>>,
    stream_idle_timeout_secs: u64,
    fault: Option<FaultKind>,
    request_timeout_override_secs: Option<u64>,
    is_long: bool,
    adapt: Option<Arc<LongAdapt>>,
) -> Result<()> {
    const MAX_ATTEMPTS: usize = 5;
    const BACKOFF_BASE_MS: u64 = 500;
    const BACKOFF_FACTOR: f64 = 2.0;
    const BACKOFF_MAX_MS: u64 = 30_000;

    for attempt in 1..=MAX_ATTEMPTS {
        if verbose {
            println!("{} 尝试#{} 请求 {}", ts_now(), attempt, summary_path.display());
        }

        if let Some(l) = &limiter {
            l.acquire_request().await;
        }

        if let Some(parent) = summary_path.parent() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("创建摘要目录失败: {}", parent.display()))?;
        }

        let (mut tmp_guard, mut writer) = open_temp_writer(summary_path).await?;

        let request_body = serde_json::json!({
            "model": model,
            "stream": true,
            "temperature": temperature,
            "top_k": top_k,
            "messages": [
                {"role": "system", "content": &*prompt},
                {"role": "user", "content": user_message}
            ]
        });

        // 故障注入：状态码类
        if let Some(FaultKind::Status429) | Some(FaultKind::Status500) = fault {
            let code = if matches!(fault, Some(FaultKind::Status429)) { 429 } else { 500 };
            if is_retryable_status(code) && attempt < MAX_ATTEMPTS {
                let delay_ms = backoff_delay_ms(attempt, BACKOFF_BASE_MS, BACKOFF_FACTOR, BACKOFF_MAX_MS);
                if verbose { println!("{} [注入] 状态 {} 可重试，退避 {}ms", ts_now(), code, delay_ms); }
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            } else {
                bail!("[注入] 状态 {} 失败", code);
            }
        }

        let mut rb = client
            .post(DEEPSEEK_ENDPOINT)
            .bearer_auth(&*api_key)
            .header("Content-Type", "application/json")
            .json(&request_body);

        if let Some(req_secs) = request_timeout_override_secs {
            // 0 视为“不限时”，以极大超时值代替（24 小时）
            let secs = if req_secs == 0 { 86_400 * 24 } else { req_secs };
            rb = rb.timeout(Duration::from_secs(secs));
        }

        let send_res = rb.send().await;

        let response = match send_res {
            Ok(resp) => resp,
            Err(err) => {
                if should_retry_error(&err) && attempt < MAX_ATTEMPTS {
                    let delay_ms = backoff_delay_ms(attempt, BACKOFF_BASE_MS, BACKOFF_FACTOR, BACKOFF_MAX_MS);
                    if verbose { println!("{} 发送失败（可重试）: {}，退避 {}ms", ts_now(), err, delay_ms); }
                    sleep(Duration::from_millis(delay_ms)).await;
                    continue;
                } else {
                    return Err(err).context("调用 DeepSeek 接口失败");
                }
            }
        };

        if verbose { println!("{} HTTP 状态: {}", ts_now(), response.status()); }
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "<无法读取错误响应>".to_string());

            if is_retryable_status(status.as_u16()) && attempt < MAX_ATTEMPTS {
                let delay_ms = backoff_delay_ms(attempt, BACKOFF_BASE_MS, BACKOFF_FACTOR, BACKOFF_MAX_MS);
                if verbose { println!("{} 状态 {} 可重试，退避 {}ms", ts_now(), status, delay_ms); }
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            bail!("DeepSeek 返回错误: {}，响应: {}", status, body);
        }

        // 故障注入：idle 超时
        if matches!(fault, Some(FaultKind::Idle)) {
            if attempt < MAX_ATTEMPTS {
                if verbose { println!("{} [注入] 触发 idle 超时", ts_now()); }
                let delay_ms = backoff_delay_ms(attempt, BACKOFF_BASE_MS, BACKOFF_FACTOR, BACKOFF_MAX_MS);
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            } else {
                bail!("[注入] idle 超时");
            }
        }

        let mut stream = response.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();
        // P2: 长通道自适应 idle（使用历史 p95）
        let mut effective_idle_secs = stream_idle_timeout_secs;
        if is_long {
            if let Some(ad) = &adapt {
                if stream_idle_timeout_secs > 0 {
                    if let Some(p95_ms) = ad.p95_ms().await { let extra = ((p95_ms as f64) * 1.2 / 1000.0).ceil() as u64; effective_idle_secs = effective_idle_secs.max(extra); }
                }
            }
        }
        let idle_dur = if effective_idle_secs == 0 { None } else { Some(Duration::from_secs(effective_idle_secs)) };
        let mut finished = false;
        let mut last_instant = Instant::now();

        loop {
            let next_chunk = if let Some(d) = idle_dur { timeout(d, stream.next()).await } else { Ok(stream.next().await) };
            match next_chunk {
                Err(_) => {
                    if verbose { println!("{} 触发流式 idle 超时", ts_now()); }
                    // 重试
                    if attempt < MAX_ATTEMPTS {
                        let delay_ms = backoff_delay_ms(attempt, BACKOFF_BASE_MS, BACKOFF_FACTOR, BACKOFF_MAX_MS);
                        sleep(Duration::from_millis(delay_ms)).await;
                        break; // 跳出到外层重试
                    } else {
                        bail!("流式 idle 超时");
                    }
                }
                Ok(None) => {
                    // 流结束
                    break;
                }
                Ok(Some(Err(e))) => {
                    if should_retry_error(&e) && attempt < MAX_ATTEMPTS {
                        let delay_ms = backoff_delay_ms(attempt, BACKOFF_BASE_MS, BACKOFF_FACTOR, BACKOFF_MAX_MS);
                        if verbose { println!("{} 流式读取失败（可重试）: {}，退避 {}ms", ts_now(), e, delay_ms); }
                        sleep(Duration::from_millis(delay_ms)).await;
                        break;
                    } else {
                        return Err(e).context("读取 DeepSeek 流式响应失败");
                    }
                }
                Ok(Some(Ok(chunk))) => {
                    if let Some(l) = &limiter {
                        l.acquire_bytes(chunk.len() as u64).await;
                    }
                    buffer.extend_from_slice(&chunk);
                    if is_long { if let Some(ad) = &adapt { let now = Instant::now(); let dt = now.duration_since(last_instant); last_instant = now; let _ = ad.observe(dt); } }
                    while let Some(position) = buffer.iter().position(|&b| b == b'\n') {
                        let line_bytes: Vec<u8> = buffer.drain(..=position).collect();
                        if process_line(line_bytes, &mut writer).await? {
                            finished = true;
                            break;
                        }
                    }
                    if finished { break; }
                }
            }
        }

        if !finished && !buffer.is_empty() {
            let line_bytes = buffer.drain(..).collect();
            process_line(line_bytes, &mut writer).await?;
        }

        writer.flush().await.context("写入摘要文件失败")?;
        tmp_guard
            .commit()
            .await
            .with_context(|| format!("重命名摘要文件失败: {}", summary_path.display()))?;

        return Ok(());
    }

    unreachable!("重试循环应已返回或报错");
}

fn build_file_summary_path(input: &Path, version: &str) -> Result<PathBuf> {
    let mut summary_path = input.to_path_buf();
    let file_name = input
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| anyhow::anyhow!("无法解析文件名: {}", input.display()))?;
    summary_path.set_file_name(format!("{}.summary.{}.md", file_name, version));
    Ok(summary_path)
}

fn build_file_summary_path_in_output(
    output_root: &Path,
    relative_path: &Path,
    version: &str,
) -> Result<PathBuf> {
    let file_name = relative_path
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| anyhow::anyhow!("无法解析文件名: {}", relative_path.display()))?;

    let mut summary_rel = relative_path.to_path_buf();
    summary_rel.set_file_name(format!("{}.summary.{}.md", file_name, version));

    let summary_path = output_root.join(summary_rel);
    Ok(summary_path)
}

fn build_output_root(input_dir: &Path, version: &str) -> Result<PathBuf> {
    let dir_name = input_dir
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| anyhow::anyhow!("无法解析目录名: {}", input_dir.display()))?;
    let parent = input_dir.parent().unwrap_or_else(|| Path::new("."));
    let output_name = format!("{}.summaries.{}", dir_name, version);
    Ok(parent.join(output_name))
}

fn detect_file_metadata(path: &Path) -> FileMetadata {
    let ext = path
        .extension()
        .and_then(|os| os.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let language = match ext.as_str() {
        "md" | "markdown" => "Markdown",
        "txt" => "纯文本",
        "rs" => "Rust",
        "py" => "Python",
        "js" => "JavaScript",
        "ts" => "TypeScript",
        "tsx" => "TypeScript/TSX",
        "jsx" => "JavaScript/JSX",
        "go" => "Go",
        "java" => "Java",
        "c" => "C",
        "cpp" | "cxx" | "cc" | "hpp" | "hxx" => "C++",
        "h" => "C/C++ 头文件",
        "cs" => "C#",
        "swift" => "Swift",
        "kt" | "kts" => "Kotlin",
        "php" => "PHP",
        "rb" => "Ruby",
        "scala" => "Scala",
        "lua" => "Lua",
        "sh" | "bash" => "Shell",
        "ps1" => "PowerShell",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "scss" | "sass" => "SCSS/SASS",
        "less" => "LESS",
        "json" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "ini" => "INI",
        "env" => "环境变量",
        "lock" => "锁定文件",
        "xml" => "XML",
        "sql" => "SQL",
        "csv" => "CSV",
        "tsv" => "TSV",
        "bin" => "二进制",
        "wasm" => "WebAssembly",
        "exe" => "可执行文件",
        "dll" => "动态链接库",
        _ => {
            let inferred = mime_guess::from_path(path)
                .first_raw()
                .unwrap_or("未知语言");
            match inferred {
                "application/json" => "JSON",
                "text/plain" => "纯文本",
                "text/markdown" => "Markdown",
                "text/css" => "CSS",
                "text/html" => "HTML",
                _ => "未知语言",
            }
        }
    };

    FileMetadata { language }
}

async fn is_long_file_by_thresholds(path: &Path, bytes_threshold: u64, lines_threshold: u64) -> Result<bool> {
    let meta = fs::metadata(path).await?;
    if meta.len() >= bytes_threshold { return Ok(true); }
    if lines_threshold == 0 { return Ok(false); }
    // 小于字节阈值仍可因行数命中：计数换行符
    let data = fs::read(path).await?;
    let lines = bytecount::count(&data, b'\n') as u64 + 1;
    Ok(lines >= lines_threshold)
}

fn compute_long_timeout(base_secs: u64, override_secs: Option<u64>, multiplier: f32) -> u64 {
    if let Some(v) = override_secs { return v; }
    let mul = if multiplier <= 0.0 { 1.0 } else { multiplier } as f64;
    let v = (base_secs as f64 * mul).round() as u64;
    v.max(base_secs)
}

type Job = (PathBuf, PathBuf, u64, u64, bool);

async fn try_take(rx: &Arc<Mutex<mpsc::Receiver<Job>>>) -> Option<Job> {
    let mut guard = rx.lock().await;
    match guard.try_recv() {
        Ok(j) => Some(j),
        Err(_) => None,
    }
}

async fn take_blocking(rx: &Arc<Mutex<mpsc::Receiver<Job>>>) -> Option<Job> {
    let mut guard = rx.lock().await;
    guard.recv().await
}

fn collect_directory_entries(input_dir: &Path) -> Result<(Vec<PathBuf>, Vec<(PathBuf, PathBuf)>)> {
    let mut dir_rel_paths = Vec::new();
    dir_rel_paths.push(PathBuf::new());

    let mut file_entries = Vec::new();

    for entry in WalkDir::new(input_dir).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if path == input_dir {
            continue;
        }

        let relative = path
            .strip_prefix(input_dir)
            .with_context(|| format!("计算相对路径失败: {}", path.display()))?
            .to_path_buf();

        if entry.file_type().is_dir() {
            dir_rel_paths.push(relative);
        } else if entry.file_type().is_file() {
            file_entries.push((path.to_path_buf(), relative));
        }
    }

    Ok((dir_rel_paths, file_entries))
}

async fn determine_concurrency_limit(max_override: Option<usize>, total_files: usize) -> usize {
    let total_files = total_files.max(1);

    if let Some(limit) = max_override {
        return limit.clamp(1, total_files);
    }

    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.refresh_cpu();

    let cpu_cores = sys.cpus().len().max(1);
    let cpu_limit = ((cpu_cores as f32) * 0.85).ceil() as usize;

    let available_memory = sys.available_memory().max(PER_TASK_MEMORY_KB);
    let memory_limit = ((available_memory as f32 / PER_TASK_MEMORY_KB as f32) * 0.85)
        .floor()
        .max(1.0) as usize;

    let mut networks = Networks::new_with_refreshed_list();
    let initial_bytes = total_network_bytes(&networks);
    sleep(Duration::from_millis(500)).await;
    networks.refresh();
    let later_bytes = total_network_bytes(&networks);
    let delta_bytes = later_bytes.saturating_sub(initial_bytes);

    let bandwidth_bytes_per_sec = (delta_bytes as f64) * 2.0; // 0.5s 采样
    let network_limit = if bandwidth_bytes_per_sec < (PER_TASK_BANDWIDTH_BYTES as f64) {
        cpu_limit.max(memory_limit)
    } else {
        ((bandwidth_bytes_per_sec / PER_TASK_BANDWIDTH_BYTES as f64) * 0.85)
            .ceil()
            .max(1.0) as usize
    };

    cpu_limit
        .min(memory_limit)
        .min(network_limit)
        .max(1)
        .min(total_files)
}

fn total_network_bytes(networks: &Networks) -> u64 {
    networks.iter().fold(0u64, |acc, (_name, data)| {
        acc + data.total_received() + data.total_transmitted()
    })
}

async fn load_prompt(path: &Path) -> Result<String> {
    let prompt = fs::read_to_string(path)
        .await
        .with_context(|| format!("读取提示词文件失败: {}", path.display()))?;
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        bail!("提示词文件内容为空");
    }
    Ok(prompt)
}

async fn load_api_key() -> Result<String> {
    if let Ok(path) = env::var("DEEPSEEK_API_KEY_FILE") {
        let explicit_path = PathBuf::from(path);
        match read_key_from_path(&explicit_path).await? {
            Some(key) => return Ok(key),
            None => bail!("指定的密钥文件不存在: {}", explicit_path.display()),
        }
    }

    // 优先当前工作目录
    if let Some(key) = read_key_from_path(Path::new(DEFAULT_KEY_FILE)).await? {
        return Ok(key);
    }

    // 其次 Cargo manifest 目录
    let manifest_default = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_KEY_FILE);
    if let Some(key) = read_key_from_path(&manifest_default).await? {
        return Ok(key);
    }

    if let Ok(key) = env::var("DEEPSEEK_API_KEY") {
        let key = key.trim().to_string();
        if key.is_empty() {
            bail!("环境变量 DEEPSEEK_API_KEY 为空");
        }
        return Ok(key);
    }

    bail!(
        "未找到可用的 DeepSeek 密钥。请在项目根目录放置 `{}`，或通过环境变量提供。",
        DEFAULT_KEY_FILE
    );
}

async fn read_key_from_path(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path).await {
        Ok(content) => {
            let trimmed = content.trim().to_string();
            if trimmed.is_empty() {
                bail!("密钥文件内容为空: {}", path.display());
            }
            Ok(Some(trimmed))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("读取密钥文件失败: {}", path.display())),
    }
}

async fn process_line(line_bytes: Vec<u8>, writer: &mut BufWriter<fs::File>) -> Result<bool> {
    let line = String::from_utf8_lossy(&line_bytes);
    let trimmed = line.trim();

    if trimmed.is_empty() || !trimmed.starts_with("data:") {
        return Ok(false);
    }

    let payload = trimmed["data:".len()..].trim();
    if payload == "[DONE]" {
        return Ok(true);
    }

    let parsed: StreamResponse = match serde_json::from_str(payload) {
        Ok(resp) => resp,
        Err(err) => {
            eprintln!("解析流式响应失败: {}", err);
            return Ok(false);
        }
    };

    for choice in parsed.choices {
        if let Some(delta) = choice.delta {
            if let Some(content) = delta.content {
                writer
                    .write_all(content.as_bytes())
                    .await
                    .context("写入摘要内容失败")?;
                writer.flush().await.context("刷新摘要内容失败")?;
            }
        }
    }

    Ok(false)
}

struct TempWriterGuard {
    tmp_path: PathBuf,
    final_path: PathBuf,
    committed: bool,
}

impl Drop for TempWriterGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.tmp_path);
        }
    }
}

impl TempWriterGuard {
    async fn commit(&mut self) -> Result<()> {
        fs::rename(&self.tmp_path, &self.final_path).await?;
        self.committed = true;
        Ok(())
    }
}

async fn open_temp_writer(summary_path: &Path) -> Result<(TempWriterGuard, BufWriter<fs::File>)> {
    let parent = summary_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无法获取摘要文件父目录: {}", summary_path.display()))?;
    fs::create_dir_all(parent)
        .await
        .with_context(|| format!("创建摘要目录失败: {}", parent.display()))?;

    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    let pid = std::process::id();
    let suffix = format!("{:x}{:x}", pid, nanos);

    let tmp_name = format!(
        "{}.tmp-{}",
        summary_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("summary.md"),
        suffix
    );
    let tmp_path = parent.join(tmp_name);
    let file = fs::File::create(&tmp_path)
        .await
        .with_context(|| format!("创建临时摘要文件失败: {}", tmp_path.display()))?;
    Ok((
        TempWriterGuard { tmp_path, final_path: summary_path.to_path_buf(), committed: false },
        BufWriter::new(file),
    ))
}

async fn should_skip(path: &Path, max_size_mb: Option<u64>, skip_exts: &Vec<String>) -> Result<Option<String>> {
    // 扩展名判断
    if !skip_exts.is_empty() {
        let ext = path
            .extension()
            .and_then(|os| os.to_str())
            .map(|s| s.trim_start_matches('.').to_ascii_lowercase());
        if let Some(ext) = ext {
            if skip_exts.iter().any(|e| e == &ext) {
                return Ok(Some(format!("扩展名匹配跳过: .{}", ext)));
            }
        }
    }

    // 大小判断
    if let Some(mb) = max_size_mb {
        let meta = match fs::metadata(path).await { Ok(m) => m, Err(_) => return Ok(None) };
        let size = meta.len();
        let limit = mb.saturating_mul(1024 * 1024);
        if size > limit {
            return Ok(Some(format!("文件大小 {:.2}MB 超过阈值 {}MB", size as f64 / (1024.0*1024.0), mb)));
        }
    }

    Ok(None)
}

fn ts_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    // 打印为 mm:ss 格式，简单直观
    let secs = now.as_secs();
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}

// ------ 限速与重试工具 ------

#[derive(Debug)]
struct RateLimiterInner {
    rps: Option<f64>,
    bytes_per_sec: Option<u64>,
    last_request: Option<Instant>,
    epoch_start: Instant,
    bytes_in_epoch: u64,
}

#[derive(Clone, Debug)]
struct RateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
}

impl RateLimiter {
    fn new(rps: Option<f64>, bytes_per_sec: Option<u64>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                rps,
                bytes_per_sec,
                last_request: None,
                epoch_start: Instant::now(),
                bytes_in_epoch: 0,
            })),
        }
    }

    async fn acquire_request(&self) {
        let min_interval = {
            let inner = self.inner.lock().await;
            inner.rps.map(|rps| Duration::from_secs_f64((1.0 / rps).max(0.0)))
        };
        if let Some(min_interval) = min_interval {
            loop {
                let mut inner = self.inner.lock().await;
                let now = Instant::now();
                let due = match inner.last_request {
                    Some(last) => last + min_interval,
                    None => now,
                };
                if now >= due {
                    inner.last_request = Some(now);
                    break;
                } else {
                    drop(inner);
                    sleep(due - now).await;
                }
            }
        }
    }

    async fn acquire_bytes(&self, need: u64) {
        let limit = {
            let inner = self.inner.lock().await;
            inner.bytes_per_sec
        };
        if let Some(limit) = limit {
            loop {
                let mut inner = self.inner.lock().await;
                let now = Instant::now();
                if now.duration_since(inner.epoch_start) >= Duration::from_secs(1) {
                    inner.epoch_start = now;
                    inner.bytes_in_epoch = 0;
                }
                if inner.bytes_in_epoch + need <= limit {
                    inner.bytes_in_epoch += need;
                    break;
                } else {
                    let wait = Duration::from_secs(1) - now.duration_since(inner.epoch_start);
                    drop(inner);
                    sleep(wait).await;
                }
            }
        }
    }
}

fn is_retryable_status(code: u16) -> bool {
    code == 429 || (500..600).contains(&code)
}

fn backoff_delay_ms(attempt: usize, base_ms: u64, factor: f64, max_ms: u64) -> u64 {
    let pow = factor.powi((attempt.saturating_sub(1)) as i32);
    let v = (base_ms as f64 * pow).round() as u64;
    v.min(max_ms)
}

fn should_retry_error(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() || err.is_request() {
        return true;
    }
    false
}

// ------ 长通道自适应 idle 统计 ------

#[derive(Debug)]
struct LongAdaptInner {
    samples_ms: std::collections::VecDeque<u64>,
    cap: usize,
}

#[derive(Clone, Debug)]
struct LongAdapt {
    inner: Arc<Mutex<LongAdaptInner>>,
}

impl LongAdapt {
    fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(LongAdaptInner { samples_ms: std::collections::VecDeque::with_capacity(256), cap: 256 })) }
    }

    async fn observe(&self, dt: Duration) -> Result<()> {
        let ms = dt.as_millis() as u64;
        let mut inner = self.inner.lock().await;
        if inner.samples_ms.len() >= inner.cap { inner.samples_ms.pop_front(); }
        inner.samples_ms.push_back(ms);
        Ok(())
    }

    async fn p95_ms(&self) -> Option<u64> {
        let inner = self.inner.lock().await;
        if inner.samples_ms.is_empty() { return None; }
        let mut v: Vec<u64> = inner.samples_ms.iter().copied().collect();
        v.sort_unstable();
        let idx = ((v.len() as f64) * 0.95).ceil() as usize - 1;
        v.get(idx).copied()
    }
}
