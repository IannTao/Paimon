# GitHub 工单索引

**权威 tracker：** [IannTao/Paimon Issues](https://github.com/IannTao/Paimon/issues)。

**开发总览：** [BetterGI 路径点兼容与全自动跑图采集](https://github.com/IannTao/Paimon/issues/1)。

**下一项：** [GitHub #16：导入并预检 BetterGI 路径点原文件](https://github.com/IannTao/Paimon/issues/16)（本地 15）。

2026-10-31 迁移了 18 张本地工单：11 张当前待办、1 张已完成、6 张已被新方案取代的历史任务。当前计划的 12 张工单作为总览的原生子任务；历史 01–06 只保留归档链接，不计入当前计划进度。

迁移校验：已通过。18 张正文与历史讨论、标签、状态和关闭原因已逐项回读；12 个当前子任务及 15 条原生依赖均与迁移计划一致，开发总览已置顶。

## 当前执行顺序

[路径点导入 #16](https://github.com/IannTao/Paimon/issues/16) → [坐标定位 #17](https://github.com/IannTao/Paimon/issues/17) → [按点跑图 #18](https://github.com/IannTao/Paimon/issues/18) / [自动传送 #19](https://github.com/IannTao/Paimon/issues/19) → [到点拾取 #9](https://github.com/IannTao/Paimon/issues/9) → [完整闭环 #10](https://github.com/IannTao/Paimon/issues/10) → 后续扩展与验收。

GitHub 编号与迁移前本地编号不同。裸 issue 编号默认 GitHub；旧编号需明确为“本地 NN”或 local:NN。依赖和验证条件见具体 issue，关闭不自动表示真机验证通过。

## 编号映射

| 本地编号 | GitHub | 标题 | 迁移时处理 |
| --- | --- | --- | --- |
| 01 | [#2](https://github.com/IannTao/Paimon/issues/2) | 真机验证自动点击与玩家触控共存 | 已取代 / not planned |
| 02 | [#3](https://github.com/IannTao/Paimon/issues/3) | 跑通单个物品的自动拾取会话 | 已取代 / not planned |
| 03 | [#4](https://github.com/IannTao/Paimon/issues/4) | 覆盖连续拾取与混合交互场景 | 已取代 / not planned |
| 04 | [#5](https://github.com/IannTao/Paimon/issues/5) | 验证异常中断与用户恢复流程 | 已取代 / not planned |
| 05 | [#6](https://github.com/IannTao/Paimon/issues/6) | 提供用户主动开启的本地诊断 | 已取代 / not planned |
| 06 | [#7](https://github.com/IannTao/Paimon/issues/7) | 完成首版真机验收并交付 APK | 已取代 / not planned |
| 07 | [#8](https://github.com/IannTao/Paimon/issues/8) | 从应用启动并完成一次有画面反馈的自动移动 | 已完成 / completed |
| 08 | [#9](https://github.com/IannTao/Paimon/issues/9) | 自动调整视角并拾取近处单个物品 | 待办 / open |
| 09 | [#10](https://github.com/IannTao/Paimon/issues/10) | 从固定起点完成单资源点跑图采集闭环 | 待办 / open |
| 10 | [#11](https://github.com/IannTao/Paimon/issues/11) | 正确处理多个拾取提示和混合交互 | 待办 / open |
| 11 | [#12](https://github.com/IannTao/Paimon/issues/12) | 连续完成短路线上的多个资源点 | 待办 / open |
| 12 | [#13](https://github.com/IannTao/Paimon/issues/13) | 采集受阻时自动有限恢复并明确结束 | 待办 / open |
| 13 | [#14](https://github.com/IannTao/Paimon/issues/14) | 查看任务报告并主动开启本地诊断 | 待办 / open |
| 14 | [#15](https://github.com/IannTao/Paimon/issues/15) | 整合短路线真机验收并交付自用 APK | 待办 / open |
| 15 | [#16](https://github.com/IannTao/Paimon/issues/16) | 导入并预检 BetterGI 路径点原文件 | 待办 / open |
| 16 | [#17](https://github.com/IannTao/Paimon/issues/17) | 将手机屏幕定位对应到 BetterGI 游戏坐标 | 待办 / open |
| 17 | [#18](https://github.com/IannTao/Paimon/issues/18) | 按导入路线的地面路径点自动跑图 | 待办 / open |
| 18 | [#19](https://github.com/IannTao/Paimon/issues/19) | 自动完成导入路线要求的传送并重新定位 | 待办 / open |

## 迁移记录

本地原文件保留只读迁移快照和 GitHub 入口；原验收项、历史讨论与源 SHA-256 已随工单发布。01–06 以 not planned 关闭而非声称通过，07 以 completed 关闭，当前 08–18 为 open。五个默认分诊标签与生命周期分开维护。

机器可读映射见 [github-migration.json](github-migration.json)。后续操作遵循 [GitHub tracker 约定](../agents/issue-tracker.md)。规格和 ADR 仍在仓库文档中维护，GitHub 总览包含此次迁移的完整内容。
