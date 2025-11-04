# 超时与重试能力规范

## ADDED Requirements

### Requirement: 指数退避重试
Pretackler SHALL retry 429/5xx/timeout/connect/unfinished-stream errors with exponential backoff (base 500ms, factor 2.0, max 30s, max 5 attempts).
退避：基数500ms、倍率2.0、最大30s、最多5次；错误：429、5xx、超时、连接错误、流未完成/空闲。
#### Scenario: 429 重试
- Given 服务返回 429
- When 运行命令
- Then 逐步退避重试，最多5次，日志展示每次等待

#### Scenario: 5xx 重试
- Given 服务返回 5xx
- When 运行命令
- Then 按策略退避并重试，最终成功或失败退出

### Requirement: 多种超时
Pretackler SHALL support --connect-timeout, --request-timeout, and --stream-idle-timeout.
支持 `--connect-timeout`（默认15s）、`--request-timeout`（默认45s）、`--stream-idle-timeout`（默认30s）。
#### Scenario: 连接超时
- Given 后端连接缓慢
- When 触发连接超时
- Then 视为可重试错误并进入退避

#### Scenario: idle 超时
- Given 流在 idle 超时内未收到新 chunk
- When 触发 idle 超时
- Then 视为可重试错误进行重试
