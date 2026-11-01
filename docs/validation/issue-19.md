# #19 自动传送与重新定位

2026-11-01，应用0.6.0-teleport / versionCode9。基准设备为 nubia NX712J/Z50 Ultra，Android14 / MyOS14.5.33，原神7.1.0中文、2480×1116、ROTATION_90，shell Shizuku UID2000。交互限定为本机屏幕观察与Android系统输入，不读游戏内存或文件。

## 范围与启动条件

支持Teyvat璃月港地表参照内、正常可游玩HUD、当前固定布局，整条路线必须通过全部能力预检。本轮只解锁空动作/walk的璃月港锚点id128：固定参照坐标 `(267.9441,-665.0128)`，导入请求允许距该锚点≤2单位以兼容上游坐标舍入。原样本首点 `(267.96,-665.16)` 被保留，不改成点击像素，不删除首点。

未知地图、地下/分层或非空未知配置、其他锚点、force_tp、非空动作/参数及未支持移动方式仍阻止执行。地图当前视口需要可经有限放大取得可信参照及可用目标；本轮不实现任意世界地图拖动、自动缩小、切层或解锁锚点。既有地图打开、登录/奖励界面等不满足正常HUD条件时，本轮结束，不在运行中等待玩家补操作。启动前系统授权与开发准备不算任务执行。

界面提供“启动整条受支持路线”和“仅验证首个传送点（非整条通过）”。后者仍读取同一份原字节和SHA，只限制验证范围，不能算原路线通过。预检中同种能力的多次需求采用全部满足，前面的已支持传送不能覆盖后面的未知锚点。

## 实现与判据

1. 原文件进入 `RouteTask`；先从真实小地图两帧确认可游玩位置，再点击已验证固定HUD中的地图入口。
2. 截图保留264×264无损小地图，另生成1240×558JPEG概览；概览最后14行遮盖UID，压缩quality95，JPEG≤700KiB；总Binder包低于1MiB，解码尺寸严格检查。
3. 识别地图关闭及缩放控件，使用地图概览 `(400,80,440,380)` 特征匹配既有璃月港1024参照。至少8个内点、误差<3参照像素、旋转≤3°，尺度范围0.5—12，歧义解拒绝；用相似变换反投影原游戏坐标。投影附近还须识别青白色可用锚点，连续两帧目标稳定<6物理像素，才选择目标。匹配不到时最多4次已识别控件放大；没有可信目标就结束。
4. 重叠目标须识别“传送锚点”选项；详情必须识别锚点标题及可用“传送”按钮。每步依赖新画面，一帧许可只消费一次。单次点击80ms，正常UP、异常CANCEL，并保留650ms独立释放看门狗和前台检查。
5. 点击后必须观察到加载过渡，再用两帧新的小地图匹配确认目标附近≤8单位。重新建立定位跟踪，允许真实传送产生的大位移；不能把之前的位置、输入回执、加载消失或原首点当作到达证据。没有加载证据、返回位置不符或无法定位均不通过。
6. 首点确认后，整条模式把原字节及后续原数组范围交给既有地面任务，等下一帧再推进；不重排、不去重，不执行拾取。每次传送≤45秒、起始定位≤5秒、回执≤1秒、画面许可≤1500ms；整条核心≤180秒、Android会话≤190秒。条件失效、停止和迟到回执均不能继续输入。

屏幕可用不等于输入成功；整体 `passed` 还要求平台确认释放。沿用#18的独立取消执行器、报告所有权和有界历史，保存故障不阻止取消。

## 真机结果

