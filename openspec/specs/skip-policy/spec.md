# skip-policy Specification

## Purpose
TBD - created by archiving change improve-pretackler-cli. Update Purpose after archive.
## Requirements
### Requirement: 按大小跳过
Pretackler SHALL skip files larger than --skip-large-file-size-mb and log the reason.
`--skip-large-file-size-mb <MB>` 超过则跳过并打印原因。
#### Scenario: 大文件跳过
- Given 文件大小超过阈值
- When 扫描与处理
- Then 跳过该文件并打印提示

### Requirement: 按扩展名跳过
Pretackler SHALL skip files whose extensions match --skip-ext (case-insensitive) and log the reason.
`--skip-ext ext1,ext2`（不区分大小写）。
#### Scenario: 扩展名跳过
- Given 文件扩展名在列表内
- When 扫描
- Then 跳过该文件并打印提示

