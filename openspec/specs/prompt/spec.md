# prompt Specification

## Purpose
TBD - created by archiving change improve-pretackler-cli. Update Purpose after archive.
## Requirements
### Requirement: 指定 Prompt 模板文件
Pretackler SHALL accept --prompt and MUST fail when the template is missing or empty.
CLI 支持 `--prompt <path>` 指定提示词模板，默认 `./prompt_template.md`。
#### Scenario: 默认路径存在
- Given 目录下存在 `prompt_template.md`
- When 运行 `pretackler <INPUT> --version v1`
- Then 成功读取模板并用于请求

#### Scenario: 显式传入路径
- Given `--prompt ./my_prompt.md`
- When 运行命令
- Then 使用 `my_prompt.md` 内容作为模板

#### Scenario: 模板缺失或为空
- Given 模板文件不存在或内容为空
- When 运行命令
- Then 立刻报错退出（非0），打印明确错误信息

