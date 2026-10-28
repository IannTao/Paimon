# 工单 01：触控共存验证记录

## 当前结论

**失败（当前输入路径，基准设备）。** 2026-10-27，nubia Z50 Ultra / Android 14 / MyOS 14.5.33 上，0.1.3-probe 通过 Shizuku shell UID 2000 执行 `/system/bin/input touchscreen tap`。22:41 的持续摇杆完整轮次执行 10/10 次请求，命令均返回 0、正常自动结束；用户确认实际取得物品，但报告“中间按住摇杆会被打断，导致需要重新压摇杆”。因此拾取有效、触控连续性失败，违反无需重新落指的门槛，当前路径不能作为已验证方案推进。此前 5 次请求的部分运行中，用户报告取得风车菊且未中断；该观察保留，但不能抵消完整轮次暴露的失败。后续工单继续阻塞，需重新讨论输入方案；不以松手再拾取降级。

本会话问题、诊断证据、有效办法及复用命令见 [Android USB 与权限排障](android-usb-troubleshooting.md)。以下环境准备和排查过程保留当时状态，以本节为最新进度。

2026-10-27（返回入口更新）：已构建并安装 `0.1.1-probe`（versionCode 2），增加独立可拖动返回按钮、返回时停止后续点击、保存实验配置及返回按钮遮挡目标时的检查。`:app:assembleDebug` 与 `:app:lintDebug` 通过，Lint 为 0 错误、20 警告。APK SHA-256：`4974053D54841450E41A7DF7A1339483A552431C98B486725760327B3912AF92`。真机已验证按钮在游戏中可拖动，并能点击返回 Paimon；未执行拾取点击，不构成触控共存通过。

更新后已恢复 X=1680、Y=510，切换桌面再返回应用后，通过私有配置文件确认两坐标及默认次数 10 已保存。用户确认游戏画质为“极低”，显示设置栏仍为空，待填写；更新后界面显示“无障碍连接：false”，实验前需重新确认服务连接，必要时关闭再开启 Paimon 无障碍服务。

2026-10-27（实验前确认）：用户重新开启无障碍后，`dumpsys accessibility` 确认 Paimon 服务处于 Bound services。用户随后确认已填写游戏显示设置，创建悬浮控制后按钮显示“已连接：点击开始”。在未启动注入点击时，用户报告摇杆及摇杆加视角操作均正常，无停步、摇杆复位或需要重新落指现象。此为人工基线观察，注入点击共存结果仍待执行。

2026-10-27（继续验证）：用户报告上一轮结束，但应用私有目录不存在 `files`，没有可读取的运行报告；截图的停止原因被固定高度按钮截断，不能确认该轮曾进入点击阶段，不计为有效轮次或触控共存失败。恢复会话时发现无障碍启用名单包含 Paimon，但 Bound services 为空。UIAutomator 可能干扰无障碍服务，后续改用截图和 `dumpsys`，不再在实验期间调用 UIAutomator。

已构建并安装 `0.1.2-probe`（versionCode 3），停止按钮高度随文本展开，并增加 `dumpsys activity service dev.paimon.probe/.ProbeService` 诊断输出。构建与 Lint 通过（0 错误、20 警告）；APK SHA-256：`459C23C3279763F7AC2A15DF112D6B8B7FC4CC9CFA267926875C7546C08E1D0E`。本次更新后系统无障碍启用名单变为空，已恢复用户此前授权的 Paimon 服务并确认绑定。显示设置实际保存为“画质极低；横屏；UI设置待确认；myos 14.5.33”。重新创建暂停控制并进入原神后，诊断确认 `accessibilityConnected=true`、`foregroundPackage=com.miHoYo.Yuanshen`、`inputConnected=true`、`running=false`、`attempts=0`。等待用户显式启动持续摇杆第 1 轮。

2026-10-27（前台误判定位）：用户再次报告结束后，0.1.2 诊断显示 `attempts=0`、无报告、`lastStopReason=未开始：请先进入原神游戏画面`；所记窗口包为 `cn.nubia.gameassist`。同一时刻 `dumpsys activity activities` 的 topResumedActivity、ResumedActivity、mFocusedApp 均为原神，确认“最后一次无障碍窗口事件等于前台应用”的判断错误，游戏助手浮窗造成启动前拦截。本次仍不计有效点击轮次，不构成输入共存失败。

