# Issue tracker: 本地 Markdown

规格和工单保存在仓库内，通过文件读写操作维护。

## 存放位置

- 规格：`docs/specs/<feature-slug>.md`。已有规格直接沿用原文件。
- 工单：`.scratch/<feature-slug>/issues/<NN>-<slug>.md`，每张工单一个文件。
- 同一功能的工单从 `01` 开始，按依赖顺序编号，阻塞方在前。追加工单使用尚未分配的新编号，保留已有编号和文件名。

## 操作

- 发布规格或工单：创建或更新对应 Markdown 文件。遵守当前调用技能要求的确认步骤，创建文件不代替用户确认。
- 读取：读取对应文件的全文及讨论记录。
- 列表：枚举工单文件，读取 `Status` 和 `Lifecycle`；旧文件缺少 `Lifecycle` 时视为 `open`。
- 分诊：更新 `Status`，取值见 `triage-labels.md`。
- 开始工作：将 `Lifecycle` 设为 `in-progress`。
- 关闭：将 `Lifecycle` 设为 `closed`，追加关闭原因或验证结果，保留文件。
- 讨论：在文件末尾的 `Discussion` 中追加带日期的记录，保留已有讨论。
- 阻塞关系：在 `Blocked by` 中列出阻塞工单的编号、标题和相对 Markdown 链接；无阻塞时写 `None (can start immediately)`。
- 父子关系：来源为已有工单时，在子工单的 `Parent` 中链接父工单；拆票本身不修改或关闭父工单。
- 执行顺序：选择所有阻塞条件均已满足的工单。存在“真机验证通过”等条件时，以验证证据为准，关闭工单不自动表示该条件通过。

## 工单格式

```md
# <NN>: <标题>

**What to build:** 从用户视角描述可演示的端到端行为。

**Blocked by:** 阻塞工单的编号、标题和链接，或 None (can start immediately)。

**Status:** ready-for-agent

**Lifecycle:** open

- [ ] 验收条件
```

`Status` 表示分诊状态，`Lifecycle` 表示执行进度（`open`、`in-progress`、`closed`）。规格需要记录分诊状态时使用同一 `Status` 字段。保留技能要求的正文结构。
