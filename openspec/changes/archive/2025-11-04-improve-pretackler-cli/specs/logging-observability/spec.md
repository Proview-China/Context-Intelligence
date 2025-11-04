# 日志与可观测性能力规范

## ADDED Requirements

### Requirement: 关键阶段日志
Pretackler SHALL log start/retry/complete/fail/skip per file with index/total/timestamp/duration/rate/ETA/reason.
按文件打印 开始/尝试/重试/完成/失败/跳过，含索引、总数、时间戳、耗时、速率、ETA、错误原因。
#### Scenario: 基础日志
- Given 正常执行
- When 处理多个文件
- Then 日志包含上述关键阶段信息

### Requirement: verbose 模式
When --verbose is enabled, Pretackler SHALL log wait/backoff, HTTP status codes, and idle timeout triggers without leaking secrets.
`--verbose` 开启后打印等待/退避时间、HTTP 状态、idle 超时触发等细节，不包含任何敏感信息。
#### Scenario: 详细日志
- Given 加入 `--verbose`
- When 运行命令
- Then 可见退避等待、HTTP 状态码与 idle 超时触发记录
