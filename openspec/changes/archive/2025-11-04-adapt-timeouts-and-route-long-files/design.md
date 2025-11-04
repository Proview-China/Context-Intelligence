# 设计说明：自适应超时 + 双通道路由（禁止分片、不降并发）

## 核心思路
- 路由：根据文件 `bytes` 与 `lines` 阈值判断是否进入 Long-Channel；
- 通道：Normal-Channel 使用现有 request/idle 超时；Long-Channel 使用“不限时”或大超时（参数化，0 代表不限）；
- 并发：保留总并发估算值不变，将 worker 在两个通道之间按比例划分（缺省 50/50，或按队列长度动态调整），保证总和不降低；
- 禁止分片：任何实现不对单文件做切块；
- 观测：日志打印通道、阈值命中维度、最终超时设置。

## 参数（更新后）
- `--long-file-bytes-threshold`：默认 512KB（524_288）。
- `--long-file-lines-threshold`：默认 4_000 行。
- `--long-channel-enabled`：默认启用。
- `--long-channel-timeout-multiplier`：默认 5.0（将普通 `request/idle` 超时放大 5 倍）。
- `--long-channel-request-timeout`：可选，显式覆盖（传 0 表示不限时；未设置时按倍数计算）。
- `--long-channel-idle-timeout`：可选，显式覆盖（传 0 表示不限时；未设置时按倍数计算）。

## 数据流与调度
1) 扫描文件 → 计算 bytes/lines → 路由到 normal_queue 或 long_queue；
2) 基于总并发 N，分配 Nn + Nl = N；
3) 两个 JoinSet/任务循环并行拉取各自队列；
4) 每次请求前按通道覆盖有效超时（req/idle）：优先显式 long-channel 超时；否则按 `multiplier`×普通超时计算；0 表示不限时。

## 取舍
- 不限时可能导致卡顿：通过队列公平与总并发保障整体吞吐；
- 阈值的默认值需适中，允许用户覆盖。

## 兼容性
- 仅新增参数，默认启用长时通道且阈值温和；原行为可通过 `--long-channel-enabled=false` 关闭。
