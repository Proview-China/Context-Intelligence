# 验收规范

## ADDED Requirements

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
