# #18 地面路径点导航验证

2026-10-31，基准设备 nubia NX712J/Z50 Ultra，Android14 / MyOS14.5.33，原神7.1.0中文，2480×1116、ROTATION_90、锁北小地图。应用0.5.0-navigation / versionCode8，shell Shizuku UID2000。ADB可执行文件按项目规则验证为允许，逐条执行；仅用于安装、开发准备、启动自动检查和读取证据，导航期间没有外部游戏触控。

## 来源和通过范围

原样本为固定提交 `84c7e15e6096eb6c313b1b525ca7b38298971425` 的“01-琉璃百合-璃月港右-3个”，原始字节和来源见[样本](../../probe-core/tests/fixtures/bettergi/README.md)。SHA-256 `1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2`。含必需 teleport，**整条原路线禁止启动，未通过整条验收**。应用先检查原文件身份，再将相同字节传给Rust；显式分段只改变执行范围，保留原数组序号、坐标、id与原SHA。

[原文件导航分段报告](reports/issue-18/segment.json)验证原数组第2—4点；`segment_passed=true`，`original_route_passed=false`。各点均要求至少两帧独立新画面确认：

| 原序号/id | 语义 | 原坐标 | 确认位置 | 距离/阈值 |
| --- | --- | --- | --- | --- |
| 2 / 2 | path | (269.57,-661.89) | (266.875,-664.675) | 3.876 / 5 |
| 3 / 3 | path | (278.26,-666.81) | (273.616,-665.325) | 4.875 / 5 |
| 4 / 4 | target | (279.47,-663.86) | (277.256,-664.537) | 2.315 / 2.5 |

