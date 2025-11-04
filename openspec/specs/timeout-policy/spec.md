# timeout-policy Specification

## Purpose
TBD - created by archiving change adapt-timeouts-and-route-long-files. Update Purpose after archive.
## Requirements
### Requirement: 基于大小/行数的超时自适应（降低阈值）
Pretackler SHALL map files to timeout policies based on bytes/lines thresholds.
#### Scenario: 命中 bytes 阈值
- Given 文件大小 >= `--long-file-bytes-threshold`（默认 512KB）
- When 运行 pretackler
- Then 应应用 Long-Channel 的超时策略

#### Scenario: 命中 lines 阈值
- Given 文件行数 >= `--long-file-lines-threshold`（默认 4_000 行）
- When 运行 pretackler
- Then 应应用 Long-Channel 的超时策略

### Requirement: Long-Channel 覆盖超时（默认 5 倍）
Pretackler SHALL apply 5x (configurable) timeouts for Long-Channel and MUST treat 0 as unlimited for request/idle.
#### Scenario: 不限时配置
- Given `--long-channel-request-timeout 0 --long-channel-idle-timeout 0`
- When 处理长文件
- Then 请求不因整体超时或 idle 超时而中断

#### Scenario: 倍数放大
- Given 未显式覆盖 long-channel 超时
- When 处理长文件
- Then 实际超时 = 普通超时 × 5（默认）

