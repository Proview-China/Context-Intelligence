% 项目说明（OpenSpec Project Overview）

## 项目概述
- 名称：Context Intelligence（上下文智能）
- 目标：为大语言模型（LLM）生成高质量、可控的“上下文包”（Context Package），用于代码审阅、变更提案（OpenSpec）与知识对齐，降低上下文噪声与成本。
- 当前可用组件：
  - `PreTackler`（Rust CLI）：扫描文件或目录，基于提示词调用 DeepSeek 接口生成结构化摘要，按并发与流式方式输出结果。

## 技术栈与工具
- 语言与运行时：
  - Rust 1.80+（2024 edition）
- 主要依赖（`PreTackler/Cargo.toml`）：
  - `clap`（命令行解析）、`tokio`（异步运行时）、`reqwest`（HTTP 客户端，`rustls-tls`）
  - `serde/serde_json`（序列化）、`anyhow`（错误处理）、`futures-util`、`base64`、`sysinfo`、`walkdir`、`rand`、`mime_guess`
- 工程与质量：
  - 格式与静态检查：`cargo fmt`、`cargo clippy`
  - 测试与打包：`cargo test`、`cargo build --release`
  - 提交规范：Conventional Commits（见 `CONTRIBUTING.md`）

## 目录结构（简要）
- `PreTackler/`：Rust CLI 工程
  - `src/main.rs`：参数解析与入口
  - `src/processor.rs`：核心处理逻辑（并发、调用、输出）
- `openspec/`：OpenSpec 元数据（本文件、变更提案等）
- `AGENTS.md`：AI 助手使用 OpenSpec 的顶层指引
- `CONTRIBUTING.md`：贡献与工作流说明

## 约定与规范
- 文档统一中文，必要时保留英文术语以减少歧义。
- 所有功能性改动必须先有 OpenSpec 变更提案，经评审再实施。
- 安全：严禁明文密钥入库；模型 API Key 通过环境变量或密钥文件注入（参考 `PreTackler`）。
- 观察性：优先结构化日志与基本性能指标；必要时引入 Trace。

## 构建与运行
- 构建：`cd PreTackler && cargo build --release`
- 运行：
  - 单文件：`pretackler <input_path> --version v1 --model <deepseek-model>`
  - 关键参数：
    - `--prompt <path>`：提示词模板（默认内置路径）
    - `--temperature <float>`（默认 0.65）、`--top-k <int>`（默认 1）
    - `--max-concurrency <int>`：最大并发（可选）

## 部署与发布
- 二进制发布：优先使用 GitHub Releases（手动或 CI）。
- 与 OpenSpec 集成：在 PR 描述中链接对应的 `openspec/changes/<change-id>/` 目录。

## 风险与注意事项
- 模型依赖与速率限制：对 DeepSeek 的依赖可能受限于额度与速率；必要时提供重试与退避策略。
- 私有代码与数据：在处理敏感仓库时，需遵循数据最小化与脱敏策略。
- 大目录并发：对超大文件树时，建议限制并发、增加超时与失败重试次数。

## 后续路线（建议）
- 增加本地摘要器（如 `onnx`/`ggml`）以降低外部依赖。
- 扩展更多 LLM 供应商适配层（OpenAI、Ollama、vLLM 网关）。
- 增加 `openspec validate` 的 CI 集成与变更影响评估。

---
本文件将随功能演进持续更新，所有修改需通过 OpenSpec 提案流程。

