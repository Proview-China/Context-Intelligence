# I/O 幂等与临时文件能力规范

## ADDED Requirements

### Requirement: 临时文件与原子落盘
Pretackler SHALL write to a temporary file and MUST atomically rename to the final path on success, cleaning up on failure.
写入时先写 `*.summary.<v>.md.tmp-<随机>`，完成后原子重命名为目标路径；失败/中断清理临时文件。
#### Scenario: 正常完成
- Given 流式写入完成
- When 关闭 WriterGuard
- Then 目标文件就位且无临时文件残留

#### Scenario: 异常中断
- Given 写入过程中出错
- When 清理过程触发
- Then 临时文件被移除，不影响后续重试