已构建并安装 `0.1.3-probe`（versionCode 4），由 Shizuku shell 查询系统 resumed Activity，在开始、窗口事件变化时核对，并在每次实际点击前再次核对；无法确认或不是目标包时拒绝点击。输入服务 AIDL 版本同步更新。构建与 Lint 通过（0 错误、20 警告）；APK SHA-256：`ACF5DC1694A8F5E0633FFF0CDE2C927411AD3BF6376B49E397AD48282585E188`。真机检查前台查询在 Paimon 中返回 `dev.paimon.probe`、切回原神后返回 `com.miHoYo.Yuanshen`，两边无障碍及输入连接正常；当前控制保持暂停、`attempts=0`，尚需用户启动实测回归。

2026-10-27（开始控制恢复）：用户报告“开始”悬窗消失，诊断确认 ProbeService 已不存在，而返回按钮仍在；无障碍启用名单有 Paimon，但 Bound services 为空，因此重建控制被拦截。未运行 UIAutomator 的期间仍出现未绑定，不能将全部断连归因于 UIAutomator；服务停止和断连的持续性原因尚未确定。重新绑定并创建控制后，诊断确认无障碍、输入连接正常、原神为 resumed Activity、`running=false`、`attempts=0`，截图确认左上角“已连接：点击开始”已恢复。过程中滚动误触过未提交的观察选项，已重新打开应用恢复默认未验证，未写入实验报告。当前游戏截图提示“重新连接服务器”，且场景已离开原拾取位置，需恢复游戏连接并重新确认拾取提示及坐标后再启动实验。

2026-10-27（读取真实运行报告）：本次已从设备归档两份 0.1.3-probe 报告：[22:19 运行，6 次点击请求](reports/ticket-01/1586c618-b132-4cfd-8ed8-c2be45d85de5.json)、[22:29 最新运行，5 次点击请求](reports/ticket-01/3ae5810b-ec09-494a-a3ee-7c076b501681.json)。两份均配置目标 10 次，实际均提前 `User stopped`，每次命令退出码为 0；未记录单次命令失败。屏幕 2480×1116、rotation=1、坐标 (1680,510)、shell UID 2000、场景“持续摇杆”。报告原始人工观察字段均为 unverified，保留原样。当前诊断虽记录窗口为 `cn.nubia.gameassist`，系统 resumed Activity 仍是原神，确认前台误判修复后能实际进入点击阶段。最新运行的游戏接受、物品取得与触控连续性待用户逐项确认，暂不计为完整通过轮次。

2026-10-27（最新运行的人工观察）：针对 22:29 运行 `3ae5810b-ec09-494a-a3ee-7c076b501681`，询问 5 次点击期间是否出现停步、摇杆复位或必须重新落指，用户回复“没有”。因此本轮人工触控连续性记为 yes；游戏接受点击与实际取得物品仍为 unverified。此观察记录于本文，归档原始 JSON 保留设备读取时的内容，不覆盖原始证据；整体结论仍为未验证。

2026-10-27（最新运行拾取观察）：继续询问同一最新运行是否实际拾取到物品，用户回复“是”。结合此前触控未中断的回复，本次人工观察为 gameAccepted=yes、itemObtained=yes、touchContinuity=yes（游戏接受点击依据用户确认取得物品）；物品名称与数量尚待补充。该运行仅执行 5/10 次请求并手动停止，记为一次有效的部分运行观察，尚未完成重复持续摇杆和摇杆加视角双指测试，整体结论保持未验证。

2026-10-27（物品补充）：用户确认 22:29 最新运行拾取的物品为“风车菊”，数量未询问、未确认。该运行的人工观察形成“实际取得风车菊且摇杆触控未中断”的记录；原始报告 5 次请求均返回 0、停止原因为 User stopped。后续需继续重复持续摇杆测试，并执行摇杆加视角双指测试，不提前解除门槛。

2026-10-27（下一轮准备）：用户在新物品旁就位，截图确认单条“「风车菊」的种子”提示。提示中心与原两条提示布局不同，将目标由 (1680,510) 调整为 (1680,558)，次数 10、场景持续摇杆、其余显示配置不变。重建控制时出现 Input service disconnected；旧控制停止与新绑定的时序问题是待查方向，未确认根因。恢复过程中输入法自动弹出曾导致坐标临时误追加为 55885，已在暂停状态恢复正确值并重新打开应用，未用错误坐标启动实验。最终配置文件确认 X=1680、Y=558、count=10；诊断确认无障碍与输入连接正常、resumed Activity 为原神（窗口事件可仍为游戏助手）、running=false、attempts=0，等待用户启动下一轮。归档原始报告不变。

