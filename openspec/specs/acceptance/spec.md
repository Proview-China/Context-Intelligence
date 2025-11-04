# acceptance Specification

## Purpose
TBD - created by archiving change improve-pretackler-cli. Update Purpose after archive.
## Requirements
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

### Requirement: 混合目录通过
Directories with at least one long file and multiple short files SHALL complete without timeout interruptions for long files.
#### Scenario: 不中断完成
- Given 长/短文件混合目录
- When 运行 pretackler（启用长通道默认配置）
- Then 所有文件处理完成，长文件不因 request/idle 超时中断

### Requirement: 并发不降低
The total concurrency after enabling dual channels SHALL NOT decrease compared to baseline.
#### Scenario: 并发验证
- Given 记录启用前后并发
- When 对比日志与估算输出
- Then 前后一致或不降低

