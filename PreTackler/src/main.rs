use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

mod processor;
use processor::{PretacklerConfig, run};

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
    #[arg(long, default_value = processor::DEFAULT_PROMPT_FILE)]
    prompt: PathBuf,

    /// 调用的 DeepSeek 模型
    #[arg(long, default_value = processor::DEFAULT_MODEL)]
    model: String,

    /// 采样温度
    #[arg(long, default_value_t = 0.65)]
    temperature: f32,

    /// Top-K 采样参数
    #[arg(long, default_value_t = 1)]
    top_k: u32,

    /// 最大并发请求数（可选），不设置则自动估算
    #[arg(long)]
    max_concurrency: Option<usize>,
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
        max_concurrency: args.max_concurrency,
    };

    run(config).await
}