2026-10-27（完整轮次失败）：原始报告 [92f12e21-4da2-4964-b378-16c58cab7fcf](reports/ticket-01/92f12e21-4da2-4964-b378-16c58cab7fcf.json)，开始于北京时间 22:41:10，结束于 22:41:27。持续摇杆，目标 (1680,558)，2480×1116、rotation=1，画质极低，UI 设置待确认，配置 10 次且实际请求 10 次，全部 exitCode=0，stopReason=Bounded sequence ended。用户在本轮结束后报告：“已结束，中间按住摇杆会被打断，导致需要重新压摇杆”。人工 touchContinuity=no，整体判定 failed；游戏接受及取得物品仍待补充。原始设备 JSON 的 unverified 字段保持读取时内容，人工判定以本记录为准。诊断确认 running=false、pending=false、cancelling=false；不再继续该路径的重复或双指测试。系统层具体触点取消机制尚未确认，当前结论限于实测现象和路径，不推断所有 Android 输入方案都不可行。

2026-10-27（失败轮次拾取结果补充）：针对完整运行 `92f12e21-4da2-4964-b378-16c58cab7fcf`，用户确认“拾取到了”。最终人工判定：gameAccepted=yes、itemObtained=yes、touchContinuity=no、overallGate=failed；取得物品的具体名称和数量未进一步确认。原始设备报告保持未修改，人工观察另存于 [判定文件](reports/ticket-01/92f12e21-4da2-4964-b378-16c58cab7fcf.observation.json)。当前路径的失败结论不变，停止继续重复与双指测试，等待重新讨论输入实现。

2026-10-27（替代方案研究）：已查证一手资料并完成无注入的设备权限探测，结论见 [触控共存输入方案研究](input-coexistence-research.md)。优先候选为只读独占物理事件流后完整转发至单一 uinput 虚拟触屏，在同一流中合成拾取触点。实测物理 evdev 写入被拒绝、只读成功，uinput 打开及版本查询成功；对全新 fd 只解除不存在的 grab 得到 EINVAL。未执行真正抓取、未创建设备、未写入事件，因此这些证据不能当作候选方案通过。下一步应先交付限时且可审查的 helper，再依次验证设备创建、纯转发、多指合成及原神实际拾取。既有输入路径失败结论和后续阻塞保持不变。

## 验证环境准备（2026-10-27）

工具统一安装在 `C:\Users\tyc\code\dev-tools`，已有 Rust 工具链补充安装 `aarch64-linux-android` 目标。

| 工具 | 实测版本或位置 |
| --- | --- |
| Microsoft OpenJDK | 17.0.21.0.2，`jdk-17` |
| Gradle | 8.9，`gradle-8.9` |
| Android 命令行工具 | 12.0，`android-sdk\cmdline-tools\12.0` |
| SDK Platform | Android 35，revision 2 |
| Build Tools | 34.0.0 |
| NDK | 27.0.12077973（r27） |
| Platform Tools | 37.0.1，ADB 1.0.41 |

已配置用户级环境变量：

```text
JAVA_HOME=C:\Users\tyc\code\dev-tools\jdk-17
GRADLE_HOME=C:\Users\tyc\code\dev-tools\gradle-8.9
GRADLE_USER_HOME=C:\Users\tyc\code\dev-tools\gradle-cache
ANDROID_HOME=C:\Users\tyc\code\dev-tools\android-sdk
ANDROID_SDK_ROOT=C:\Users\tyc\code\dev-tools\android-sdk
```

