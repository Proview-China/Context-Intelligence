# docs Specification

## Purpose
TBD - created by archiving change improve-pretackler-cli. Update Purpose after archive.
## Requirements
### Requirement: 中文文档完善
The README SHALL include quick start, parameter table, output structure, sample logs, and troubleshooting guidance.
README 提供快速开始、参数表、输出目录结构、示例日志、常见故障与建议。
#### Scenario: 参数自查
- Given 阅读 README
- When 查阅参数表
- Then 能看到 `--prompt`、`--model`、`--temperature`、`--top-k`、`--concurrency-ceil`、`--rate-limit-*`、`--connect-timeout`、`--request-timeout`、`--stream-idle-timeout`、`--skip-*`、`--verbose` 等说明

#### Scenario: 故障排查
- Given 网络抖动或 429
- When 查阅故障章节
- Then 获得调小并发/限速、缩短 idle timeout 等建议

