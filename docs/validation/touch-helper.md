# 限时单流触屏代理 helper

实现日期：2026-10-28。对应 [输入方案研究](input-coexistence-research.md) 的最小验证顺序，交付独立 native helper，尚未接入现有 APK / Shizuku UserService。现有 shell tap 路径及失败证据保持不变，**本实现未执行真机创建、抓取、转发或游戏拾取验证**。

## 构建与自动验证

Rust 核心无第三方依赖；Android Linux UAPI 由 NDK 编译的小型 C 适配层提供，不在 Rust 中手写 ioctl 数字或 `input_event` 时间字段布局。仅支持 `aarch64-linux-android`、Android API 34+、shell UID 2000，拒绝 Root 身份。

```powershell
./scripts/build-touch-helper.ps1 -Ndk C:/path/to/android-sdk/ndk/27.0.12077973
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
```

输出：`target/aarch64-linux-android/release/paimon-touch-helper`。单独检查可用 `./scripts/build-touch-helper.ps1 -CheckOnly`。Windows 主机只能运行纯 Rust 测试和 CLI 参数校验，不能执行设备操作。watchdog 测试中的两个 ignored 测试是被父测试显式启动并终止的子进程夹具，不是遗漏的设备验证。

## 进程与恢复

公开入口始终先启动手机本地监护进程，再启动持有 evdev/uinput fd 的 worker。监护进程不持有这些 fd；worker 每 100ms 报送心跳，超过 2 秒没有心跳或达到 `--seconds` 硬期限，监护进程直接终止并回收 worker。worker 设置 `PR_SET_PDEATHSIG=SIGKILL`，并复查父 PID，监护进程异常退出也会触发资源释放。`--worker` 是内部入口，不应直接调用。

`stop`、stdin EOF、信号、许可过期都停止新增合成触点，保留健康的真实输入转发，直到所有真实触点抬起再交接。正常停用前会主动释放尚存的合成触点。开始进入转发后，软停止时间为该阶段开始后的 `seconds - 1` 秒；启动开销计入监护进程的硬期限，无法保证一定有一整秒交接窗口。持指不放会最终触发硬期限强制恢复。

遇设备断开、`SYN_DROPPED`、非法/不完整状态、uinput 写失败或帧超过 250ms 未完成时，worker 退出并释放 fd；不猜测或继续发送未知状态。单帧最多 4096 个事件。以上异常恢复、达到硬期限以及进程被杀时，**不能保证仍按住的手势连续**。手机上的可用恢复入口是向仍连接的控制通道发送 `stop`；控制通道丢失会自行排空，卡死则由本地 watchdog 恢复，最长 120 秒，不依赖电脑维持恢复能力。当前没有 APK 内的手动恢复按钮。

## 分阶段入口

以下命令均在手机 shell UID 2000 下执行；helper 不会自动推送、启动或更改系统权限。示例中的 `eventN` 必须替换为本轮只读发现并核实的设备，不使用研究报告里的旧节点号。每个阶段都需要单独运行，不能直接从能力探测跳到游戏合成点击。