最终构建复测 runId `499142503047776`，59.861秒、49帧、43次输入、15次短时移动，`passed=true` 且确认释放。距离是屏幕定位与原坐标的差值；它不是独立测量的绝对真值。绝对坐标精度锚点及地图来源见[#17记录](issue-17.md)，不得将该局部短路线证据外推所有地形或整个Teyvat。

另用明确标记的[派生导航文件](../../probe-core/tests/fixtures/navigation/derived-ground-orientation.json)验证 orientation：保留原点2/3/4的坐标和id，追加朝向原点1坐标的点，未声称这是上游原文件。SHA `704adb4a00c6e955a43cdd3fd451856bde06de4c6d8194db75a7a5e99628d8e6`。

[派生文件完整报告](reports/issue-18/orientation.json)：runId `498200709502510`，73.145秒、60帧、52个输入回执、17次短时移动；结束确认释放。此报告的 `original_route_passed=true` **仅指该SHA的派生文件整条完成**，不代表上游7点文件。最终按id `2,3,4,1` 推进：

| 派生序号 / 原id | 语义 | 确认位置 | 判据 |
| --- | --- | --- | --- |
| 1 / 2 | path | (266.852,-664.576) | 距离3.821 ≤5 |
| 2 / 3 | path | (273.941,-664.593) | 距离4.855 ≤5 |
| 3 / 4 | target | (277.530,-664.178) | 距离1.966 ≤2.5，连续位置稳定 |
| 4 / 1 | orientation | (277.476,-664.130) | 摄像机89°，目标方位96.175°，误差7.175° ≤12° |

orientation阶段7次转镜头、**0次移动**；精确点至朝向结束的位置差约0.072。角色箭头方向在此处歧义而报告未知；摄像机朝向独立来自两帧视野扇形变化，绝不拿角色箭头代替。目标坐标距离9.572，未被误当成需要走到的地点。target未触发拾取操作。

## 闭环与有界停止

- Rust完整任务入口接收原始字节、范围、运行条件及真实RGB画面，输出观察/转镜头/短时移动/结束。到点来自图像证据，输入回执或经过固定时长都不能推进。
- 地图仍为#17的璃月港局部参照，至少8项特征在3参照像素内一致；全部内点最小二乘精化位置，近期真实定位可将特征候选限定为200参照像素范围，仍需新图像独立匹配。无近期定位不采用此回退。重复/未来/过期时间、未知地图或位置跳变继续拒绝。
- 摄像机校准比较静止转动前后两帧的视野扇形，正负变化扇区相距约90°；不可信时只允许有界校准，不移动。单次视角水平位移不超过240px、180ms；移动120—400ms，每次释放后用新画面重新定位和校准。转镜头期间定位位移超过1.5则停止。
- path距离≤5；target距离≤2.5且相邻位置变化≤1.5；orientation方位误差≤12°且只转镜头。各点至少两帧，重复坐标也不能共享同一帧或去重。起点离第一移动点超过30、偏离当前线段超过12均拒绝；朝向点距当前位置不足2时拒绝模糊方位。
- 连续4次移动距离改善不足0.5、每点40次命令、核心120秒、初次定位5秒、输入回执1秒、画面有效期1500ms、连续3次未知画面均有界停止。输入由新画面租约控制，一帧只允许一次输入；后台截图/输入操作超过2300ms时应用取消，整个应用会话上限130秒。
- shell进程独立650ms看门狗与并行前台检测，正常完成发UP，异常发CANCEL。应用取消走独立执行器，不等待截图或输入RPC结束。释放确认超过2500ms仍未取得则任务不通过并关闭UserService。
- 单轮最多256点，原id的JSON序列化每项≤1KiB、合计≤64KiB；超限仍可导入预览，不能启动超限范围。历史只保存当前状态摘要，不重复完成点，限400条/512KiB；回执限400项；完整完成点仅保存一次。报告读写同限2MiB，旧会话不能覆盖新会话，保存故障会触发结束且不阻止独立取消。
- [按住期间主动停止](reports/issue-18/stopped.json)：实际 `holdingSince>0`，取消时 `heldWhenCancelled=true`，随后 `releaseConfirmed=true`，停止后迟到回调不能再发输入。
- [按住期间切出前台](reports/issue-18/foreground-lost.json)：自动检查先观察到真实按住，再启动Home；停止失败任务且确认释放，不续跑。此情况下shell先释放，应用取消时可已无按住。

终态报告同时保留核心输入回执和平台释放确认。主动停止后核心不再接受迟到输入回执，核心 `input_released` 可能仍为false；平台的 `releaseConfirmed` 才是取消释放证据。总结果 `passed` 必须同时满足核心完成与平台释放确认，不能只读取核心的完成标记。

## 回归与复现

完整任务入口测试读取[派生JSON和全部60帧原像素](../../probe-core/tests/fixtures/navigation/README.md)，按设备记录的时序回放全部52个输入决策，验证实际点序、到点及orientation无移动。其余入口用例覆盖原始传送阻断、显式分段身份、重复坐标/乱序id、未知配置/模式/动作、无进展、偏离、未知图像、静止定位抖动、角色朝向歧义、运行条件失效、停止与迟到回执。没有直接喂入识别后的坐标或方向。

构建与检查全部通过：Rust格式、主机和Android目标Clippy、Android APK/测试APK构建及lint；单文件导航/定位/导入测试；结束时一次Rust全套65通过、4项既有平台条件测试忽略。完整60帧回放核对52个输入决策。Android最终构建的导航分段、真实按住时停止及前台失效检查通过；原7/20点文件、JSONC和离线导入回归通过；两锚点历史图回放与地下拒绝入口也通过，未将历史位置用于导航。检查还在隔离存储验证旧会话写入不能覆盖新会话。

APK：`android/app/build/outputs/apk/debug/app-debug.apk`，SHA-256 `9feb6a69d098bd00d7ee8a7ef10a38d7b2b9cdef156971684e8fe0a897b94f69`，已安装到基准设备。[双轴审查与修复](issue-18-review.md)无剩余阻塞项。APK为本地构建产物，不将二进制提交到源码仓库。

```powershell
# 构建时选择检查runner；SDK/JDK/Gradle配置见README。
gradle -p android :app:assembleDebug :app:assembleDebugAndroidTest :app:lintDebug -PdeviceCheckRunner=dev.paimon.probe.NavigationDeviceChecks --offline
# 安装主APK与测试APK后，各命令单独执行；预先进入已确认起点附近。
adb -s D123133103AF shell am instrument -w -e scenario segment dev.paimon.probe.test/dev.paimon.probe.NavigationDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario orientation dev.paimon.probe.test/dev.paimon.probe.NavigationDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario stop dev.paimon.probe.test/dev.paimon.probe.NavigationDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario foreground dev.paimon.probe.test/dev.paimon.probe.NavigationDeviceChecks
cargo test --workspace --locked --offline
```

开发时在任务开始前用ADB通过游戏地图准备tp128起点，测试启动后不补充摇杆、镜头或标点。前几轮发现截图过期判断顺序、前台查询阻塞滑动、后台检测的迟到取消、视野变化导致匹配不足、角色箭头歧义和两点拟合抖动，均保留失败事实并用真实画面回归，未把这些失败计作通过。地图打开、传送准备操作不属于应用能力证据。

仍未实现自动传送、拾取、飞行、攀爬、战斗和障碍恢复；本次不宣称整条上游路线或完整采集任务通过。镜头校准可能多次重试，超限会停止，当前不承诺运行效率或任意地形成功率。
