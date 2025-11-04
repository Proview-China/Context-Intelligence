# PreTackler 使用说明（中文）

PreTackler 是用于批量生成“上下文摘要”的 Rust CLI 工具。它会扫描文件/目录，读取 Prompt 模板，调用 DeepSeek（流式 data:）生成摘要，并将结果以 `*.summary.<version>.md` 形式输出。支持并发、自适应上限、指数退避重试、流式空闲超时、可选令牌桶限速与故障注入验收。

## 快速开始

```bash
# 构建
cd PreTackler && cargo build --release

# 在项目根目录放置密钥（四选一，按顺序加载）
# 1) 环境变量 DEEPSEEK_API_KEY_FILE 指向密钥文件
# 2) ./deepseek_api_key.secret
# 3) $CARGO_MANIFEST_DIR/deepseek_api_key.secret
# 4) 环境变量 DEEPSEEK_API_KEY

# 准备提示词模板（默认使用工作目录 prompt_template.md）
# echo "你的系统提示词..." > prompt_template.md

# 处理单个文件
pretackler ./path/to/file.rs --version v1

# 处理目录（自定义参数）
pretackler ./repo --version v1 \
  --prompt ./prompt_template.md \
  --model deepseek-chat --temperature 0.6 --top-k 1 \
  --concurrency-ceil 16 \
  --skip-ext .png,.jpg --skip-large-file-size-mb 5
```

## 参数说明
- `--prompt <path>`：提示词模板（默认：`./prompt_template.md`），为空或缺失将报错退出。
- `--model <name>`：模型名称（默认：`deepseek-chat`）。
- `--temperature <f32>`：采样温度（默认：`0.65`）。
- `--top-k <u32>`：Top-K（默认：`1`）。
- `--concurrency-ceil <N>`：并发上限（可选），未设置时根据系统资源自适应估算，等效别名 `--max-concurrency`。
- `--skip-large-file-size-mb <MB>`：超过指定大小（MB）文件将跳过。
- `--skip-ext ext1,ext2`：按扩展名跳过（不区分大小写，可带或不带点）。
- `--connect-timeout <秒>`：连接超时（默认 15s）。
- `--request-timeout <秒>`：整体请求超时（默认 45s）。
- `--stream-idle-timeout <秒>`：流式空闲超时（默认 30s）。
- `--rate-limit-rps <f64>`：每秒请求数限速（默认关闭）。
- `--rate-limit-bytes-per-sec <u64>`：字节级限速（默认关闭）。
- `--verbose`：更详细日志（等待/退避/HTTP 状态/idle 触发）。
- `--inject-fault 429|5xx|idle`：仅用于本地验收测试的人为故障注入。
- 长/大文件与长时通道：
  - `--long-file-bytes-threshold <u64>`：默认 512KB（524_288）。
  - `--long-file-lines-threshold <u64>`：默认 4000 行。
  - `--long-channel-enabled`：默认启用。
  - `--long-channel-timeout-multiplier <f32>`：默认 5.0（将普通 request/idle 超时放大 5 倍）。
  - `--long-channel-request-timeout <秒>`：可选，显式覆盖（0 表示不限时）。
  - `--long-channel-idle-timeout <秒>`：可选，显式覆盖（0 表示不限时）。
  - `--long-channel-adaptive-idle-enabled`：默认启用；基于历史流间隔 p95 自适应放宽 idle 超时（不影响 0=不限时）。

## 输出目录结构
- 单文件：与源文件同目录生成 `filename.summary.<v>.md`。
- 目录：在源目录同级生成 `dirname.summaries.<v>/.../*.summary.<v>.md`，保留子目录结构。

## 日志示例
```
00:01 [1 / 245] 开始 /repo/a.rs
00:02 尝试#1 请求 /repo.summaries.v1/a.rs.summary.v1.md
00:02 HTTP 状态: 200 OK
00:03 [1 / 245] 完成 /repo.summaries.v1/a.rs.summary.v1.md 用时 2.10s 大小 12.3KB 速率 5.8KB/s
00:02 [skip] /repo/assets/logo.png - 扩展名匹配跳过: .png
00:05 触发流式 idle 超时（示例） 退避 1000ms
00:10 [5 / 245] 完成 ...
```

## 常见故障与建议
- 429 / 带宽不足：
  - 调小并发（`--concurrency-ceil`），或开启限速（`--rate-limit-*`）。
- 请求超时 / 网络抖动：
  - 缩短 `--stream-idle-timeout` 以更快失败重试；检查网络与代理。
- 模板为空：
  - 确认 `--prompt` 路径正确、文件内容非空。

## 验收（建议流程）
- 对小目录与 >1000 文件目录各执行一次，记录总耗时、成功率、重试次数。
- 使用故障注入验证不中断：`--inject-fault 429|5xx|idle`，结合 `--verbose` 与 `--stream-idle-timeout`。
- 执行脚本：`scripts/acceptance.sh <pretackler_bin> <small_dir> <large_dir> [version]`
