# 工单 01：Android 真机连接与权限排障

记录日期：2026-10-27。本文整理本次会话的实测问题、解决办法和未完成事项，供后续继续验证时查阅。

## 实测环境与当前进度

- 手机：nubia Z50 Ultra，型号 NX712J。
- 系统：Android 14，`MyOS14.5.33_NX712J`。
- 构建指纹：`nubia/CN_PQ82A11/PQ82A11:14/UKQ1.230917.001/20250225.113840:user/release-keys`。
- 电脑：Windows；项目 SDK 的 ADB 为 Platform Tools 37.0.1。
- 验证应用：`dev.paimon.probe`，版本 `0.1.0-probe`。
- Shizuku：`13.6.0.r1086.2650830c`，已通过 USB ADB 启动，实测服务 UID 为 2000。
- APK 已安装；Shizuku 已授权；悬浮窗权限已允许；Paimon 无障碍服务已列入启用名单。
- 已执行真实点击实验：完整 10 次运行中用户确认取得物品，但摇杆触控中断、需要重新按下。当前 shell tap 路径触控共存结论为**失败**；[原始报告与人工判定](ticket-01.md)已归档。

以下命令在仓库根目录的 PowerShell 中执行。先设置本机 ADB 路径：

```powershell
$adb = 'C:/Users/tyc/code/dev-tools/android-sdk/platform-tools/adb.exe'
& $adb devices -l
```

命令示例默认只连接一台手机；连接多台时，在 `shell`、`install` 等子命令前加 `-s <设备序列号>`。

## 1. Windows 识别手机，但 ADB 设备列表为空

**现象：** Windows 可见手机和 `ADB Interface`，设备状态为 OK；`adb devices -l` 却只有表头，没有设备，也没有 `unauthorized` 提示。

**已尝试但未解决：** 重新开启 USB 调试、重启 ADB 服务、将 ADB 37.0.1 的 USB 后端切为 LIBUSB、换用电脑已有的 ADB 36.0.0。

**诊断证据：** 文件传输模式的接口标识为 `VID_19D2&PID_0306&MI_01`，绑定 Microsoft `winusb.inf`。该实例的 `Device Parameters` 缺少 `DeviceInterfaceGUIDs`，PnP 未列出该实例的 ADB 接口。手机其他历史 USB 模式存在 ADB 接口注册。因此，“设备状态正常”不能证明 ADB 可以枚举它。

**本次有效办法：** 在手机 USB 通知中将用途从“文件传输”改为“仅充电”，保持 USB 调试开启。切换后设备标识变为 `VID_19D2&PID_1352`，ADB 随即识别为 `device`。未修改注册表或重装驱动。证据指向原 USB 模式的接口注册问题；这是本机实测结果，不代表所有手机都必须使用仅充电模式。

复查命令：

```powershell
& $adb devices -l
Get-PnpDevice -PresentOnly |
    Where-Object { $_.FriendlyName -match 'Android|ADB|nubia' } |
    Select-Object Status, Class, FriendlyName, InstanceId
```

