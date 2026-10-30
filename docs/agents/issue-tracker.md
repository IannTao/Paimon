# Issue tracker: GitHub Issues

## 权威来源与引用

- 仓库：`IannTao/Paimon`，地址为 https://github.com/IannTao/Paimon 。
- 开发总览：https://github.com/IannTao/Paimon/issues/1 。总览是聚合项，具体实现选择其当前子工单。
- 工单状态、依赖、验收结果和后续讨论以 GitHub Issues 为准。全部 `gh` 命令显式指定 `--repo IannTao/Paimon`，或在 API 路径写明该仓库，不依赖本地 Git remote 的缺省值。
- `#16` 等引用指 GitHub issue 编号。迁移前的“本地 15”或 `local:15` 通过[编号映射](../issues/index.md)转换，不能把本地编号直接当成 GitHub 编号。
- 规格和 ADR 在仓库文档维护；需求变更同步总览及受影响工单。GitHub 总览同时保留此次工单迁移的完整规格、兼容合同和架构决定，供直接阅读。

## 读取与选择任务

1. 读取目标 issue 全文、所有讨论、标签、状态、关闭原因和原生依赖；有父总览时同时检查当前路线与范围。
2. 用户给出本地编号时先查映射。用户只给编号时默认 GitHub 编号；与标题或会话意图冲突时，一次只澄清一个关键问题。
3. 选择 open 的具体任务，且所有阻塞条件均满足。总览和已关闭历史任务不作为实现入口。
4. 真机验证等前置条件以证据为准，issue closed 不自动表示验证通过；`not_planned` 关闭不算完成能力。

```powershell
gh issue view 16 --repo IannTao/Paimon --json number,title,body,state,stateReason,labels,comments
gh issue list --repo IannTao/Paimon --state open --limit 100 --json number,title,state,labels
```

## 发布和更新

- 发布前搜索全部状态的现有 issue，核对标题、来源和迁移标记，避免重复创建。
- 每个具体任务一个 issue，正文保留 What to build、Acceptance criteria、Blocked by，以及技能要求的其他结构。多行正文写入 UTF-8 临时文件，使用 `--body-file` 或结构化 API 参数提交。
- 默认使用 `ready-for-agent` 标签，其他分诊情况按 `triage-labels.md`。保留与当前操作无关的标签。
- 后续新增任务直接使用 GitHub 编号，不再创建一套本地编号或本地工单；原有 local ID 只用于追溯迁移。
- 补充讨论使用 issue comment，保留既有讨论。更新范围时重写当前描述，并说明影响到的依赖或验收条件。
- 开始工作将 issue 正文的 `Lifecycle` 设为 `in-progress` 并保持 open；尚未开始为 `open`。GitHub 目前未配置独立 Project 状态字段。
- 验收完成才以 `completed` 关闭；范围取消或被新方案取代时以 `not planned` 关闭并说明原因，通常使用 `wontfix` 标签。保留已有验收和失败记录。

## 依赖和父子关系

- 使用 GitHub 原生 blocking dependency，同时在正文 Blocked by 保留可读的 issue 链接及额外证据条件。
- 阻塞关系 API：`repos/IannTao/Paimon/issues/<GitHub编号>/dependencies/blocked_by`。添加请求中的 `issue_id` 是阻塞 issue 的 REST 数字 id，从 issue API 读取；它不是 GitHub 编号，也不是 GraphQL node id。
- 当前具体工单是开发总览的原生 sub-issue；历史 01–06 仅作为已取代方案的归档链接，不混入当前任务进度。
- 父工单派生子任务时使用原生 sub-issue 关系并保留 Parent 链接。拆分本身不修改或关闭父工单。
- 移除或改变依赖时同时更新原生关系与正文。不要用关闭前置任务来替代未通过的验证门槛。

## 本地迁移归档

2026-10-31 已将 `.scratch/android-auto-pickup/issues/` 的 18 张工单迁移到 GitHub。原文件保留为历史快照，顶部 GitHub 链接指向当前记录；旧 Status/Lifecycle 及讨论只反映迁移前状态，后续不据此分派任务。

编号与源快照摘要见[迁移索引](../issues/index.md)及 `docs/issues/github-migration.json`。本地文件和迁移脚本用于追溯与防重复，不是第二个活跃 tracker。
