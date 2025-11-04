# 并发约束规范（禁止降并发与分片）

## ADDED Requirements

### Requirement: 禁止降低整体并发
When dual channels are enabled, total concurrency SHALL be equal to or greater than the estimated/user-capped value.
#### Scenario: 并发守恒
- Given `--concurrency-ceil 16`
- When 启用双通道
- Then 总并发仍为 16（或估算值），不低于单通道设置

### Requirement: 禁止分片处理
Pretackler MUST NOT perform content chunking on any single file.
#### Scenario: 无分片
- Given 任意输入文件
- When 运行 pretackler
- Then 不产生对子文件的切块请求与二级汇总
