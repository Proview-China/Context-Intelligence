# security-keys Specification

## Purpose
TBD - created by archiving change improve-pretackler-cli. Update Purpose after archive.
## Requirements
### Requirement: 密钥加载顺序
Pretackler SHALL load API keys in the specified order and MUST error when all sources are absent.
加载顺序：`DEEPSEEK_API_KEY_FILE` → `./deepseek_api_key.secret` → `$CARGO_MANIFEST_DIR/deepseek_api_key.secret` → 环境变量 `DEEPSEEK_API_KEY`。
#### Scenario: 指定密钥文件环境变量
- Given 设置 `DEEPSEEK_API_KEY_FILE`
- When 运行命令
- Then 使用该文件内容作为密钥

#### Scenario: 回退到默认路径
- Given 未设置文件环境变量且本地存在 `./deepseek_api_key.secret`
- When 运行命令
- Then 使用该文件内容作为密钥

#### Scenario: 环境变量兜底
- Given 上述文件均不存在
- When 运行命令且设置 `DEEPSEEK_API_KEY`
- Then 使用该环境变量作为密钥

### Requirement: 禁止泄露
Pretackler MUST NOT print or write any API key material in logs or files.
日志与输出文件中严禁包含密钥或其片段。
#### Scenario: 敏感信息检查
- Given 开启 verbose
- When 运行命令
- Then 日志不包含任何密钥片段

