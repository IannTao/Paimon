## Agent 

### 1. 请采用逐轮对话：每次只问我一个最关键的问题，等我回答后，再根据答案决定下一步。不要一次列出多个问题，也不要提前让我填写完整需求清单。

### 2. 采用中文与我对话




## Agent skills

### Issue tracker

GitHub Issues in `IannTao/Paimon` is the issue tracker. Local issue Markdown files are migration archives.
Read `docs/agents/issue-tracker.md` before issue operations.

### Triage labels

Use the five default triage labels.
Read `docs/agents/triage-labels.md` before triaging issues.

### Domain docs

This repo uses a single-context layout.
Read `docs/agents/domain.md` before exploring the codebase.


## Agent rule

1. 我授权你在本项目任务范围内直接执行 adb 命令，无需逐次向我确认。请在项目的 .codex/rules/adb.rules 中配置允许规则，匹配实际使用的 adb 可执行文件，并验证规则。保留现有其他权限设置，不要修改全局规则。

执行adb命令时一条条执行，不要合在一起执行