用户级 `PATH` 已加入 JDK、Gradle、ADB 和 sdkmanager 所在目录。新开终端后生效；已运行的终端或开发工具需要重新启动以继承环境变量。SDK 许可证已接受。JDK 下载按微软官方 SHA-256 校验；Gradle 从华为云镜像下载，并按 [Gradle 官方校验值](https://gradle.org/release-checksums/) 校验。

本次实际检查结果：

- `scripts/build-probe.ps1` 完整执行成功，包含 Rust 类型与格式检查、Android ARM64 release 交叉编译、`:app:assembleDebug` 和 `:app:lintDebug`。
- Android Lint：0 个错误、17 个警告；Kotlin 编译有两处 `defaultDisplay` 弃用警告。
- `cargo test --workspace --locked --offline`：8 项测试通过；Clippy（警告视为错误）通过。
- APK：`android/app/build/outputs/apk/debug/app-debug.apk`；包名 `dev.paimon.probe`，版本 `0.1.0-probe`（versionCode 1），最低与目标 API 均为 34。
- APK 包含 `lib/arm64-v8a/libpaimon_probe.so`，APK v2 签名验证通过。
- APK SHA-256：`F846B8B7EF898AF7B704202E7ECE6E98BC6BA5EC92106DAFD4DB612CDF598DEE`。
- `adb devices -l`：设备列表为空。尚未执行安装、权限配置、Shizuku 后端确认或游戏内实验。

下一步连接指定真机并开启 USB 调试，授权电脑后执行 `adb devices -l`，确认状态为 `device`，再安装 APK：

```powershell
adb install -r android/app/build/outputs/apk/debug/app-debug.apk
```

随后按 README 准备 Shizuku、悬浮窗和无障碍权限，并执行下述复现流程；真实系统构建号和游戏显示配置仍需在设备上记录。

## 基准与证据

2026-10-27：用户开启“USB 安装”后，`adb install -r` 返回 `Success`，验证 APK 已安装，并已通过 `am start` 打开主界面。此前无效安装会话问题已解除。手机已安装 Shizuku；初次检查尚无运行中的 `shizuku_server`，Paimon 无障碍与悬浮窗权限尚未准备。尚未执行游戏内实验，结论仍为未验证。

2026-10-27（Shizuku 启动）：按用户要求，通过 USB ADB 执行已安装 Shizuku 13.6.0 的 `lib/arm64/libshizuku.so`，启动器返回退出码 0 并报告 `shizuku_server pid is 411`。旧版说明中的外部存储 `start.sh` 在此版本上不存在。此次是电脑辅助准备实验环境，尚未验证脱离电脑后的使用，也未执行点击实验。

2026-10-27（授权确认）：`ps` 确认 Shizuku 服务以 UID 2000 运行；用户在应用中授权后，包管理器确认 `moe.shizuku.manager.permission.API_V23: granted=true`。悬浮窗权限仍为默认状态，无障碍服务尚未启用，继续准备权限。

2026-10-27（权限准备）：用户开启权限后，`appops get --user 0 dev.paimon.probe SYSTEM_ALERT_WINDOW` 确认 `allow`；用户 0 的 `enabled_accessibility_services` 包含 `dev.paimon.probe/dev.paimon.probe.ProbeAccessibilityService`。开始准备游戏内拾取场景和目标坐标，尚未创建点击实验或产生真机结果。

2026-10-27（场景准备）：用户就位后，通过 ADB 截图确认中文游戏画面出现两条“「琉璃百合」的种子”拾取提示。物理屏幕为 1116×2480，截图为横屏，计划使用上方提示中心附近的全屏坐标 X=1680、Y=510。初始画面保存在仓库 `.scratch/android-auto-pickup/ticket01-scene.png`。尚未注入点击；游戏画质、UI 配置及人工基线观察仍待记录。

### USB 连接排查（2026-10-27）

Windows 能枚举 nubia Z50 Ultra 的 MTP 与 ADB Interface，但 `adb devices -l` 为空。重启 ADB、37.0.1 切换 LIBUSB 后端及已有 36.0.0 对照均未恢复。当前 USB 标识 `VID_19D2&PID_0306&MI_01` 绑定 Microsoft `winusb.inf`，设备状态正常，但 Device Parameters 缺少 `DeviceInterfaceGUIDs`，PnP 未列出该设备的 ADB 接口。另两种历史 USB 标识（PID 1352、PID 0310）存在 ADB GUID 注册，目前处于未连接状态。证据指向当前 USB 模式的接口注册缺失；待切换 USB 模式或修复驱动注册后复测，尚未确认修复成功，未安装 APK。

用户将 USB 用途切换为“仅充电”后，设备改为 PID 1352，ADB 成功识别为 `device`，确认该方式可恢复连接。实测型号 NX712J、Android 14、系统显示版本 `MyOS14.5.33_NX712J`，构建指纹 `nubia/CN_PQ82A11/PQ82A11:14/UKQ1.230917.001/20250225.113840:user/release-keys`。流式和普通 APK 安装均失败：`SecurityException: Caller has no access to session -1`；单独执行 `pm install-create` 返回 `Success: created install session [-1]`，未得到有效安装会话。APK 尚未安装，待核实手机的 USB 安装权限；游戏内验证仍未开始。

- 指定设备：nubia Z50 Ultra。
- 指定系统：Android 14、MyOS 14.5.33。
- 游戏语言：中文。
- 真实设备构建号、游戏显示配置、横屏方向与应用版本：待在设备上记录，不能以预设设备信息替代实测信息。
- 输入路径：Shizuku UserService（shell UID 2000）执行 `input touchscreen tap`。
- 实验结果：无，尚未产生真机报告。

## 复现流程

按项目使用说明构建、安装并准备权限。固定显示设置、横屏方向、目标坐标和测试次数，先在不执行注入点击的情况下确认玩家摇杆及视角操作正常，再分别进行下面两组实验。

1. 持续按住并移动摇杆，保持触点不离开屏幕，启动倒计时后执行一轮指定位置点击。
2. 持续操作摇杆，同时用另一根手指转动视角，保持真实双指操作，执行相同点击实验。

建议每组至少重复三轮，每轮请求十次点击。每轮结束后填写并导出人工观察，保留每次命令的时间、返回结果、实际游戏接受情况和取得物品的证据。一次命令成功或单轮偶然成功不足以证明输入共存。

| 场景 | 轮次 | 游戏接受点击 | 实际取得物品 | 停步/复位/触点取消/重新落指 | 结论与报告 |
| --- | --- | --- | --- | --- | --- |
| 持续摇杆 | 部分运行：22:29，5/10 次请求 | 用户确认（依据取得物品） | 风车菊，用户确认，数量未确认 | 用户报告均未出现 | 单次部分运行观察成立；[原始报告](reports/ticket-01/3ae5810b-ec09-494a-a3ee-7c076b501681.json)，重复测试待执行 |
| 持续摇杆 | 完整运行：22:41，10/10 次请求 | 用户确认（依据取得物品） | 用户确认取得物品，名称和数量未进一步确认 | 用户报告按住摇杆被打断，需重新按下 | **失败**；[原始报告](reports/ticket-01/92f12e21-4da2-4964-b378-16c58cab7fcf.json)、[人工判定](reports/ticket-01/92f12e21-4da2-4964-b378-16c58cab7fcf.observation.json) |
| 摇杆加视角双指 | 未执行 | 未验证 | 未验证 | 未验证 | 持续摇杆已失败，停止当前路径的后续实验 |

## 停止与异常复现

在倒计时中和点击之间分别使用悬浮按钮停止，确认不再请求后续点击。结束通知中的关闭操作应关闭悬浮验证。在切换应用、锁屏、关闭无障碍服务、Shizuku 断连或屏幕方向变化后，检查实验停止且不会自行恢复。已经进入系统的在途触摸不能保证撤回，应在记录中区分停止后的新请求与此前在途操作。

## 判定规则

- 通过：两类场景重复测试中，游戏确实接受点击且有实际取得物品的证据，真实触控保持连续，未观察到由注入引起的停步、复位、触点取消或需要重新落指。
- 失败：出现上述触控中断，或点击被游戏忽略、无法有效取得物品；保留环境及复现证据，重新讨论输入路径。
- 未验证：未构建、未执行、证据不足、只有模拟接收器结果，或只知道命令退出码为 0。
- APK 源码、类型检查或命令完成均不构成真机门槛通过。即使工单被关闭，后续实施也必须显式检查验证结论。

## 历史开发环境限制（环境准备前）

本次初始化了 Git 仓库，但会话沙箱拒绝创建 `.git/index.lock`，未能建立提交。Android 构建与真机验证不能在当前环境完成。

用户要求继续后，采用已提出的公开会话行为测试边界，覆盖倒计时、有界次数、在途操作、停止、命令失败及超时。晚于会话期限的命令完成原先会被错误标记为序列完成，已通过先失败再通过的测试修复。自动测试仅覆盖 Rust 会话行为，不证明 Android 平台可运行或真机门槛通过。

本次实际验证结果：完整 Rust 测试套件 8 项全部通过；Rust 类型检查、Clippy（警告视为错误）和格式检查通过；3 个 XML 文件语法检查通过。JNI 分支以主机编译器完成语法与类型检查，不是 Android 交叉编译或 ABI 验证。

最后尝试暂存并提交实现时，Git 仍报告 `Unable to create '.git/index.lock': Permission denied`，因此没有生成提交。

当前文件已按 Standards 和 Spec 两个轴进行静态审查，审查记录见 [工单 01 审查](ticket-01-review.md)。由于没有可写的 Git 历史，审查对象是当前文件而非历史提交 diff。
