# concurrency-rate Specification

## Purpose
TBD - created by archiving change improve-pretackler-cli. Update Purpose after archive.
## Requirements
### Requirement: 自适应并发与上限
Pretackler SHALL estimate concurrency from CPU/memory/network and SHALL cap it with --concurrency-ceil.
基于 CPU/内存与（可选）简易网络采样估算并发度，并支持 `--concurrency-ceil <N>` 裁剪。
#### Scenario: 默认自适应
- Given 未设置并发上限
- When 运行命令
- Then 估算并发在安全区间内（如 4..=64）并稳定运行

#### Scenario: 显式上限
- Given `--concurrency-ceil 16`
- When 运行命令
- Then 实际并发不超过 16

### Requirement: 可选令牌桶限速
Pretackler SHALL support optional rate limiting via --rate-limit-rps and --rate-limit-bytes-per-sec.
支持 `--rate-limit-rps <f64>`、`--rate-limit-bytes-per-sec <u64>`，默认关闭。
#### Scenario: 开启限速
- Given 设置 RPS 与字节速率
- When 运行命令
- Then 请求/字节发送速率受控，日志展示限速生效信息

