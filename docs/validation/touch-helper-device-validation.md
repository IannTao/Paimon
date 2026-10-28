# 工单 01：单流代理阶段真机验证

日期：2026-10-28。基准设备实测为 NX712J、Android 14 / API 34、MyOS14.5.33_NX712J，shell UID 2000、SELinux `u:r:shell:s0`。构建指纹见 [环境记录](reports/touch-helper-2026-10-28/environment.txt)。没有修改物理输入节点权限、Root 或 SELinux 设置。

原 Shizuku shell tap 路径的失败结论保持有效。本页只记录候选代理的阶段证据，**工单 01 整体仍未验证，游戏验收及后续门槛尚未解除**。

## 已执行阶段

| 阶段 | 实测结果 | 证据 |
| --- | --- | --- |
| 只读能力 | goodix_ts 位于本轮 `/dev/input/event6`，10 个物理槽；X 0–17856，Y 0–39680，tracking ID 0–65535，DIRECT，含 BTN_TOOL_FINGER / BTN_TOUCH | [物理能力](reports/touch-helper-2026-10-28/physical-capabilities.txt)、[helper 输出](reports/touch-helper-2026-10-28/inspect-after-tool-finger.jsonl) |
| 空设备创建及销毁 | UI_SET_EVBIT、KEYBIT、ABSBIT、PROPBIT、UI_ABS_SETUP、UI_DEV_SETUP、CREATE 与 DESTROY 实际成功；没有发送触摸 | [创建输出](reports/touch-helper-2026-10-28/create-final-start.jsonl)、[正常清理](reports/touch-helper-2026-10-28/create-normal-cleanup.jsonl) |
| 竖屏识别 | Android 识别为 DIRECT / TOUCHSCREEN / MULTI_TOUCH，内置显示 0，1116×2480；虚拟槽最大 10，即 11 槽 | [dumpsys 摘录](reports/touch-helper-2026-10-28/input-created-excerpt.txt) |
| 手机本地 watchdog | create-only worker 被 STOP 暂停后，手机本地 supervisor 约 2 秒报告 HeartbeatExpired、回收 worker；harness 验证预期退出码 2，最终自身退出 0 | [最终故障注入记录](reports/touch-helper-2026-10-28/watchdog-final.jsonl)、[回收后 EventHub](reports/touch-helper-2026-10-28/event-hub-after-watchdog.txt) |
| 真正抓取 | 用户确认所有手指离屏后，执行 grab --seconds 3；EVIOCGRAB(1) 成功，随后正常 closed、退出 0。软期限使实际运行约 2 秒，未发送触摸 | [grab 原始输出](reports/touch-helper-2026-10-28/grab.jsonl) |
| 横屏识别 | 接收器固定 2480×1116、rotation=1；虚拟设备关联 INTERNAL display 0，公开 X/Y motion range 对应该横屏尺寸；实际触摸坐标仍需接收器比对 | [横屏映射](reports/touch-helper-2026-10-28/input-created-landscape.txt) |
| 物理触摸基线 | 第一轮 645 个事件，包含单指、双指，0 CANCEL，无截断，结束时 0 活跃触点；第二轮 381 个单指事件，同样 0 CANCEL | [第一轮](reports/touch-helper-2026-10-28/baseline-1793201525508.jsonl)、[第二轮](reports/touch-helper-2026-10-28/baseline-1793201549836.jsonl) |
| 首次纯转发 | 源虚拟设备 ID 14 的 166 个事件，含两组 DOWN/UP 与一组 POINTER_DOWN/UP，0 CANCEL，实际移动跨度约 2.45 秒。此为短时结构观察，尚非完整持续性/旋转/异常验证 | [接收器日志](reports/touch-helper-2026-10-28/forward-1793201558649.jsonl)、[helper 输出](reports/touch-helper-2026-10-28/forward-helper.jsonl) |

设备路径和 Android deviceId 都是本轮发现值，不作为通用配置硬编码。纯转发文件还含代理启动前物理设备 ID 6 的 188 个事件，不能把整份 354 个事件都算为代理结果。文件未截断、末尾 summary 为 volume_down，保存时触点为 0。

## 实测中修复的前置问题

1. 初版将普通 BTN_TOOL_FINGER 也作为不支持的工具计数键拒绝，inspect 在创建前返回 errno 95；[原始失败输出](reports/touch-helper-2026-10-28/inspect-before.jsonl)。补齐源键缓存、合成期间的单指计数语义及结束时恢复，并以失败测试先行验证；其它工具/多指计数按键仍拒绝。语义依据 [Linux 5.15 事件码文档](https://www.kernel.org/doc/html/v5.15/input/event-codes.html)，不把该参考文档等同本机驱动手势实测。
2. 初次空设备创建时，Android 实际公布 tracking ID 上限仍为 65535，而非请求的 65536。尚未发送触摸便发现此问题。现改为范围内空闲 ID，真实输入若随后碰撞该 ID，先结束合成，再原样转发真实帧；额外预留的是槽，不是超范围 ID。已通过冲突顺序回归测试。
3. 初版接收器在按音量减保存后同时停止绘制触点，用户据此报告“音量减之后触屏完全没有响应”。当时代理已正常关闭；用户按音量加后确认圆圈重新跟随手指，确认该保存后无反馈来自接收器交互。现已改为保存后仍实时显示触点。该观察不扩大为所有转发行为均已正确。

兼容修复提交为 `1ef6f90`；执行创建、grab、首次转发的 helper SHA-256 为 `2C35BDB7E553473E645538886B3F012382F9C74B63FA9EA355AE5CAE343F98D6`。后续增加真实触点计数诊断时会重新记录二进制校验值。

## 接收器与复现

独立模块 `android/touch-receiver`，包名 `dev.paimon.touchreceiver`，无需新增权限。构建命令为 `gradle -p android :touch-receiver:assembleDebug :touch-receiver:lintDebug`；输出 `android/touch-receiver/build/outputs/apk/debug/touch-receiver-debug.apk`。已构建、安装并在基准手机打开；Lint 0 错误、5 个警告（API/横屏/图标/绘制分配等验证工具限制）。

接收器只记录其窗口收到的 MotionEvent：action/index、pointerId、坐标、工具类型、压力/尺寸、history、downTime、eventTime、接收 uptime、设备 ID 与来源。最多缓存 12000 条记录，超限明确标记 truncated；保存时写入应用私有文件。音量减在所有手指抬起后保存，音量加新开一轮；保存后保留实时触点预览。离开 Activity 会以 activity_paused 保存，不把这种中断轮当成正常完成。

报告中的 `maxAppDispatchDelayMs` 包含 Android 批处理和应用调度，不等于代理新增延迟，不能以它宣称触控延迟已达标。崩溃/被杀前未保存的内存记录可能丢失，不能把缺失报告算为通过。使用 `adb shell run-as dev.paimon.touchreceiver ls files` 查找文件，再以单独 `adb exec-out run-as ... cat files/<name>` 归档。

下一步在接收器里验证同流第三触点，检查原有两指没有 CANCEL / UP / 重新 DOWN、中央合成触点坐标正确；再补纯转发持续、旋转/锁屏和异常恢复实验，最后才进入游戏实际拾取与原工单重复门槛。尚未进行任何游戏合成点击。
