# 验收规范

## ADDED Requirements

### Requirement: 小目录与千级目录验证
Pretackler SHALL record total duration, success rate, and retry count for both small and >1000-file directories.
对小目录与 >1000 文件目录各执行一次，记录总耗时、成功率、重试次数。
#### Scenario: 小目录
- Given 目标目录包含少量文件
- When 执行命令
- Then 完成并输出统计指标

#### Scenario: 大目录
- Given 目标目录包含 >1000 文件
- When 执行命令
- Then 完成并输出统计指标

### Requirement: 人工错误注入
Pretackler SHALL continue processing other files while retrying the current one when 429/5xx/jitter/idle faults are injected.
制造 429/5xx/网络抖动/idle 超时，处理不中断其他文件。
#### Scenario: 429 注入
- Given 人为降低限速或服务返回 429
- When 执行命令
- Then 仅当前文件重试，其他文件可继续处理