1. `paimon-touch-helper inspect`：只读枚举支持的直接 Type B 触屏，输出路径、名称、物理槽数、坐标范围、tracking ID 范围，以及属性、按键、轴范围和分辨率。也可加 `--device /dev/input/eventN` 精确检查。不会打开 uinput 或独占设备。
2. `paimon-touch-helper create --device /dev/input/eventN --seconds 20`：配置、创建空虚拟触屏；不 grab，不写任何触摸。独立运行 `dumpsys input` 记录 `Paimon proxy touchscreen` 的设备分类、内置显示关联、方向和缩放，然后发送 `stop`，确认 `destroyed` 和 `closed`。`syscall_error` 保留具体 ioctl 与真实 errno；`created` 不等于显示验证通过。
3. `paimon-touch-helper grab --device /dev/input/eventN --seconds 3`：不创建虚拟设备，仅在双重空闲核对后执行真正的 `EVIOCGRAB(1)`。此阶段会短暂阻断触屏，不能用来操作手机；手指保持离屏，等待限时释放。EACCES / EPERM / EBUSY 直接保留为失败，不 chmod、不 Root、不改 SELinux。
4. `paimon-touch-helper forward --device /dev/input/eventN --seconds 30 --display-verified`：只转发；要求已在上一阶段人工核实显示映射。启动时手指全部离屏，虚拟设备枚举后才接管；完整空闲槽快照写入 uinput，若快照期间有输入事件则拒绝接管。先在独立触摸接收器验证单指、双指、快速移动、抬指、旋转/锁屏和异常退出，并归档接收的完整 MotionEvent 序列与延迟。
5. `paimon-touch-helper merge --device /dev/input/eventN --seconds 30 --display-verified`：同样持续转发，另外允许受控合成点击。先在接收器验证第三触点，确认原有两指未出现 CANCEL / UP / 重新 DOWN，再讨论游戏实验。

`--display-verified` 仅记录操作者已完成前置检查的断言，不会自动验证 Android 显示关联，也不意味着横屏映射已通过。helper 的点击坐标是 **evdev 原始 MT 坐标**，不接受全屏像素坐标，不自行猜测旋转/缩放；需要在固定显示配置中校准后再传入。独立触摸接收器、系统 MotionEvent 归档以及游戏人工验收属于后续真机验证，未由本 helper 实现或证明。

## 控制通道与合流

stdin 使用换行分隔的 ASCII 命令，stdout/stderr 为 JSON 行报告；建议调用方同时归档两条输出。关闭输入流会请求正常停止；超长或非法命令进入排空。不要通过一次性关闭 stdin 的管道启动希望持续交互的会话。

```text
permit
tap 1234 5678
permit
stop
```

`permit` 的有效期为 1000ms；调用方必须在前台、识别、权限与显示配置检查成功后才发送，并在运行中持续续租。任意一次过期后不可恢复本轮点击，需等待排空结束后重新启动。helper 尚未接入上述 Android 检查，不应将人工发送 permit 当成自动前台保护已完成。没有许可时只转发真实输入；无效许可、超频和非 merge 模式点击均拒绝。

单次合成触点持续约 50ms，相邻点击至少 1000ms，每轮最多 30 次，不积压补发。物理输入优先，每次循环最多读取 256 个事件，全部写入由同一线程串行完成；合成 DOWN/UP 只发生在物理帧边界，保留并恢复物理选择槽。

虚拟设备复制物理设备的键、轴、分辨率、fuzz/flat、MSC 能力和输入属性，并额外保留一个物理流不可使用的槽及 tracking ID。这样真实手指不会在合成点击中途占用同一槽或 ID；物理槽已经全满时仍拒绝新增合成触点。真实 tracking ID 与各槽轴事件保持原样，合成触点存在时聚合 `BTN_TOUCH` 状态，避免最后一根真实手指抬起时误结束整个触摸。合成槽初始化所有已声明的 MT 附加轴。

当前明确拒绝半多点触控、非 Type B、缺少 DIRECT/BTN_TOUCH、非零 MT 坐标/槽最小值、超过 32 个物理槽、tracking ID 无法额外分配、带工具/触点计数按键或不支持的事件族的设备。拒绝不是设备策略已禁止 grab/uinput 的证据，应依据报告区分“不支持”“权限拒绝”和“实测通过”。

## 验收状态

自动化测试验证流结构、许可/停止/次数边界、CLI 参数拒绝，以及监护进程的卡死与硬期限恢复。Android 构建验证 FFI 和本机链接。它们不证明 MyOS 的创建/抓取权限、显示识别、物理输入体验或原神接受拾取。只有按研究文档的顺序取得设备与人工证据，才能更新工单 01 的门槛；当前保持 **unverified / 后续功能受阻**。
