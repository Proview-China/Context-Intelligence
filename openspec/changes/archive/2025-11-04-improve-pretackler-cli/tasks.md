# 任务清单（按序执行，逐步可验证）

## M1（基础能力）
- [x] CLI 增参：`--prompt`、`--model`、`--temperature`、`--top-k`、`--concurrency-ceil`、`--skip-ext`、`--skip-large-file-size-mb`、`--verbose`
- [x] Prompt 读取与非空校验（默认 `./prompt_template.md`）
- [x] WriterGuard：临时文件写入与原子重命名、失败清理
- [x] 并发上限：在现有并发逻辑上增加上限裁剪
- [x] 日志（基础）：开始/完成/失败/跳过（含索引/总数/时间/速率）
- [x] 密钥加载顺序实现与脱敏日志
- [x] README（中文）：参数表、快速开始与最小示例

## M2（稳定性与可观测）
- [x] 指数退避重试：基数500ms、倍率2.0、最大30s、最多5次
- [x] 超时：`--connect-timeout`、`--request-timeout`、`--stream-idle-timeout`
- [x] 流式 data: 解析、实时落盘、空闲监测
- [x] 日志（扩展）：尝试/重试/HTTP状态/idle超时触发
- [x] 自适应并发：CPU/内存采样；网络采样可失败回退
- [x] README：示例日志、常见故障与建议

## M3（限速与验收）
- [x] 令牌桶：`--rate-limit-rps`、`--rate-limit-bytes-per-sec`（可选）
- [x] 验收脚本/命令：小目录与>1000文件目录各跑一次，记录指标（`scripts/acceptance.sh`）
- [x] 人造错误注入与验证：`--inject-fault 429|5xx|idle`
- [x] 文档完善与最终校验（README 已更新）

## 验证
- [x] `pretackler <dir> --version v1 --prompt ./prompt_template.md --model deepseek-chat` 正常完成并产出 `*.summary.v1.md`
- [x] 触发跳过策略时打印原因；触发重试与超时时打印相应日志（M1 仅包含跳过与失败日志）
