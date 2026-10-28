# 触控共存失败后的输入方案研究

研究日期：2026-10-27。适用基准为 nubia Z50 Ultra / NX712J、Android 14、MyOS 14.5.33。本文区分已测事实、源码事实与待验证推断；不将某个权限可用写成触控共存已经通过。

## 结论与推荐顺序

最值得制作下一个最小原型的是：**读取并独占物理触屏的事件流，将全部真实触点转发到一个 uinput 虚拟触屏，再在这个同一事件流中加入拾取触点。** 这样尝试让 Android 只接收一个来源、同一组多点触摸状态，而非在真实摇杆手势中另外发起独立点击。它仍使用同机系统输入，不涉及游戏内存或文件，与现有 ADR 边界不冲突；但本次只完成权限探测和设计研究，**没有实际创建虚拟触屏、独占触屏或验证转发**。

不建议把“直接调用 injectInputEvent”“换个 deviceId”“鼠标来源”“创建独立虚拟触屏并同时保留物理流”当作已经解决问题。它们都没有自动把真实触点加入合成事件。直接写入物理 evdev 的路线，在这台设备上已经被实际写权限检查排除。Shizuku 提供 shell 身份执行 Java/JNI 的能力，不改变系统的触摸事件语义。[Shizuku API 官方说明](https://github.com/RikkaApps/Shizuku-API/blob/master/README.md)

## 已有实测证据

1. 现路径使用 Shizuku UID 2000 执行 `input touchscreen tap`。完整 10 次运行命令全部成功返回，用户确认取得物品，但持续摇杆被打断，需要重新按下，因此当前路径失败。短轮次偶然没有中断不抵消失败。[原始运行报告](reports/ticket-01/92f12e21-4da2-4964-b378-16c58cab7fcf.json)、[人工观察](reports/ticket-01/92f12e21-4da2-4964-b378-16c58cab7fcf.observation.json)
2. 本次只读检查识别 `goodix_ts` 为当前物理触屏，Android InputDevice ID 为 6，evdev 节点为 `/dev/input/event6`；支持 MT 槽 0–9，原始 X 范围 0–17856、Y 范围 0–39680。节点名、ID 和范围属于这次设备检查，不能硬编码为通用设备规则。
3. native 能力探针以 UID 2000 运行：物理触屏 O_RDWR 打开失败，errno=13；O_RDONLY 成功，EVIOCGVERSION 成功；`/dev/uinput` O_RDWR 成功，UI_GET_VERSION 返回 5。[探针结果](reports/input-capabilities-2026-10-27.json)、[探针源码](../../.scratch/android-auto-pickup/input-capabilities.c)
4. 对一个从未抓取过设备的只读 fd 调用 `EVIOCGRAB`，参数仅为 **0（解除本 fd 自己不存在的抓取）**，返回 EINVAL=22。该结果与内核 ungrab 对非拥有者返回 EINVAL 的分支吻合，说明这次调用并未在此前被直接拒绝为 EACCES/EPERM。它不是 `EVIOCGRAB(1)` 成功证据；探针明确记录 `grabbedDevice=false`、`createdDevice=false`、`wroteInputEvents=false`。[探针结果](reports/input-capabilities-2026-10-27.json)、[Linux v5.15 evdev_grab / evdev_ungrab 源码](https://github.com/torvalds/linux/blob/v5.15/drivers/input/evdev.c#L320-L347)
5. 本次读取 `dumpsys package com.android.shell`，确认 INJECT_EVENTS、MANAGE_ACTIVITY_TASKS、CREATE_VIRTUAL_DEVICE、ASSOCIATE_INPUT_DEVICE_TO_DISPLAY 为 granted；输出未列出 FILTER_EVENTS / MONITOR_INPUT，`shizuku_server` 进程标签为 `u:r:shell:s0`。拥有前述权限不代表已拥有任意输入过滤、全局监控或私有系统服务能力；InputFilter / monitor 路线的相应调用权限尚未证实，不列为首选。

SELinux Enforcing 下，即使 DAC 的 input 组看起来可写，也不能绕过 SELinux。AOSP Android 14 QPR3 的 shell 策略允许读取 input_device，并明确禁止 shell 的 input_device 写权限；本机 O_RDWR 实测失败与其一致。这里引用的是 AOSP 参考策略，不是对 MyOS 实际完整策略的反编译证明。[AOSP shell.te](https://android.googlesource.com/platform/system/sepolicy/+/refs/heads/android14-qpr3-s9-release/public/shell.te#234)

## 为什么单独发起点击不能等同新增一根手指

已核验 AOSP 官方 GitHub 镜像的固定标签 `android-14.0.0_r1`：`InputShellCommand.sendTap()` 构造独立 ACTION_DOWN / ACTION_UP；`injectMotionEvent()` 固定 pointerCount=1、pointerId=0，然后调用 `InputManagerGlobal.injectInputEvent()`。它并没有读取并携带用户正在按住的摇杆或视角触点。[Android 14 InputShellCommand](https://github.com/aosp-mirror/platform_frameworks_base/blob/android-14.0.0_r1/services/core/java/com/android/server/input/InputShellCommand.java#L145-L170)、[sendTap](https://github.com/aosp-mirror/platform_frameworks_base/blob/android-14.0.0_r1/services/core/java/com/android/server/input/InputShellCommand.java#L347-L356)

一个容易误判的细节：该版本会通过 `getInputDeviceId(source)` 找第一个支持来源的设备，找不到才回退到 0；因此不能笼统说 input tap 总使用 deviceId=0，更不能声称“改成物理设备 ID 就能修复”。即使 ID 相同，完整多指状态仍然缺失。[同一固定版本的设备选择逻辑](https://github.com/aosp-mirror/platform_frameworks_base/blob/android-14.0.0_r1/services/core/java/com/android/server/input/InputShellCommand.java#L106-L114)

由独立 DOWN/UP 与实际中断现象，可以提出“注入流与既有触摸手势冲突”的机制解释，但本次没有采集到游戏收到的 ACTION_CANCEL 或完整系统输入跟踪，不能将具体取消分支写成已直接观测到的事实。Android 14 固定标签的 native InputDispatcher.cpp 在本次检索中无法成功取得；没有拿 main、Android 15/16 或旧 Android 源码替代其精确行为证明。后续如尝试并行设备来源，必须先补该版本机制证据与真机分发实验。

scrcpy v2.4 的多指注入实现维护自己的 PointersState，并对第二个及之后的触点构造 ACTION_POINTER_DOWN / ACTION_POINTER_UP 和完整指针数组。它说明“自行管理的一组触点”如何形成多指事件，不能证明它会自动合并手机用户的物理触点；其状态来自该控制通道的事件。[scrcpy Controller](https://github.com/Genymobile/scrcpy/blob/v2.4/server/src/main/java/com/genymobile/scrcpy/Controller.java#L240-L281)、[PointersState](https://github.com/Genymobile/scrcpy/blob/v2.4/server/src/main/java/com/genymobile/scrcpy/PointersState.java)

## 候选路径比较

| 路径 | 一手依据与当前判断 | 下一步 |
| --- | --- | --- |
| 只把 shell 命令改成直接 injectInputEvent | Android 14 的 input 本来已调用该 API；独立单点 DOWN/UP 的问题不会因删掉命令行外壳自动消失。 | 不优先作为修复；仅能减少进程启动开销。 |
| 仅换 deviceId、pointerId 或来源 | 没有维护真实触点数组和后续状态；不能等同多指合流。换鼠标还可能改变游戏接收路径。 | 若要试，先做隔离分发实验；不可直接宣称可共存。 |
| 无障碍 dispatchGesture | 官方 API 明确会取消正在进行的用户或服务手势。 | 与不打断真实摇杆目标冲突，不作为替代方案。[官方 API](https://developer.android.com/reference/android/accessibilityservice/AccessibilityService#dispatchGesture(android.accessibilityservice.GestureDescription,%20android.accessibilityservice.AccessibilityService.GestureResultCallback,%20android.os.Handler)) |
| 无障碍节点 ACTION_CLICK | 需要目标实际暴露可点击节点；不是按坐标给任意游戏表面新增触点的通用能力。现应用未读取游戏节点，未验证原神暴露拾取节点。 | 可做只读节点可见性调查，但不先扩大权限或假设存在节点。 |
| 同一物理 evdev 的空闲 MT 槽写入 | Linux Type B 支持独立槽；理论上可由 InputReader 看作同一设备的附加触点，但本机 evdev O_RDWR 被拒绝。 | 在当前权限边界下排除，不尝试 chmod、关闭 SELinux 或 Root。 |
| 独立 uinput / UHID 虚拟触屏并行运行 | uinput 可从用户空间创建设备；新设备并不自动与物理设备成为同一触点状态。本机仅证明 uinput 可打开和版本查询。 | 不把可创建设备等同共存；优先改为下述完整单流代理。[Linux uinput 文档](https://www.kernel.org/doc/html/v5.15/input/uinput.html) |
| 只读抓取物理流 → 单一 uinput 转发与合流 | 不需写物理 evdev；输入系统只接收代理的一组真实和合成触点，是当前最有依据的候选。 | 分阶段验证 GRAB、CREATE、设备识别、纯转发与合成触点；全部仍未验证。 |
| OEM 游戏助手自身的触控映射能力 | 本次仅看到努比亚浮窗影响前台事件记录，没有核实公开的映射 API 或授权接口。 | 不依赖未知私有服务，不据 OEM 内部能力推断普通 shell 可调用。 |

## 单流代理的实现约束

建议让 Shizuku 启动的 native helper 同时持有物理 evdev 只读 fd 与 uinput fd：物理事件只被代理接收，代理维护完整 MT 状态、原样转发真实触点，并在自己的虚拟设备中增加一个合成触点。Linux evdev 的抓取机制会把事件交给拥有者；其 ioctl 分支本身没有要求 fd 是 O_RDWR，但内核、SELinux 及设备当前是否已被抓取仍可拒绝请求。[Linux v5.15 evdev.c](https://github.com/torvalds/linux/blob/v5.15/drivers/input/evdev.c#L270-L347)、[ioctl 分支](https://github.com/torvalds/linux/blob/v5.15/drivers/input/evdev.c#L1024-L1028)

Type B 协议用 ABS_MT_SLOT 选择触点，ABS_MT_TRACKING_ID 非负值表示触点、-1 表示移除，SYN_REPORT 分隔帧。代理必须保留真实 tracking ID 与各槽状态，在确认空闲的槽中分配合成触点，并序列化写帧，不能让两个写线程相互穿插。真实设备槽已全部占用、状态不完整或遇 SYN_DROPPED 时，应停止新增合成点击，不猜测覆盖真实槽。[Linux MT 协议](https://www.kernel.org/doc/html/v5.15/input/multi-touch-protocol.html)

虚拟设备不能只设置 X/Y：应核对真实设备的触摸属性、必要按钮、轴范围、分辨率与坐标变换；INPUT_PROP_DIRECT 是 Android 将设备识别为触屏的重要输入。还必须通过实际 `dumpsys input` 确认关联的是内置显示，横屏旋转和缩放正确，不能因为创建设备成功就直接在游戏中点击。[AOSP 触摸设备分类与校准](https://source.android.com/docs/core/interaction/input/touch-devices)

启动/退出属于输入来源切换：原型应在**开始代理前所有手指已抬起**时接管，运行期间持续转发，不能在每次拾取时要求松开摇杆。正常退出也应在真实触点全部抬起时释放；这不是“松手再拾取”的运行降级。若产品要求任何时刻退出都无缝保留仍按住的物理手势，这一点还没有解决证明。

关闭 evdev fd 会释放该 fd 的 grab；关闭 uinput fd 会销毁其虚拟设备。它们有助于不遗留独占与虚拟设备，但**不保证退出时仍按住的手指能无缝恢复到原始输入流**。进程卡死而 fd 未关闭也不等同退出，需独立超时 watchdog、明确的可用恢复入口，并保证真实触摸转发优先于识别任务。[evdev_release](https://github.com/torvalds/linux/blob/v5.15/drivers/input/evdev.c#L409-L430)、[uinput_release](https://github.com/torvalds/linux/blob/v5.15/drivers/input/misc/uinput.c#L668-L675)

## 建议的最小验证顺序

这些是后续实验设计，本文没有执行抓取或创建设备。

1. **先交付可审查的限时 helper。** 单一受控进程持有两个 fd，独立 watchdog 也运行在手机上，不以电脑承担日常恢复职责。识别、前台或权限检查失败时停止新增合成点击；转发仍健康时继续保留真实手指流，待触点全部抬起后正常交接。转发自身失效、进程卡死或达到实验硬期限时，由 watchdog 终止持有 fd 的进程并释放资源，此类异常恢复不能保证仍按住的手势连续。读取现有能力与完整 MT 状态，不硬编码 event6 或 Android deviceId=6。
2. **只创建、不发送触摸。** 验证 UI_SET_EVBIT、UI_SET_ABSBIT、UI_ABS_SETUP、UI_SET_PROPBIT、UI_DEV_SETUP、UI_DEV_CREATE 与 UI_DEV_DESTROY 的实际权限；确认系统正确识别虚拟触屏及显示关联，再销毁。UI_GET_VERSION 成功不能替代这些测试。[uinput 接口与创建流程](https://www.kernel.org/doc/html/v5.15/input/uinput.html)
3. **限时验证真正 EVIOCGRAB(1)。** 必须是在明确的受控实验中，不能把此前 EVIOCGRAB(0) 返回 EINVAL 当作成功。先保证所有手指离屏、watchdog 已生效。若返回 EACCES/EPERM/EBUSY，记录真实错误，不改系统权限去硬闯。
4. **只转发，不合成。** 在独立触摸接收器中验证单指、双指、快速移动、抬指、旋转/锁屏和异常退出；观察真实 tracking ID、DOWN/MOVE/POINTER_UP 序列及延迟。不读取或改写游戏数据。
5. **同一虚拟流加入第三触点。** 先验证原有两指全程没有 CANCEL/UP/重新 DOWN，再在游戏中验证拾取是否有效。接收器结果只能验证输入结构，不能代替原神的实际拾取与触控连续性。
6. **恢复工单 01 原门槛。** 在基准手机、固定显示配置中重复持续摇杆及摇杆加视角测试，归档系统事件和人工结果。全程不打断且实际取得物品，才可讨论解除后续阻塞；现 shell tap 的失败记录保持有效。

## 尚未解决的问题

- MyOS 是否允许真正 grab、uinput 配置/创建设备和持久转发，尚未验证；本次只探测了有限 ioctl。
- Android 14 native InputDispatcher 固定源码的具体多设备冲突分支未成功读取，不能把其他版本的 multi-device 或 mirror-touch 特性套用到本机。
- 物理触屏被独占后，MyOS 游戏助手、屏幕手势、锁屏、掌压过滤和系统其他输入消费者是否受到影响，待实验。
- 原始事件采集、代理帧缓存和虚拟触屏引入的帧率、功耗、温度与触摸延迟未测量。
- 正常停用时如何在仍按住手指的情况下切回物理来源，以及卡死后的用户恢复体验，不能仅凭 fd 清理语义宣称已解决。

此候选的价值在于已有权限证据与明确的数据流设计，并非已证明可用。若 GRAB 或 uinput 创建被设备策略拒绝，在现有无 Root 边界内暂时没有已经证实的替代路径，应保留“未解决”结论并重新讨论边界，不把实现愿望写为通过。