补充原理：[微软 WinUSB 文档](https://learn.microsoft.com/en-us/windows-hardware/drivers/usbcon/winusb-installation)说明了 `DeviceInterfaceGUIDs` 与设备接口注册的关系。

## 2. 安装失败：Caller has no access to session -1

**现象：** 流式安装和 `--no-streaming` 普通安装都报错：

```text
java.lang.SecurityException: Caller has no access to session -1
```

单独运行 `pm install-create` 还返回过 `Success: created install session [-1]`。虽然文字包含 Success，但后续无法访问该会话，不能视为安装成功。

**本次有效办法：** 用户在手机开发者选项中开启“USB 安装”，再次运行原安装命令即成功：

```powershell
& $adb install -r android/app/build/outputs/apk/debug/app-debug.apk
# 实测结果：Performing Streamed Install / Success
```

USB 调试与 USB 安装是两个独立开关；本次无需改 APK 或更换安装传输方式。

## 3. Shizuku 未运行，旧启动脚本不存在

**现象：** Shizuku 已安装，界面显示未运行；以下旧路径不存在：

```text
/sdcard/Android/data/moe.shizuku.privileged.api/start.sh
```

**本次有效办法：** 使用当前已安装 Shizuku 自带的 `lib/arm64/libshizuku.so` 启动程序。先通过包管理器查询 APK 路径，再定位启动程序；不要复用安装目录中的随机字符串，应用更新后目录可能变化。

```powershell
$apkLine = & $adb shell pm path --user 0 moe.shizuku.privileged.api
$apkPath = ($apkLine | Where-Object { $_ -match '^package:.*/base\.apk$' } |
    Select-Object -First 1) -replace '^package:', ''
if (-not $apkPath) { throw '未找到 Shizuku base.apk' }
$starter = $apkPath.Trim() -replace '/base\.apk$', '/lib/arm64/libshizuku.so'
& $adb shell ls -l $starter
& $adb shell $starter
& $adb shell ps -A -o UID,PID,NAME | Select-String 'shizuku_server'
```

此示例针对本次 ARM64 安装布局。实测启动器退出码为 0，服务 PID 为 411，`ps` 确认 UID 为 **2000**；PID 只作本次证据，后续启动可能不同。本实验不接受 Root UID 0。

此次电脑仅辅助准备环境。尚未验证断开电脑后的运行，也未验证重启手机后的恢复。Shizuku 运行与 Paimon 获得 Shizuku 授权是两个步骤：启动后还须在 Paimon 中点击“3. 请求 Shizuku 授权”，并允许请求。

## 4. 权限是否已经开启

本次通过以下命令核实，避免只凭用户点击或设置页外观判断：

```powershell
# Shizuku 授权：查找 API_V23: granted=true
& $adb shell dumpsys package dev.paimon.probe |
    Select-String 'API_V23|versionName'

# 悬浮窗：本次结果 SYSTEM_ALERT_WINDOW: allow
& $adb shell appops get --user 0 dev.paimon.probe SYSTEM_ALERT_WINDOW

# 无障碍启用名单：本次包含下面的组件
& $adb shell settings --user 0 get secure enabled_accessibility_services
# dev.paimon.probe/dev.paimon.probe.ProbeAccessibilityService
```

悬浮窗此前为 `default`，用户开启后变为 `allow`。无障碍服务列入启用名单只证明系统配置；实际服务连接情况仍应看 Paimon 顶部的“无障碍连接”状态。通知权限建议开启，本会话未确认其结果。

## 5. 从游戏回不到 Paimon

本会话后续已增加常驻返回按钮，更新 APK 为 `0.1.1-probe`（versionCode 2）。允许悬浮窗后打开 Paimon 即会显示；无需创建点击实验，可以拖动调整位置，点击返回应用。返回时停止实验后续点击，系统已接受的在途触摸不能保证撤回。应用和通知均可关闭该返回按钮；强制停止、重启或系统终止服务后需重新打开应用恢复。

本次已在真机验证：返回按钮显示为前台服务；在原神中拖动按钮会改变窗口位置；点击拖动后的按钮，界面返回 `dev.paimon.probe`；检查期间未发现 AndroidRuntime 崩溃。升级同时增加实验配置的离开页面保存；旧版没有持久化的输入不会自动恢复。仍未执行游戏内拾取点击实验。

**本次有效办法：** 用显式组件名及用户 0 将验证应用切到前台：

```powershell
& $adb shell am start --user 0 -n dev.paimon.probe/.MainActivity
```

实测返回 `Starting: Intent ...`；此前也出现过“当前任务已带到前台”的提示。切回应用不会启动点击实验。

另一次尝试通过通用 MAIN/LAUNCHER Intent 打开 Shizuku 时返回 `unable to resolve Intent`，原因未查明；包管理器确认它已安装且未停用，后来用户能手动打开。不能据此断言 Shizuku 未安装。

## 6. 工具沙箱限制与手机问题的区别

最初在沙箱内执行 ADB 报 `Cannot mkdir '\.android': Permission denied`；读取 Windows PnP 设备时也报过“拒绝访问”。获准在沙箱外执行后，工具能够运行，才继续定位真实 USB 问题。这两类错误属于电脑执行权限限制，不是手机拒绝 USB 调试，也不能通过反复切换手机设置解决。

会话中的“沙箱外执行”审批与 Windows 管理员提权不同。本次检查显示进程不是 Windows 管理员，未执行驱动安装或注册表修改。

用户后续在 AGENTS.md 明确授权项目任务范围内的 ADB 操作，已据此创建项目级 [adb.rules](../../.codex/rules/adb.rules)，精确匹配实际 SDK 的 adb.exe（正斜杠及 Windows 反斜杠路径），未修改全局规则或其他权限设置。使用本机 Codex 0.164.0 的 `execpolicy check` 验证 7 项案例全部通过：设备列表、shell、安装匹配 allow；其他路径、裸 adb 命令、近似文件名及通用 PowerShell 均不被此规则放行。验证结果保存在 [.scratch/android-auto-pickup/adb-policy-check.json](../../.scratch/android-auto-pickup/adb-policy-check.json)。

按 [OpenAI 官方规则文档](https://learn.chatgpt.com/docs/agent-configuration/rules)，项目 `.codex/` 配置层需受信任，且规则在启动时加载；添加规则后需重启 Codex。规则文件校验通过不表示当前会话已热加载，也不替代外部执行环境的权限限制。

重启后，多行 PowerShell 包装的 ADB 调用仍出现审批。改为每次 `exec_command` 仅提交一条直接使用绝对路径的 ADB 命令后，已实测连续执行成功，没有再次等待审批。电脑端过滤、变量和文件处理分开执行；不通过放行整个 PowerShell 来解决。例：`C:\Users\tyc\code\dev-tools\android-sdk\platform-tools\adb.exe devices -l`。ADB 在沙箱内仍可能遇到 `.android` 目录错误，因此工具调用仍需标记 `require_escalated`，由已批准规则处理。

## 7. 无障碍已启用，但没有连接；开始失败且无报告

继续验证时实测启用名单有 Paimon，而 `dumpsys accessibility` 的 Bound services 为空。不能仅以系统开关判断服务已连接。[Android 官方文档](https://developer.android.com/reference/android/app/UiAutomation#FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES)说明 UIAutomation 默认屏蔽其他无障碍服务；本次此前使用过 `uiautomator dump`，这是可能的干扰因素，尚未隔离证明唯一根因。后续实验用 `screencap`、`dumpsys accessibility` 和应用诊断检查，避免使用 UIAutomator。

本次更新 APK 后还实测无障碍启用名单变为空，恢复已授权的 Paimon 服务后确认 Bound services 正常。不要在未读取并保留其他服务名单的情况下覆盖 `enabled_accessibility_services`。

已安装 0.1.2-probe，将被截断的停止提示改为随文本展开，并提供诊断命令：

```powershell
C:\Users\tyc\code\dev-tools\android-sdk\platform-tools\adb.exe shell dumpsys activity service dev.paimon.probe/.ProbeService
```

实测准备完成时：`accessibilityConnected=true`、`foregroundPackage=com.miHoYo.Yuanshen`、`inputConnected=true`，同时 `running=false`、`attempts=0`。如果没有 `files/probe-reports`，不能把“已结束”或按钮显示“已停止”当作点击执行完成；先读取 `lastStopReason`。之前无报告的尝试不计入有效实验轮次，也不据此判定输入共存失败。

## 8. 原神仍在前台，却提示“请先进入原神”

0.1.2 真机诊断捕获到：无障碍记录的最后窗口包为 `cn.nubia.gameassist`，但系统 topResumedActivity、ResumedActivity、mFocusedApp 均指向原神。原型误把游戏助手浮窗当成切换应用，在开始前拒绝实验，`attempts=0`，没有运行报告。

0.1.3 已改为由 Shizuku shell 查询系统 resumed Activity，在开始、窗口事件变化及每次实际点击前核对目标包，查询失败时拒绝点击。无需关闭游戏助手，也未对 `cn.nubia.gameassist` 设置绕过前台检查的白名单。已实测从 Paimon 切换到原神时分别得到正确包名；修复后的手指操作与拾取点击共存仍待用户启动实测。

## 9. 点击命令成功、物品取得，但摇杆被打断

完整轮次 `92f12e21-4da2-4964-b378-16c58cab7fcf` 的 10 次 shell tap 全部返回 0、程序自动结束，用户确认实际取得物品，但按住摇杆中途被打断，必须重新按下。当前输入路径判定失败，不能用调整成功统计或松手拾取作为通过。保留此前短轮次未中断的观察，同时以完整轮次暴露的失败维持后续工单阻塞。具体系统触点取消机制尚未证实，不能将此结果扩大为所有系统输入方案都不可行。

## 原实验准备信息（历史，当前路径已失败）

手机已切回 Paimon。用户先前确认坐标填好，更新为 0.1.1 后已恢复 X=1680、Y=510，并确认离开应用时成功保存。初始游戏截图见 [ticket01-scene.png](../../.scratch/android-auto-pickup/ticket01-scene.png)，其中有两条“「琉璃百合」的种子”拾取提示。

- 屏幕物理尺寸：竖屏 1116×2480；此次截图为横屏。
- 计划目标：上方拾取提示中心附近 **X=1680、Y=510**，尚未验证点击命中。
- 原神包名：`com.miHoYo.Yuanshen`。
- 用户确认画质为“极低”，显示设置栏仍为空；UI 显示设置、横屏方向及未注入点击时的真实触控基线待记录。
- 更新后应用显示无障碍连接为 false；实验前确认服务已连接，必要时在系统中关闭再开启 Paimon 无障碍服务。
- 待执行：持续摇杆、摇杆加视角双指两类重复实验，以及停止和异常实验。

上述坐标仅适用于当时画面；移动角色、旋转屏幕或改变 UI 后应重新确认。每轮需要游戏接受点击、实际取得物品和触控连续性的观察证据。权限准备与命令返回成功都不能替代真机验证通过，完整判定规则见 [工单 01 验证记录](ticket-01.md)。
