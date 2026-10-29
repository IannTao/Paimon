# Paimon

当前实现为工单 01 的触控共存验证原型：在同一台 Android 手机上，由用户显式启动一轮有限次数的指定坐标点击，检查真实摇杆和视角操作是否被打断。它不识别物品，也不是已通过验证的自动拾取工具。

2026-10-27 真机结论：当前 Shizuku shell `input touchscreen tap` 路径在基准设备上出现摇杆触控中断、需要重新按下，**触控共存验证失败**。后续功能仍受此门槛阻塞；完整证据见 [验证记录](docs/validation/ticket-01.md)。

替代方向及权限实测见 [触控共存输入方案研究](docs/validation/input-coexistence-research.md)；单一虚拟触屏转发方案尚未通过真机验证。

已实现独立的限时 Rust/native 单流代理 helper。真机取得创建/销毁、显示识别、watchdog、grab、转发和一次单指加合成触点的证据；已捕获仅含触摸按键的物理帧，修正其导致代理退出的问题，待修复版真机复测。三指测试还触发过系统分屏及触摸取消，用户已关闭该手势。尚未接入现有 APK，第三触点与游戏共存门槛未通过。详见 [阶段验证记录](docs/validation/touch-helper-device-validation.md) 与 [helper 操作说明](docs/validation/touch-helper.md)。

Rust 控制倒计时、点击次数、单次在途操作和停止状态；Kotlin 提供权限引导、悬浮控制、Shizuku UserService、真实 shell 点击与本地报告。输入路径是 `/system/bin/input touchscreen tap`，并不预设这种路径能与玩家触控共存。

## 构建

开发环境需要 Rust、JDK 17、Gradle 8.9、Android SDK Platform 35、Build Tools 34.0.0 和 NDK 27.0.12077973。目标为 `arm64-v8a`，应用最低与目标 Android API 均为 34。依赖固定为 AGP 8.7.3、Kotlin 2.0.21 和 Shizuku API 13.1.5。本仓库没有附带 Gradle Wrapper 二进制，使用已安装的 Gradle 8.9。

```powershell
rustup target add aarch64-linux-android
# 设置 JAVA_HOME、ANDROID_HOME，并确保 cargo 和 Gradle 8.9 可运行。
sdkmanager "platforms;android-35" "build-tools;34.0.0" "ndk;27.0.12077973" "platform-tools"
./scripts/build-probe.ps1
# 或指定 Gradle 可执行文件：
./scripts/build-probe.ps1 -Gradle C:/tools/gradle-8.9/bin/gradle.bat
```

首次解析 Android 依赖需要网络。Rust 核心没有第三方依赖，使用锁文件与离线构建。Gradle 在打包前调用 Cargo 交叉编译并将生成的本机库打入 APK，缺少 NDK 或 Rust 目标时构建失败，不生成缺少本机库的“成功”交付。

输出 APK 为 `android/app/build/outputs/apk/debug/app-debug.apk`。仅做 Rust 类型与格式检查可运行 `./scripts/build-probe.ps1 -CheckOnly`；这不代表 Android 编译、安装或真机验证通过。

Rust 会话行为测试：`cargo test --workspace --locked --offline`。测试通过公开会话接口验证倒计时、有界次数、单次在途、停止、命令失败和超时，不证明 Kotlin 平台接入、JNI ABI 或真实触控共存。

## 手机上使用

1. 安装 APK 和 Shizuku，在手机上通过无线调试启动 Shizuku。本实验要求 shell UID 2000，不接受 Root 后端。
2. 在应用中准备悬浮窗权限、Paimon 无障碍服务和 Shizuku 授权；通知权限建议开启。无障碍服务只观察前台窗口，不读取界面内容或派发手势。本原型不获取截图，无需录屏授权。

允许悬浮窗并回到应用后，会自动显示可拖动的“Paimon”返回按钮，无需先创建点击实验。点击可从游戏返回应用，并停止当前实验的后续点击；已进入系统的在途操作可能仍会完成。返回按钮可在应用或通知中单独关闭，在应用中点击“显示返回 Paimon 悬浮按钮”可再次开启。坐标、次数、游戏包名、显示设置和场景会在离开应用时保存。强制停止、重启或系统终止服务后，需要重新打开应用恢复返回按钮。

3. 固定游戏显示设置和横屏方向，用系统开发者选项的指针位置确认拾取按钮的全屏像素坐标，以左上角为原点。记录游戏设置、方向及 MyOS 版本，选择测试场景和 2–30 次点击。
4. 创建悬浮控制。它默认暂停，不会立即点击。进入对应原神包名的游戏画面后，点击左上角悬浮按钮启动，5 秒倒计时内恢复摇杆或摇杆加视角操作。
5. 每次命令完成后间隔至少 1 秒再请求下一次，不积压或补发；一轮最多 30 次，最长 120 秒。再次点击悬浮按钮停止后续请求，通知也可关闭验证。已被系统接受的触摸无法撤回，报告保留在途结果。
6. 权限、无障碍连接、前台游戏、锁屏或屏幕配置检查失败时停止。结束后关闭悬浮验证，回到应用填写游戏接受点击、实际取得物品与触控连续性的人工观察，导出 JSON 报告。

每轮报告独立存放在应用私有存储中，界面导出最近一轮；需要保存多轮证据时，在下一轮开始前导出。报告不包含游戏截图。命令退出码为 0 只表示命令返回成功，整体门槛保持 `unverified`，必须依据重复真机实验人工确认。

完整复现与结果记录见 [工单 01 验证记录](docs/validation/ticket-01.md)。

本次真机连接、USB 安装、Shizuku 启动和权限问题的处理方法见 [Android USB 与权限排障](docs/validation/android-usb-troubleshooting.md)。
