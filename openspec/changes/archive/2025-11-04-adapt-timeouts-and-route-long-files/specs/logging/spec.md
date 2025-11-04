# 日志规范（通道与策略可观测）

## ADDED Requirements

### Requirement: 打印通道与阈值命中
Logs SHALL indicate the channel (normal/long) and which threshold (bytes/lines) was met per file.
#### Scenario: 混合输入
- Given 同时存在长/短文件
- When 运行 pretackler
- Then 日志可见对应通道与命中类型

### Requirement: 打印实际超时配置
Logs SHALL include effective request/idle timeout values per file (0=unlimited).
#### Scenario: 长通道不限时
- Given Long-Channel 配置为不限时
- When 处理长文件
- Then 日志显示 request=0, idle=0（或等价文案）
