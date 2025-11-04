# 贡献指南（CONTRIBUTING）

本项目采用 OpenSpec 规范驱动开发，统一中文文档与提交规范。请在提交任何改动前阅读本指南。

## 开发流程总览
- 变更前：先创建 OpenSpec 变更提案（proposal + tasks + specs delta），并通过严格校验。
- 实施阶段：仅在提案通过评审后进行开发，按任务清单逐项完成并勾选。
- 归档阶段：部署完成后执行归档，将变更移动到 `openspec/changes/archive/`。

## OpenSpec 工作流
1) 创建变更
- 选择唯一的 `change-id`（kebab-case、动词开头，例如：`add-two-factor-auth`）。
- 在 `openspec/changes/<change-id>/` 下创建：
  - `proposal.md`：为何、改什么、影响面。
  - `tasks.md`：可执行任务清单，按序实现。
  - `specs/<capability>/spec.md`：使用 `## ADDED|MODIFIED|REMOVED Requirements`，每条 `### Requirement:` 至少包含一个 `#### Scenario:`。
- 本地校验：`openspec validate <change-id> --strict`
- 提交 PR 请求评审，通过后方可进入实施。

2) 实施变更
- 阅读 `proposal.md`（必读）、`design.md`（如存在）、`tasks.md`（执行顺序）。
- 开发中保持小步提交；完成后确保 `tasks.md` 全部 `- [x]`。

3) 归档与发布
- 部署完成后，将 `openspec/changes/<change-id>/` 归档：
  - 建议：`openspec archive <change-id> --yes`（工具型变更可 `--skip-specs`）。
- 确认通过 `openspec validate --strict`。

详细说明见：`openspec/AGENTS.md`。

## 分支与提交规范
- 分支命名：
  - 功能：`feat/<change-id>`
  - 修复：`fix/<topic>`
  - 性能：`perf/<topic>`
  - 重构：`refactor/<topic>`
  - 文档：`docs/<topic>`
  - 杂项：`chore/<topic>`
- 提交信息（Conventional Commits）：
  - `type(scope): message`
  - 常见类型：`feat`、`fix`、`perf`、`refactor`、`docs`、`test`、`chore`、`build`、`ci`、`revert`
  - 示例：`feat(pre-tackler): 目录并发处理与流式输出`

## 代码与文档约定
- 文档语言：统一中文；保留英文术语以减少歧义（LLM、Agent、context window）。
- Rust：`rustfmt` + `clippy` 强制，公共 API 命名清晰稳定。
- Python：`black` + `ruff` + `isort`，尽可能补全类型注解。
- 安全：严禁明文密钥入库；API Key 使用环境变量或密钥文件（参见 `PreTackler` 实现）。
- 观察性：优先结构化日志、基本指标与 Trace（按需接入）。

## PR 要求
- 对应一个明确的变更提案（或说明为何无需提案，例如：文档/格式修正）。
- 变更范围清晰，小而可审；附带影响说明与验证方式。
- CI 通过（lint → build → test）；如引入依赖，说明安全与体积影响。

## 常用命令示例
```bash
# 校验变更（严格模式）
openspec validate <change-id> --strict

# 首次推送
git branch -M main
git push -u origin main

# 功能开发分支
git checkout -b feat/<change-id>
# ...开发...
git add -A
git commit -m "feat(<scope>): <message>"
git push -u origin feat/<change-id>
```

## 联系方式
- 远程仓库：`Proview-China/Context-Intelligence`
- 任何不确定之处，请先创建 issue 或草拟提案征求意见。

```
备注：本指南为团队约定的最小集合，后续可根据实际协作迭代。
```

