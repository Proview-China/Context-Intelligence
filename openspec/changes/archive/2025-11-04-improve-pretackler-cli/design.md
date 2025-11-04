# 设计说明：PreTackler 能力扩展（保持向后兼容）

本文档阐述在保留原有接口（`pretackler <INPUT> --version <v>`）基础上，分阶段新增能力的设计。所有新增参数需出现在 `--help` 中，并补充中文 README 与最小可验证示例。

## 总体原则
- 默认值安全、可用，保持向后兼容；新增能力均为可选。
- 先实现最小可靠版本（MVP），再逐步增强（令牌桶、限速自适应等）。
- I/O 幂等、日志可观测、安全不泄密贯穿始终。

## 参数与行为设计

1) Prompt 模板
- 新增：`--prompt <path>`，默认 `./prompt_template.md`。
- 若文件不存在/为空：立刻报错退出（退出码 ≠0）。

2) 模型与采样
- 新增：`--model <name>`（默认 `deepseek-chat`）、`--temperature <f32>`（默认 `0.65`）、`--top-k <u32>`（默认 `1`）。
- HTTP 请求：使用 `stream=true`，逐行解析 `data:`，实时写入摘要目标文件（临时文件路径，见 I/O）。

3) 并发与限速
- 自适应并发：基于 `sysinfo` 采样 CPU/内存占用，并做一次简易网络吞吐采样（可失败）。
- 并发上限：`--concurrency-ceil <N>`（可选）；若未配置，以自适应估算值裁剪至安全区间（例如 4..=64）。
- 令牌桶（可选）：`--rate-limit-rps <f64>`、`--rate-limit-bytes-per-sec <u64>`；默认关闭。实现为轻量本地令牌桶（基于时间片补充）。

4) 超时与重试
- 指数退避参数：基数 500ms、倍率 2.0、最大 30s、最多 5 次。
- 连接超时：`--connect-timeout <秒>`（默认 15s）。
- 整体请求超时：`--request-timeout <秒>`（默认 45s）。
- 流式空闲超时：`--stream-idle-timeout <秒>`（默认 30s；该时间内无新 chunk 视为失败并重试）。
- 可重试错误：429、5xx、超时、连接错误、流未完成/空闲。

5) I/O 幂等与临时文件
- 写入策略：先写 `*.summary.<v>.md.tmp-<随机>`，成功后原子重命名为 `*.summary.<v>.md`；失败/中断清理临时文件。

6) 跳过策略
- `--skip-large-file-size-mb <MB>`：超过则跳过（打印原因）。
- `--skip-ext ext1,ext2`：跳过指定扩展名（不区分大小写，打印原因）。

7) 日志与可观测性
- 按文件打印：开始/尝试/重试/完成/失败/跳过；包含索引、总数、时间戳、耗时、速率、ETA、错误原因。
- `--verbose`：打印等待/退避、HTTP 状态码、idle 超时触发等调试细节。

8) 安全与密钥
- 加载顺序：`DEEPSEEK_API_KEY_FILE` → `./deepseek_api_key.secret` → `$CARGO_MANIFEST_DIR/deepseek_api_key.secret` → 环境变量 `DEEPSEEK_API_KEY`。
- 严禁在日志/文件中打印密钥或其片段。

9) 文档（中文 README）
- 快速开始、参数表、输出目录结构、示例日志、常见故障与建议（如调小并发/限速、缩短 idle timeout）。

10) 验收
- 小目录与 >1000 文件目录各跑一次，记录总耗时、成功率、重试次数。
- 人为制造 429/5xx/网络抖动/idle 超时，确认快速反馈且不中断其他文件处理。

## 模块与结构调整
- `Args` 扩展：新增上述全部参数，带中文帮助文本；`--help` 自动展示。
- `processor`：
  - 增加 `RateLimiter`（可选）与 `RetryPolicy` 组件。
  - 并发控制：估算并发（`estimate_concurrency()`）+ 上限裁剪。
  - 流式读取：`StreamWatcher` 监测空闲超时。
  - I/O：`WriterGuard` 负责临时文件与原子重命名。
  - 跳过策略：`should_skip(entry) -> Option<Reason>`。
- 配置体 `PretacklerConfig` 扩展，保持字段默认值合理。

## 最小实现里程碑（与 README 同步）
M1：Prompt/模型采样/并发上限/临时文件/日志（基础）/密钥顺序/跳过策略
M2：指数退避重试 + 空闲超时 + 观测增强 + 自适应并发
M3：令牌桶限速 + 网络采样回退逻辑

## 示例命令与预期日志片段
- 构建：`cd PreTackler && cargo build --release`
- 示例：
  - `pretackler repo/ --version v1 --prompt ./prompt_template.md --model deepseek-chat --temperature 0.6 --top-k 1 --concurrency-ceil 16 --skip-ext .png,.jpg --skip-large-file-size-mb 5`
- 预期日志（简化）：
  - `[00:00:01] [1/245] 开始 a/b.rs (估计并发=12, ETA=03:12)`
  - `[00:00:03] [1/245] 尝试#1 200 OK 流式开始` 
  - `[00:00:05] [1/245] 完成 12.3KB 用时2.1s 速率5.8KB/s`
  - `[00:00:02] 跳过 c/large.bin 超过阈值 5MB`
  - `[00:00:10] [5/245] 尝试#2 429 重试 1.0s ...`