| 场景 | 证据 | 结果 |
| --- | --- | --- |
| 原文件首点传送 | [teleport.json](reports/issue-19/teleport.json) | 通过，runId 554252247812687，12.323秒、15帧、4次点击、0次移动，释放确认 |
| 审查修复后的首点复测 | [teleport-final.json](reports/issue-19/teleport-final.json) | 通过，误差1.202，4次点击，加载/重新定位/释放确认 |
| 按住时主动停止 | [stopped.json](reports/issue-19/stopped.json) | 通过；实际按住时取消，heldWhenCancelled=true，释放确认，迟到回调不续跑 |
| 点击期间切出游戏 | [foreground-lost.json](reports/issue-19/foreground-lost.json) | 通过；前台丢失失败终态、释放确认，不在其他应用继续点击 |
| 以已打开地图作为错误起始画面 | [wrong-start.json](reports/issue-19/wrong-start.json) | 按预期失败，0次输入、定位超时、释放确认 |
| force_tp | TeleportDeviceChecks unsupported | 启动前拒绝，没有创建输入会话 |
| 整条7点原文件试跑 | [whole-route-stalled.json](reports/issue-19/whole-route-stalled.json) | **未通过整条**；传送及原点2—4已确认，第5点移动无进展后停止，passed=false，释放确认 |

成功首点验证输入为固定上游样本，SHA `1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2`，来源提交/路径见[原样本说明](../../probe-core/tests/fixtures/bettergi/README.md)。起点屏幕位置约 `(365.874,-501.877)`，终点 `(266.862,-664.674)`，实际相差约190单位；与原传送点距离1.201≤8。地图拟合80内点、重投影误差0.838，实际目标点击 `(1306,660)`，不是游戏坐标。报告 `teleport_validation_passed=true`、`original_route_passed=false`。

整条试跑证明传送后可以自动交接：首点误差0.960，原第2/3/4点距离3.712/4.999/2.193；第5点停止不能计作整条路线或采集通过。这里保留失败，没有手工补走后重标成功。

开发准备先用ADB通过游戏地图到不同锚点id127，再从应用导入/启动；任务期间不补触控、标点或坐标。ADB只逐条执行安装、准备、启动检查或读取报告，项目本地允许规则已验证，未改全局规则。

## 回归与复现

测试沿用原字节＋真实画面→任务决策/进度/结果的入口。真实首点任务全部15组JPEG与无损小地图回放核对4次输入，并检验实际起终点不同；其他用例覆盖原点序、动作预检、不同视口/运行时JPEG、深灰加载页、缺加载、真实错误位置、中断、回执失败/倒序/迟到及传送后地面交接。没有直接提供识别后的坐标或方向。

初轮因透明缩放控件混入地形背景未识别地图，按期限停止；下一轮因加载页为深灰而未取得加载证据，尽管实际已到目标也没有报告通过。两类均先以真实帧复现，再修正并回归。首轮定位曾短暂未知，程序没有移动角色兜底。

```powershell
# 构建/安装环境见README；各ADB命令单独执行。
gradle -p android :app:assembleDebug :app:assembleDebugAndroidTest :app:lintDebug -PdeviceCheckRunner=dev.paimon.probe.TeleportDeviceChecks --offline
adb -s D123133103AF shell am instrument -w -e scenario teleport dev.paimon.probe.test/dev.paimon.probe.TeleportDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario stop dev.paimon.probe.test/dev.paimon.probe.TeleportDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario foreground dev.paimon.probe.test/dev.paimon.probe.TeleportDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario unsupported dev.paimon.probe.test/dev.paimon.probe.TeleportDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario wrong-start dev.paimon.probe.test/dev.paimon.probe.TeleportDeviceChecks
cargo test -p paimon-probe --test route_execution --locked --offline
```

最终检查通过：Rust格式、主机/Android Clippy、单文件回归；结束时一次Rust全套76通过、4项既有平台条件测试忽略；Android主APK/测试APK构建及lint；审查修复后首点真机复测；原7/20点文件、JSONC/离线导入及两参考点定位回放/地下拒绝的Android入口回归。[双轴审查](issue-19-review.md)的问题已修复，分别复核无剩余阻塞项。

本地APK `android/app/build/outputs/apk/debug/app-debug.apk`，SHA-256 `d9adca404871bdc9604316fe56ed4023450c4fc4544836bd5696ee548509f82b`，已安装到基准设备；二进制不提交源码仓库。未逐项真机验证锁屏、旋转、权限撤销或未解锁账号目标；对应状态路径有入口回归，不能替代物理验收。自动拾取、任意锚点/地图、障碍恢复和完整采集仍未实现。
