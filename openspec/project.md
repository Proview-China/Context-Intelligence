# 项目上下文（Project Context）

## 项目目的（Purpose）
本项目旨在解决 Coding Agent 在实际执行任务时的三类核心问题：
1) 上下文丢失：由于上下文窗口限制导致必须压缩（compact）而引发的“遗忘”；
2) 上下文过大引发的幻觉：无效或稀释的线索导致 LLM 推理失真；
3) 低效与不准确的自主检索：Agent 在大型代码库中低效、低准确地寻找线索。

为此，我们构建“高性能、可扩展的项目知识供给系统”：
- 将仓库中所有代码与文档构建为“巨型且高性能的完整图谱（code+docs graph）”，主动、精确地喂给 Agent；
- 最小化对上下文窗口的依赖，以结构化引用与按需供给替代大段拼接；
- 基于对 gemini-cli 与 codex-cli 的改造，打造通用 Agent，使其可渗透进需要计算机工作的各行业，真正提升效率，让 Agent 走进千家万户。

## 技术栈（Tech Stack）
- 语言：Rust（核心实现）、Python（高性能服务与工具链补充）
- 数据存储：PostgreSQL（主）、可选 Redis（缓存/队列/速率限制）
- Rust 框架：优先选用高性能与生态成熟方案（如 Tokio、Axum、Actix、Tonic 等，按具体场景取舍）
- Python 框架：FastAPI（高性能异步 I/O）、Pydantic（数据建模）
- 其他：必要时选用可显著提升性能与稳定性的组件与架构

## 项目约定（Project Conventions）

### 代码风格（Code Style）
- Rust：`rustfmt` + `clippy` 强制；模块/crate 命名使用 `kebab-case`（crate）与 `snake_case`（模块/文件），公共 API 使用清晰、稳定的命名；
- Python：`ruff`/`flake8` + `black` + `isort`；类型注解尽可能完整（`mypy`/`pyright` 可选）；
- 提交规范：Conventional Commits（`feat|fix|perf|refactor|docs|test|chore|build|ci|revert`）；
- 文档语言：统一中文；技术术语保留英文原文以减少歧义（示例：Agent、LLM、context window）。

### 架构模式（Architecture Patterns）
- 仓库形态：多仓（包含多个处理部件/服务模块于本总仓库 Context-Intelligence）；
- 分层建议：接口（API/gRPC）层、服务（domain/service）层、数据访问（storage/index）层、基础设施（infra）层；
- 领域对象：以“代码/文档图谱”为核心对象，围绕构建、索引、检索、供给等能力组织模块；
- 通信：内部优先使用异步 RPC（gRPC/Tonic）或消息（Redis Stream/其他队列）按需选型；
- 可观测性：日志（结构化）、Metrics（Prometheus 格式）、Tracing（OpenTelemetry）。

### 测试策略（Testing Strategy）
- 单元测试：覆盖核心算法/索引/解析组件；
- 集成测试：覆盖服务接口、数据存储交互、跨模块流程；
- 端到端（可选）：关键用户路径与回归用例；
- 覆盖率：采用常见默认门槛，针对关键路径设定更高标准；
- CI：保存并展示测试结果与覆盖率，`lint → build → test` 流程门禁。

### Git 工作流（Git Workflow）
- 远端：后续推送至 `Proview-China/Context-Intelligence`（总仓库，多仓聚合）；
- 策略：Trunk-Based 为主，短分支 + 小步提交 + 频繁合并；
- 分支命名：`feat/*`、`fix/*`、`perf/*`、`refactor/*`、`chore/*`、`docs/*`；
- PR 规则：小粒度、带描述与关联 issue/变更；通过 CI 后再合并；
- 变更管理：功能/行为变更需先通过 OpenSpec 变更提案流程（见 openspec/AGENTS.md）。

## 领域背景（Domain Context）
- LSP（Language Server Protocol）：用于静态/动态代码分析的数据通道与协议；
- 静态代码分析：AST/符号/依赖/类型/控制流图等抽取与索引；
- 动态代码分析：运行期 Trace/覆盖率/事件流/性能指标采集；
- 超级向量库：面向大规模代码与文档的多模态嵌入、增量更新与检索（需高吞吐、低延迟、强一致性策略）。

## 重要约束（Important Constraints）
- 性能为首要目标：所有关键路径均需以性能为一等公民优化与验收；
- 安全性：
  - 网络传输严禁明文，默认启用 TLS/HTTPS；
  - 严格的密钥/凭证管理（环境变量/密钥管理服务），禁入库；
  - 数据最小化原则，外发前脱敏/加密；
- 隐私与合规：用户数据不出本地或受控边界；与外部服务交互需遵循数据最小化与审计。

## 外部依赖（External Dependencies）
- `codex-cli`：作为一部分 Agent 的执行与交互入口；
- `gemini-cli`：选用 Gemini-Pro 作为更强的总控 Agent 与对话入口；
- 数据库与基础设施：PostgreSQL、Redis（可选），以及可观测性栈（按需）。
