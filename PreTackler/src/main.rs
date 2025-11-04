use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod processor;
use processor::{PretacklerConfig, run, DEFAULT_MODEL, DEFAULT_PROMPT_FILE};

#[derive(Parser, Debug)]
#[command(name = "pretackler")]
#[command(about = "PreTackler：调用 DeepSeek 生成上下文总结", long_about = None)]
struct Args {
    /// 需要传输给 DeepSeek 的原始文件或文件夹路径
    input: PathBuf,

    /// 版本号，将拼接在输出文件名中
    #[arg(long, default_value = "v1")]
    version: String,

    /// 提示词模板文件路径
    #[arg(long, default_value = DEFAULT_PROMPT_FILE, help = "提示词模板文件路径（默认：./prompt_template.md）")]
    prompt: PathBuf,

    /// 调用的 DeepSeek 模型
    #[arg(long, default_value = DEFAULT_MODEL, help = "调用的模型名称（默认：deepseek-chat）")]
    model: String,

    /// 采样温度
    #[arg(long, default_value_t = 0.65, help = "采样温度（默认：0.65）")]
    temperature: f32,

    /// Top-K 采样参数
    #[arg(long, default_value_t = 1, help = "Top-K 采样参数（默认：1）")]
    top_k: u32,

    /// 并发上限，不设置则根据系统资源自适应估算（支持别名：--max-concurrency）
    #[arg(long = "concurrency-ceil", visible_alias = "max-concurrency", help = "并发上限（可选），未设置时自适应估算并裁剪到安全范围")]
    concurrency_ceil: Option<usize>,

    /// （可选）请求速率限速：每秒请求数上限（RPS）
    #[arg(long = "rate-limit-rps", help = "令牌桶限速：每秒请求数上限（RPS），默认关闭")]
    rate_limit_rps: Option<f64>,

    /// （可选）字节级限速：每秒发送字节上限（估算值）
    #[arg(long = "rate-limit-bytes-per-sec", help = "令牌桶限速：每秒发送字节上限（估算），默认关闭")]
    rate_limit_bytes_per_sec: Option<u64>,

    /// 连接超时（秒）
    #[arg(long = "connect-timeout", default_value_t = 15u64, help = "连接超时（秒），默认15s")]
    connect_timeout_secs: u64,

    /// 整体请求超时（秒）
    #[arg(long = "request-timeout", default_value_t = 45u64, help = "整体请求超时（秒），默认45s")]
    request_timeout_secs: u64,

    /// 流式空闲超时（秒），该时间内未收到新chunk则判定失败并重试
    #[arg(long = "stream-idle-timeout", default_value_t = 30u64, help = "流式空闲超时（秒），默认30s")]
    stream_idle_timeout_secs: u64,

    /// 超过指定大小（MB）的文件跳过
    #[arg(long = "skip-large-file-size-mb", help = "超过该大小（MB）的文件将被跳过")]
    skip_large_file_size_mb: Option<u64>,

    /// 按扩展名跳过，逗号分隔（不区分大小写），例如：--skip-ext ".png,.jpg"
    #[arg(long = "skip-ext", value_delimiter = ',', help = "按扩展名跳过（逗号分隔，不区分大小写）")]
    skip_exts: Vec<String>,

    /// 详细日志
    #[arg(long, default_value_t = false, help = "开启更详细日志（等待/退避/HTTP状态/idle超时触发）")]
    verbose: bool,

    /// 测试用故障注入：429|5xx|idle（仅用于本地验收测试）
    #[arg(long = "inject-fault", help = "测试用故障注入：429|5xx|idle（仅本地验收）")]
    inject_fault: Option<String>,

    /// 长/大文件字节阈值（默认 512KB）
    #[arg(long = "long-file-bytes-threshold", default_value_t = 524_288u64, help = "长/大文件字节阈值（默认 512KB）")]
    long_file_bytes_threshold: u64,

    /// 长/大文件行数阈值（默认 4000 行）
    #[arg(long = "long-file-lines-threshold", default_value_t = 4000u64, help = "长/大文件行数阈值（默认 4000）")]
    long_file_lines_threshold: u64,

    /// 启用长时通道（默认 启用）
    #[arg(long = "long-channel-enabled", default_value_t = true, help = "启用长时通道（默认 启用）")]
    long_channel_enabled: bool,

    /// 长通道超时放大倍数（默认 5.0）
    #[arg(long = "long-channel-timeout-multiplier", default_value_t = 5.0f32, help = "长通道超时放大倍数（默认 5.0）")]
    long_channel_timeout_multiplier: f32,

    /// 长通道请求超时（秒，0 表示不限时；未设置则按倍数计算）
    #[arg(long = "long-channel-request-timeout", help = "长通道请求超时（秒，0 不限时；未设置按倍数计算）")]
    long_channel_request_timeout_secs: Option<u64>,

    /// 长通道流式 idle 超时（秒，0 表示不限时；未设置则按倍数计算）
    #[arg(long = "long-channel-idle-timeout", help = "长通道流式 idle 超时（秒，0 不限时；未设置按倍数计算）")]
    long_channel_idle_timeout_secs: Option<u64>,

    /// 启用长通道自适应 idle 超时（基于历史流间隔 p95；默认 启用）
    #[arg(long = "long-channel-adaptive-idle-enabled", default_value_t = true, help = "长通道自适应 idle 超时（默认 启用）")]
    long_channel_adaptive_idle_enabled: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let config = PretacklerConfig {
        input: args.input,
        version: args.version,
        prompt_path: args.prompt,
        model: args.model,
        temperature: args.temperature,
        top_k: args.top_k,
        concurrency_ceil: args.concurrency_ceil,
        rate_limit_rps: args.rate_limit_rps,
        rate_limit_bytes_per_sec: args.rate_limit_bytes_per_sec,
        connect_timeout_secs: args.connect_timeout_secs,
        request_timeout_secs: args.request_timeout_secs,
        stream_idle_timeout_secs: args.stream_idle_timeout_secs,
        skip_large_file_size_mb: args.skip_large_file_size_mb,
        skip_exts: args.skip_exts,
        verbose: args.verbose,
        inject_fault: args.inject_fault,
        long_file_bytes_threshold: args.long_file_bytes_threshold,
        long_file_lines_threshold: args.long_file_lines_threshold,
        long_channel_enabled: args.long_channel_enabled,
        long_channel_timeout_multiplier: args.long_channel_timeout_multiplier,
        long_channel_request_timeout_secs: args.long_channel_request_timeout_secs,
        long_channel_idle_timeout_secs: args.long_channel_idle_timeout_secs,
        long_channel_adaptive_idle_enabled: args.long_channel_adaptive_idle_enabled,
    };

    run(config).await
}
