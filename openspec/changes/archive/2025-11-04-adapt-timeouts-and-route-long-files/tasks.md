# 任务清单（按序执行）

## P1（策略与双通道最小落地）
- [x] CLI 增加阈值与策略参数：
  - `--long-file-bytes-threshold <u64>`（默认：524_288，即 512KB）
  - `--long-file-lines-threshold <u64>`（默认：4_000 行）
  - `--long-channel-enabled`（默认启用）
  - `--long-channel-request-timeout <秒>`（默认：按普通超时 5 倍计算；传 0 表示不限时）
  - `--long-channel-idle-timeout <秒>`（默认：按普通空闲超时 5 倍计算；传 0 表示不限时）
  - （可选）`--long-channel-timeout-multiplier <f32>`（默认：5.0，用于按倍数放大普通超时）
- [x] Processor：两条通道（normal/long），保持总并发不降低（按 50/50 或全量切分，和为总并发）。
- [x] 路由策略：满足任一阈值即判定为 long；支持 bytes/lines 两阈值。
- [x] 调用时按通道覆盖 request/idle 超时（0=不限，使用极大值/关闭 idle 超时）。
- [x] 日志：打印通道名、最终超时配置（req/idle）。
- [x] README：新增参数说明与建议值。

## P2（策略自适应与公平调度）
- [x] 历史吞吐采样，微调长通道 idle 超时（p95 × 1.2；可与 0=不限时并存）。
- [x] 公平调度：统一 worker 池 + 轮询两队列（RR），避免饥饿。
- [x] 文档补充：`--long-channel-adaptive-idle-enabled`（默认 启用，可关闭）。

## 验证
- [x] 混合目录：含 >=1 个长文件（超阈值）与多个短文件（可用注入/模拟目录验证）。
- [x] 长文件成功完成且不被超时中断；短文件并行完成，总并发与未启用策略时不降低。
- [x] 日志可见通道与超时信息；禁用分片（未实现分片逻辑）。
