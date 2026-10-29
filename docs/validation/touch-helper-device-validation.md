# 工单 01：单流代理阶段真机验证

日期：2026-10-28 至 2026-10-29；同一轮证据沿用起始日期目录。基准设备实测为 NX712J、Android 14 / API 34、MyOS14.5.33_NX712J，shell UID 2000、SELinux `u:r:shell:s0`。构建指纹见 [环境记录](reports/touch-helper-2026-10-28/environment.txt)。没有修改物理输入节点权限、Root 或 SELinux 设置。

原 Shizuku shell tap 路径的失败结论保持有效。本页只记录候选代理的阶段证据，**工单 01 整体仍未验证，游戏验收及后续门槛尚未解除**。

最新进展：已捕获触发 `touch key and MT slots disagree` 的完整帧（仅 BTN_TOUCH=1 + SYN_REPORT），通过失败回归修复代理主动退出问题，修复版纯转发复测 1340 个事件、0 CANCEL。三指系统分屏手势已由用户关闭，恢复了 2480×1116 全屏；现已完成一次真实双指持续按住加合成第三触点，原指针保持连续、0 CANCEL。双指持续拖动、重复及异常门槛、游戏验收尚待执行。

当前新阻塞：后续双指拖动实验在最终抬手阶段出现 1 CANCEL，虽晚于第三触点结束约 13.7 秒，整轮仍未通过。正在区分正常关闭交接时机、系统手势和物理源流问题；不把用户主观连续报告代替日志判定。

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
| 首次合成 | 虚拟设备 15 共 7068 个事件，0 CANCEL，但最多 2 指。中心合成位于 (1239.9688,557.96875)，持续约 62ms；该瞬间仅有一根真实手指，所以只能支持 1 真实 + 1 合成的共存观察，不能算第三指通过 | [接收器日志](reports/touch-helper-2026-10-28/merge-1793202894099.jsonl)、[helper 输出](reports/touch-helper-2026-10-28/merge-first-helper.jsonl) |
| 后续状态错误 | 未发送任何点击便出现 touch key / MT 状态校验错误，退出码 1；销毁输出已归档。实时计数里出现的 3 是真实物理触点，不是合成第三指 | [错误输出](reports/touch-helper-2026-10-28/merge-retry-state-error.jsonl) |
| 双真实触点加第三合成触点 | 修复后使用 tap-if 2，仅一次中心合成，原 pointer0/1 全程保留，合成 pointer2 持续 62ms，0 CANCEL；单次接收器按住实验成立，尚不代表游戏/拖动/重复验收 | [接收器日志](reports/touch-helper-2026-10-28/merge-third-1793280499257.jsonl)、[helper 输出](reports/touch-helper-2026-10-28/merge-third-helper.jsonl) |

设备路径和 Android deviceId 都是本轮发现值，不作为通用配置硬编码。纯转发文件还含代理启动前物理设备 ID 6 的 188 个事件，不能把整份 354 个事件都算为代理结果。文件未截断、末尾 summary 为 volume_down，保存时触点为 0。

## 实测中修复的前置问题

