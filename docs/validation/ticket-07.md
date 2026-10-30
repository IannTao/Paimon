# 工单 07：有画面反馈的自动移动

日期：2026-10-30。APK 版本及 SHA-256 见[构建记录](reports/ticket-07/build.json)。范围仅为短距离移动验证，不包含拾取或导航到资源点。基准设备为 nubia Z50 Ultra / NX712J，Android 14、MyOS 14.5.33；游戏包名 com.miHoYo.Yuanshen，中文界面，截图尺寸 2480×1116，显示旋转 ROTATION_90。

## 实现与判定

应用启动默认空闲。用户在应用中完成悬浮窗和 shell Shizuku 授权、记录场景及显示设置后，点击“启动 07 自动移动”，应用自动进入原神并开始检查。07 使用系统 resumed Activity 核对游戏前台，不依赖无障碍连接；旧触控实验继续保留无障碍准备入口。

Shizuku UserService 以 UID 2000 读取原始截图，只在内存中转换成 496×224 RGB 帧，经 JNI 交给 Rust 的完整移动任务入口。固定布局检查要求正常绿色血条和有纹理的小地图；登录页、黑屏、其他尺寸和不满足布局的画面拒绝移动。它不是通用游戏场景分类器。

任务先观察两个时间独立且小地图稳定的基准帧，再执行一次摇杆操作。输入从全屏像素 (455,850) 按下并移动到 (455,730)，目标持续 900ms。shell 本地 watchdog 在 1200ms 请求取消；原生任务总期限 8 秒，应用另有 12 秒期限。移动期间继续取帧并检查运行条件。完成或中断都取消可控触点，迟到回执不能重启任务。

移动判定比较小地图地形环的二维平移，排除中央角色箭头和外围 HUD。允许范围为缩小画面上的 ±6 像素，匹配总误差低于 25000，且相比零位移误差至少改善 20%；移动前稳定误差上限为 8000。移动释放后需再取得两个时间独立、位移一致的画面才判定确认。普通世界动画、命令退出成功和重复旧帧都不足以判为移动成功。阈值仅服务当前固定布局的探针，不是全地图定位精度承诺。

输入返回与输入释放是分开的记录。报告中的 inputStartedUptimeMs 来自 shell 确认已持有摇杆的时间；releasedHoldAtUptimeMs 为正表示结束请求实际取消了仍持有的触点，为 0 表示请求取消时已经释放。releaseConfirmed 表示系统输入接口接受释放，游戏移动结果仍由画面反馈判断。

## 已取得的真机证据

- [正常移动](reports/ticket-07/movement-confirmed.json)：完整应用入口自行取帧、移动并结束，verdictCode=10，确认小地图位移且 releaseConfirmed=true。
- [按住时主动停止](reports/ticket-07/stopped-during-hold.json)：verdictCode=13；真实按住时刻 132363763ms，取消时刻 132364267ms，相差 504ms；未将停止后的输入回执计为成功。
- [按住时切离游戏](reports/ticket-07/foreground-lost-during-hold.json)：观察到本轮真实按住后切到桌面，verdictCode=16，releaseConfirmed=true；该轮不计为移动成功。releasedHoldAtUptimeMs=0 表示 shell 的移动循环已经先行释放，后续结束请求确认其为空闲。
- 登录页实测被拒绝（verdictCode=12），没有进入移动阶段。初版 PNG 截图超过时限的尝试也停止且未移动；实现已改为原始截图流，未把失败尝试计为通过。

真机检查以应用的启动和查看结果控件为入口；中断场景读取与应用共用的任务报告接口，必须先看到当前任务处于真实按住阶段，才施加中断。停止必须得到指定原因及实际取消按住的时间证据；启动失败、旧报告或任意非成功结果均不能替代中断通过。测试不使用会屏蔽无障碍的 UiAutomation，也不在运行中执行玩家移动或转视角操作。

## 候选短路线与准备条件

候选采集路线采用已有诊断的[琉璃百合场景参考图](reports/ticket-07/candidate-route.png)。图中有石板路、草地边线、右侧石坡和两株资源；拾取提示为“「琉璃百合」的种子”。候选起点取图中花丛前的石板路与草地交界，固定朝向第一株资源；第一资源点为前方已经出现明确直接拾取提示的一株。参照物为小地图建筑轮廓、道路边线、石坡和花的位置，移动方向由准备时固定的视角确定。

