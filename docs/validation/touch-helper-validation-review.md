# 工单 01 后续验证与状态修复审查

日期：2026-10-29。比较基点：`1258dc0`，审查 `git diff HEAD` 的后续工作区变更及新增回放测试、采样脚本。规格来源：`input-coexistence-research.md`、`touch-helper.md`、工单 01。按 code-review 技能由独立 Standards / Spec 两轴复审。

## Standards

确定代码问题或规范违规：0 项。无合成时完整键/槽不一致帧原样转发；合成已活动时先按旧真实状态结束合成，再转发源帧；不改写真实 tracking ID。不一致期间拒绝新合成，真正同步丢失、非法槽/ID、重复 ID 仍有健康检查。正常交接核查实时槽、键和内核队列。只读采样脚本按所属子进程清理。

验证缺口：需要修复版真机证据确认真实 pointerId 没有 CANCEL、意外 UP 或重新 DOWN；离线回放不能替代该证据。

## Spec

新增阻塞：0 项。修复符合状态不完整时停止新增合成、真实流保留转发的要求；恢复键/槽一致后才重新允许合成。正常交接不会因 MT 槽为空而忽略仍按下的 BTN_TOUCH。

验证边界：仅修复代理把完整 key-only 帧当作丢失状态而退出的条件，不等于证明该源帧在 MyOS 下的手势连续性。修复版真机复测、真实双指加合成第三触点以及游戏验收仍须以设备报告为准，见 [实测记录](touch-helper-device-validation.md)。

两轴确定问题均为 0；两轴均要求补修复版真机连续性证据，不能据此解除工单门槛。

## 后续证据复核

Spec 轴独立检查 `merge-third-1793280499257.jsonl` 全部 1128 个 MotionEvent、1801 条 history：deviceId=22，最多三指、0 CANCEL、未截断；MOVE/history 指针集合、动作转换、downTime 和时间顺序无异常。合成 pointer2 在中心持续 62ms，原 pointer0/1 未 UP、CANCEL 或重新 DOWN。仅支持该轮接收器“双真实触点持续按住 + 一次合成第三触点”连续性。

`forward-fixed` 两份分别为 309 个事件/326 条 history 和 1031 个事件/1142 条 history，deviceId=21，均 0 CANCEL、指针与时间检查无异常、末尾无活动触点。这些证据不覆盖游戏拾取、重复稳定性或异常恢复验收。