1. 初版将普通 BTN_TOOL_FINGER 也作为不支持的工具计数键拒绝，inspect 在创建前返回 errno 95；[原始失败输出](reports/touch-helper-2026-10-28/inspect-before.jsonl)。补齐源键缓存、合成期间的单指计数语义及结束时恢复，并以失败测试先行验证；其它工具/多指计数按键仍拒绝。语义依据 [Linux 5.15 事件码文档](https://www.kernel.org/doc/html/v5.15/input/event-codes.html)，不把该参考文档等同本机驱动手势实测。
2. 初次空设备创建时，Android 实际公布 tracking ID 上限仍为 65535，而非请求的 65536。尚未发送触摸便发现此问题。现改为范围内空闲 ID，真实输入若随后碰撞该 ID，先结束合成，再原样转发真实帧；额外预留的是槽，不是超范围 ID。已通过冲突顺序回归测试。
3. 初版接收器在按音量减保存后同时停止绘制触点，用户据此报告“音量减之后触屏完全没有响应”。当时代理已正常关闭；用户按音量加后确认圆圈重新跟随手指，确认该保存后无反馈来自接收器交互。现已改为保存后仍实时显示触点。该观察不扩大为所有转发行为均已正确。

首次合成的真实 pointer0 在 43780748ms 先 UP，中心合成 pointer0 在 43781735–43781797ms 出现，另一个真实 pointer1 保持同一 downTime 43756555ms；新真实 pointer0 到 43782489ms 才重新 DOWN。用户确认本轮保存后触控正常。为避免电脑观察与命令到达之间状态改变，已增加 `tap-if 2 X Y` 条件命令，经过先失败再通过的事件流和命令解析测试；它尚未在真机产生一次成功第三指实验。

状态错误的待排查方向依次为：驱动 BTN_TOUCH 与 MT 槽的合法暂态差异、接管/槽选择状态维护遗漏、事件片段不完整。尚无故障时物理帧，不能定论是哪一个。两次限时只读采样均未取得可用事件；空文件不计为输入证据，也不代表设备拒绝读取。未直接放宽一致性校验来掩盖失败。

2026-10-29 补充诊断：修正前台检测为 `dumpsys window displays`，通过只读脚本在接收器前台采得 [物理原始事件](reports/touch-helper-2026-10-28/physical-raw-20261029.txt)。脚本不抓取设备，失去接收器焦点或达到十分钟即停止；本轮用户完成后主动终止。新增 `cargo test -p paimon-touch-helper --test physical_replay` 完整帧回放，结果通过，未复现上述故障。随后安装仅在错误出口增加帧/缓存状态日志的诊断版，执行纯转发：[接收器记录](reports/touch-helper-2026-10-28/forward-diagnostic-1793279391855.jsonl) 共 533 个 MotionEvent，全部来自虚拟设备 17，27 组 DOWN/UP、6 组 POINTER_DOWN/UP、467 个 MOVE，最多双指，0 CANCEL、无截断、结束时 0 活跃触点。用户确认触控正常，[helper](reports/touch-helper-2026-10-28/forward-diagnostic-helper.jsonl) 正常销毁并退出 0，无状态错误。此轮未发送合成点击，不能算第三触点通过；根因仍未确定。

兼容修复提交为 `1ef6f90`；执行创建、grab、首次转发的 helper SHA-256 为 `2C35BDB7E553473E645538886B3F012382F9C74B63FA9EA355AE5CAE343F98D6`。后续增加真实触点计数诊断时会重新记录二进制校验值。

首次合成使用的计数诊断版 helper SHA-256 为 `89D5E2D10DA7B4B14FFB20BEC9D4E83CCC1CA791B91BF7DEA803CC45AAB1147E`；含条件命令、出现状态错误的后续版本为 `0B1AFB5F5A8F4CA2CDC4A4564EE7FA1C01C6B988B114942879E6BF3826F20DAC`。当前接收器 APK SHA-256 为 `EDC1DA09193AA1F8E37A3D2C55E60C0CCD3EE23F248CE93D0B91FDFD13D62238`；已经安装保存后预览及新轮起点检查修复。

同日继续双指反复按下/拖动/交替抬起：[接收器记录](reports/touch-helper-2026-10-28/forward-stress-1793279661201.jsonl) 为虚拟设备 18 的 1393 个事件，实际事件时间跨度 23.328 秒，46 组 DOWN/UP、47 组 POINTER_DOWN/UP、1207 个 MOVE，最多双指、0 CANCEL、无截断、保存时 0 活跃触点；用户确认触控正常。[helper](reports/touch-helper-2026-10-28/forward-stress-helper.jsonl) 正常销毁并退出 0，未复现状态错误。提示操作时长约 30 秒不等于实际持续触摸达到 30 秒。

随后仅把物理手指数增加到三指，仍未发送合成点击。[helper](reports/touch-helper-2026-10-28/forward-three-helper.jsonl) 未触发状态错误并正常退出，用户报告触控正常，但接收器记录显示窗口/手势中断，不能算连续性通过：[第一份](reports/touch-helper-2026-10-28/forward-three-1793279808039.jsonl) 46 个事件、5 CANCEL，以 activity_paused 保存，窗口 2480×1116；[第二份](reports/touch-helper-2026-10-28/forward-three-1793279860563.jsonl) 951 个事件、3 CANCEL，以 volume_down 保存，窗口变为 1234×1116。两份均来自虚拟设备 19、最多三指、无截断、保存时 0 活跃触点。用户随后确认出现了分屏或窗口缩小，与上述取消及尺寸变化相吻合；这不能证明此前 touch key / MT 状态错误由分屏造成。宽度改变后不再沿用此前全屏映射断言，也不继续合成实验。需先关闭三指分屏手势并重新核对全屏映射；2026-10-29 本轮已请求用户在手机设置操作，尚待完成。

该轮原始采样包含 1182 个完整帧；诊断版 helper SHA-256 为 `832309595EF4DB8009AFA26DF31BD624DC61E6F26D2DA6288F0C1D1318AACC93`。

用户关闭三指分屏并恢复全屏后，下一次纯转发启动即复现了状态错误：[故障帧与缓存状态](reports/touch-helper-2026-10-28/key-only-failure.txt) 显示，先有真实触点进入并退出，随后收到完整的 `EV_KEY/BTN_TOUCH=1, SYN_REPORT` 帧，所有 MT 槽为空、无活动合成。此次错误来自把 key==any_MT 当成每帧必须成立的条件，尚不能据此推断驱动为何发出该键帧。最小回放 `cargo test -p paimon-touch-helper --test stream captured_key_only` 在修复前因相同报错失败，修复后通过：源帧保持原样转发，键/槽不一致时禁止新合成，恢复一致后才允许；已有合成遇不一致先结束，再转发源帧。另一回归 `key_slot_disagreement` 验证释放顺序，先失败再通过。真正的同步丢失、非法槽/ID等仍禁止后续合成并退出，正常退出仍要求实际槽/键/队列均空闲。

修复后全部 30 项主机测试及严格 Clippy、Android helper 构建通过；已移除代码中的临时 `[DEBUG-touch-state]` 日志，保留上述历史证据。修复版 SHA-256 为 `4B283DD9A4DDCB5874B34B88C76C4B74DA1629764AA3DAA05F4EE73B5A596B9D`。离线回放只证明已修复代理的退出条件，不能证明系统收到这种源帧时真实手势一定连续，仍需修复版真机接收器复测。

修复版真机复测已执行，helper 正常销毁并退出 0，[输出](reports/touch-helper-2026-10-28/forward-fixed-helper.jsonl) 无错误或合成点击。用户保存了两段：[第一段](reports/touch-helper-2026-10-28/forward-fixed-1793280300668.jsonl) 309 个事件、跨度 8.729 秒、最多 4 个真实触点；[第二段](reports/touch-helper-2026-10-28/forward-fixed-1793280362605.jsonl) 1031 个事件、跨度 21.033 秒、最多 3 个真实触点。均来自虚拟设备 21，0 CANCEL、无截断、volume_down 保存、结束时 0 活跃触点、2480×1116 横屏。用户确认触控正常且未分屏。此前分屏手势开启时的 8 CANCEL 不被这轮结果抹去。该轮没有故障帧实时计数，不能声称现场再次出现过同一种 key-only 帧；该精确帧由离线回归覆盖，当前设备结果支持修复版正常多指纯转发。审查见 [独立双轴复审](touch-helper-validation-review.md)。

恢复合成后，`tap-if 2 8928 19840` 在接收器前台且代理最新完整帧确认两个真实触点时执行一次。[接收器](reports/touch-helper-2026-10-28/merge-third-1793280499257.jsonl) 1128 个事件、全部虚拟设备 22、maxPointers=3、0 CANCEL、无截断、volume_down 保存、最终无活动触点。真实 pointer0 在 121349250ms DOWN，pointer1 在 121349265ms 加入；合成 pointer2 在 121362722–121362784ms 出现于 (1239.9688,557.96875)，持续 62ms；原 pointer0/1 保持 downTime=121349250，直到 121374699ms、121374736ms 才分别抬起。用户确认未见分屏、中断或重新按下；许可到期后代理继续排空真实手势，全部抬起后正常销毁、退出 0。本轮支持一次双真实按住加第三合成触点的连续性观察，不能替代双指持续拖动或游戏拾取。

随后要求双指持续拖动并执行一次相同中心条件点击。[记录](reports/touch-helper-2026-10-28/merge-moving-1793280640800.jsonl) 共 3353 个事件，虚拟设备 23、最多三指、无截断、volume_down 保存、末尾 0 活跃触点、2480×1116。真实 pointer0/1 从 121502246/121502257ms 开始；合成 pointer2 在 121520226–121520288ms 存在 62ms，原两指保持同一 downTime。但原 pointer0 在 121534008ms UP 后，最后 pointer1 在 121534028ms 收到 CANCEL(flags=32)，没有最终 UP。用户报告拖动连续，然而整轮包含 1 CANCEL，判为未通过。[helper](reports/touch-helper-2026-10-28/merge-moving-helper.jsonl) 已正常退出 0，故退出码不能替代连续性检查。合成存续期只有 POINTER_DOWN/UP 两个事件、原两指坐标相同，不能据此额外声称该 62ms 内有持续移动样本。

待验证的收尾取消原因依次为：虚拟设备销毁先于 Android 处理最终抬指、系统手势、物理源流条件。针对性读取 InputReader/InputDispatcher/WindowManager 日志未取得可用输出，不能用空日志排除任何原因。下一轮先不注入、保存后才发送 stop，以对照退出时机；尚未据此修改收尾逻辑。

收尾对照：[双指纯转发](reports/touch-helper-2026-10-28/forward-manual-close-1793280823869.jsonl) 1457 个事件、deviceId=24、0 CANCEL，原指针按 POINTER_UP、UP 正常结束，保存后才发送 stop，[helper](reports/touch-helper-2026-10-28/forward-manual-close-helper.jsonl) 正常退出。随后无人触摸时，在接收器内执行三次独立的 `permit + tap-if 0 + stop` 快速收尾实验，[日志](reports/touch-helper-2026-10-28/drain-auto-1793281008452.jsonl) 为 deviceId=25/26/27 各一组 DOWN/UP、共 6 事件、0 CANCEL，[三次 helper](reports/touch-helper-2026-10-28/drain-auto-helper.jsonl) 均退出 0。三次点击因紧跟 stop 提前结束，不能当作正常 50ms 点击或游戏通过，只用于销毁时机诊断。本组未复现，不能确认立即销毁即为根因，故未添加延时掩盖问题。后续 dumpsys input 显示内置 UnwantedInteractionBlocker 的 mEnablePalmRejection=0、flag=0、mPalmRejectors=None；这不能排除厂商系统手势或驱动行为。

## 接收器与复现

独立模块 `android/touch-receiver`，包名 `dev.paimon.touchreceiver`，无需新增权限。构建命令为 `gradle -p android :touch-receiver:assembleDebug :touch-receiver:lintDebug`；输出 `android/touch-receiver/build/outputs/apk/debug/touch-receiver-debug.apk`。已构建、安装并在基准手机打开；Lint 0 错误、5 个警告（API/横屏/图标/绘制分配等验证工具限制）。

接收器只记录其窗口收到的 MotionEvent：action/index、pointerId、坐标、工具类型、压力/尺寸、history、downTime、eventTime、接收 uptime、设备 ID 与来源。最多缓存 12000 条记录，超限明确标记 truncated；保存时写入应用私有文件。音量减在所有手指抬起后保存，音量加新开一轮；保存后保留实时触点预览。离开 Activity 会以 activity_paused 保存，不把这种中断轮当成正常完成。

报告中的 `maxAppDispatchDelayMs` 包含 Android 批处理和应用调度，不等于代理新增延迟，不能以它宣称触控延迟已达标。崩溃/被杀前未保存的内存记录可能丢失，不能把缺失报告算为通过。使用 `adb shell run-as dev.paimon.touchreceiver ls files` 查找文件，再以单独 `adb exec-out run-as ... cat files/<name>` 归档。

下一步补双指持续拖动时的第三触点、重复实验及旋转/锁屏/异常恢复，最后进入游戏实际拾取与原工单重复门槛。尚未进行任何游戏合成点击。