这是一条有图片参照的短路线候选，不代表已经完成自动定位、路线复现或取得物品；资源是否仍存在、实际起点容差和到点判据由 08/09 的闭环继续验证。当前 07 不要求玩家在执行中标记起点或控制方向，也不要求开发者先实现完整导航。

07 的实际移动实验使用蒙德城喷泉附近石板路，朝草地一侧短移。必要时在任务开始前通过开发用 ADB 后退到有移动空间的位置，应用任务开始后没有补充游戏操作。此场景验证移动输入和画面反馈，不宣称喷泉处是采集目标。游戏现有 HUD 保持原样，具体画质档位未重新核实，因此报告明确记为 quality_unverified；不能据此声称性能验收通过。

## 复现

1. 使用 JDK 17、Gradle 8.9、现有 Android SDK/NDK 构建，安装应用 APK；真机检查另安装 androidTest APK。
2. 在执行前准备 shell Shizuku 和应用授权、悬浮窗权限，并进入基准游戏的正常可游玩场景，保持约定横屏方向。07 不需要录屏授权或无障碍服务；通知授权建议开启，悬浮停止入口不依赖通知权限。
3. 在应用填写真实起点、朝向和显示设置记录，点击“启动 07 自动移动”；运行中不触控游戏。结果显示在悬浮控制和应用“查看 07 最新结果”中，结束后再点击悬浮控制可返回应用。
4. 程序化检查通过以下独立 ADB 命令启动；settings 参数需替换为本轮实际记录。scenario 可分别为 movement、stop、foreground，每条命令单独执行。成功的检查返回 INSTRUMENTATION_CODE: -1，失败返回 0；不能只看 adb 进程退出码。

```powershell
C:/Users/tyc/code/dev-tools/android-sdk/platform-tools/adb.exe -s D123133103AF shell am instrument -w -e settings ACTUAL_SCENE_AND_DISPLAY_RECORD -e scenario movement dev.paimon.probe.test/dev.paimon.probe.MovementDeviceChecks
```

应用的报告保存在私有目录内，默认只保存 JSON，不持久保存游戏截图。验证所用的外部诊断截图及路线参考图不属于应用的默认截图保存行为。ADB 仅用于安装、测试准备、启动检查和取回报告，移动本身由手机上的应用与本地 shell 服务执行，不由电脑逐帧指挥。

## 自动检查与限制

- Rust 完整测试套件：38 项通过；4 个 ignored 测试为 watchdog 父测试显式运行的子进程夹具。
- 新增移动任务测试 8 项，覆盖真实位移证据、仅世界动画、非游戏画面、前置不稳定、输入失败、权限/画面/前台/方向/锁屏/主动停止条件、过期与迟到帧、停止后不重启和总期限。
- 主机全目标 Clippy 与 aarch64-linux-android 目标 Clippy 通过，Rust 格式检查通过。
- Android 应用和真机检查 APK 构建通过，Lint 无错误；保留旧目标 API、固定 ARM64、字符串国际化和旧 API 等警告，不等于广泛机型适配。
- 权限撤销、物理锁屏、旋转及截图进程异常的任务行为已有可重复核心测试或平台保护；没有将其逐项真机实测，仍明确标记为未验证。
- 未执行完整采集、长时间运行、帧率、温度和功耗验收；overallGatheringGate 保持 unverified。断开电脑后的完整独立执行尚未单独测量，不把本次连接下实验扩大为该项通过。

## 平台依据

原始截图格式依据 [Android 14 screencap 源码](https://github.com/aosp-mirror/platform_frameworks_base/blob/android-14.0.0_r1/cmds/screencap/screencap.cpp)，包含宽高、像素格式、颜色空间和无行填充的像素数据。shell 输入接入参考 [Shizuku UserService 官方说明](https://github.com/RikkaApps/Shizuku-API#userservice) 和 [scrcpy InputManager 接入实现](https://github.com/Genymobile/scrcpy/blob/master/server/src/main/java/com/genymobile/scrcpy/wrappers/InputManager.java)；这些资料用于确定实现方式，不能替代本设备上的游戏结果。
