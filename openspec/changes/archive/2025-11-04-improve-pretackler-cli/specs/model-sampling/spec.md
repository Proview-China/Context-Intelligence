# 模型与采样（含流式）能力规范

## ADDED Requirements

### Requirement: 模型与采样参数
Pretackler SHALL accept --model, --temperature, and --top-k and apply them to requests.
支持 `--model <name>`（默认 `deepseek-chat`）、`--temperature <f32>`（默认0.65）、`--top-k <u32>`（默认1）。
#### Scenario: 默认参数
- Given 未提供相关参数
- When 运行命令
- Then 使用默认模型与采样值

#### Scenario: 自定义参数
- Given 显式传入 `--model x --temperature 0.7 --top-k 2`
- When 运行命令
- Then 请求中包含这些参数

### Requirement: 流式输出
Pretackler SHALL request with stream=true, parse data: lines, and write output incrementally.
请求使用 `stream=true`，逐行解析 `data:`，实时写入摘要目标文件（临时文件）。
#### Scenario: 正常流式
- Given 服务端持续返回 `data:` 行
- When 运行命令
- Then 逐行写入并最终生成完整摘要文件

#### Scenario: 中途错误
- Given 过程中出现可重试错误
- When 触发重试
- Then 日志显示尝试/重试，并最终成功或失败退出
