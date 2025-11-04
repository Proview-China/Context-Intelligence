# 路由（双通道）规范

## ADDED Requirements

### Requirement: 双通道队列
Pretackler SHALL maintain Normal-Channel and Long-Channel queues; files meeting thresholds SHALL route to Long-Channel.
#### Scenario: 混合目录
- Given 目录包含长/短文件
- When 运行 pretackler
- Then 长文件进入 Long-Channel，短文件进入 Normal-Channel

### Requirement: 总并发不降低
The sum of concurrent workers across channels SHALL NOT be lower than the original estimated/user-capped concurrency.
#### Scenario: 保持并发
- Given 未变更 `--concurrency-ceil`
- When 启用双通道
- Then 总并发不低于启用前的并发值
