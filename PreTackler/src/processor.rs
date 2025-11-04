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
use tokio::time::sleep;
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
    pub max_concurrency: Option<usize>,
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

pub async fn run(config: PretacklerConfig) -> Result<()> {
    let PretacklerConfig {
        input,
        version,
        prompt_path,
        model,
        temperature,
        top_k,
        max_concurrency,
    } = config;

    let api_key = Arc::new(load_api_key().await?);
    let prompt = Arc::new(load_prompt(&prompt_path).await?);
    let client = Arc::new(reqwest::Client::new());

    if input.is_file() {
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
            },
            &input,
            &version,
            &model,
            temperature,
            top_k,
            max_concurrency,
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
    max_concurrency: Option<usize>,
) -> Result<ProcessingReport> {
    let PretacklerRuntime {
        client,
        api_key,
        prompt,
    } = runtime;

    let output_root = build_output_root(input_dir, version)?;
    fs::create_dir_all(&output_root)
        .await
        .with_context(|| format!("创建输出根目录失败: {}", output_root.display()))?;

    let (dir_rel_paths, file_entries) = collect_directory_entries(input_dir)?;

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

    let mut file_entries = file_entries;
    if file_entries.is_empty() {
        println!("目录不包含可处理文件: {}", output_root.display());
        return Ok(ProcessingReport {
            output_root,
            files_processed: 0,
            directories_processed: 0,
        });
    }

    let mut rng = thread_rng();
    file_entries.shuffle(&mut rng);

    let concurrency_limit = determine_concurrency_limit(max_concurrency, file_entries.len()).await;
    println!("并发任务数: {}", concurrency_limit);

    let queue_capacity = file_entries.len().max(concurrency_limit * 2);
    let (tx, rx) = mpsc::channel::<(PathBuf, PathBuf)>(queue_capacity);

    for (abs_path, rel_path) in file_entries {
        let summary_path = build_file_summary_path_in_output(&output_root, &rel_path, version)?;
        tx.send((abs_path, summary_path))
            .await
            .context("分派文件任务失败")?;
    }
    drop(tx);

    let rx = Arc::new(Mutex::new(rx));
    let mut join_set: JoinSet<Result<usize>> = JoinSet::new();

    for _ in 0..concurrency_limit {
        let client = client.clone();
        let api_key = api_key.clone();
        let prompt = prompt.clone();
        let model = model.to_string();
        let rx = Arc::clone(&rx);

        join_set.spawn(async move {
            let mut processed = 0usize;
            loop {
                let next_job = {
                    let mut guard = rx.lock().await;
                    guard.recv().await
                };

                let Some((abs_path, summary_path)) = next_job else {
                    break;
                };

                process_file(
                    client.clone(),
                    api_key.clone(),
                    prompt.clone(),
                    model.as_str(),
                    temperature,
                    top_k,
                    &abs_path,
                    &summary_path,
                )
                .await?;

                println!("摘要已生成: {}", summary_path.display());
                processed += 1;
            }

            Ok(processed)
        });
    }

    let mut files_processed = 0usize;
    while let Some(result) = join_set.join_next().await {
        files_processed += result??;
    }

    println!("全部摘要完成，输出根目录: {}", output_root.display());

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
) -> Result<()> {
    if let Some(parent) = summary_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("创建摘要目录失败: {}", parent.display()))?;
    }

    let file = fs::File::create(summary_path)
        .await
        .with_context(|| format!("创建摘要文件失败: {}", summary_path.display()))?;
    let mut writer = BufWriter::new(file);

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

    let response = client
        .post(DEEPSEEK_ENDPOINT)
        .bearer_auth(&*api_key)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .context("调用 DeepSeek 接口失败")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<无法读取错误响应>".to_string());
        bail!(
            "DeepSeek 接口返回错误状态码: {}，响应内容: {}",
            status,
            body
        );
    }

    let mut stream = response.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    let mut finished = false;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取 DeepSeek 流式响应失败")?;
        buffer.extend_from_slice(&chunk);

        while let Some(position) = buffer.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = buffer.drain(..=position).collect();
            if process_line(line_bytes, &mut writer).await? {
                finished = true;
                break;
            }
        }

        if finished {
            break;
        }
    }

    if !finished && !buffer.is_empty() {
        let line_bytes = buffer.drain(..).collect();
        process_line(line_bytes, &mut writer).await?;
    }

    writer.flush().await.context("写入摘要文件失败")?;

    Ok(())
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

    let manifest_default = Path::new(env!("CARGO_MANIFEST_DIR")).join(DEFAULT_KEY_FILE);
    if let Some(key) = read_key_from_path(&manifest_default).await? {
        return Ok(key);
    }

    if let Some(key) = read_key_from_path(Path::new(DEFAULT_KEY_FILE)).await? {
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
