# #18 真实屏幕回放

设备：nubia NX712J/Z50 Ultra，Android14 / MyOS14.5.33，原神7.1.0、中文、2480×1116、ROTATION_90。所有PNG均为本机真实截图的 `(180,28,264,264)` 小地图原像素裁片，未绘制地图或识别坐标，不包含游戏UID。

- `camera-before/after.png`：同一位置的开发准备画面，ADB水平滑动160px，仅用于从两帧视野扇形变化验证镜头校准。角色朝向没有随镜头改变。
- `ground-before/after-turn.png`：runId `497481094871535`，捕获时间 `497508417/497509721`；镜头转动后全局特征匹配不足，近期真实定位约束候选特征后仍要求至少8项新图像匹配。
- `ground-after-move.png`：runId `497624386883511`，捕获 `497660599`；移动后角色箭头朝向存在约120°歧义。位置可由新图像和可见箭头轮廓独立确认，角色朝向报告未知，绝不作为镜头朝向。
- `jitter-before/after.png`：runId `498092264348124`，捕获 `498093633/498094901`；用于复现两点假设位置抖动，以全部内点最小二乘拟合修正，保留静止转镜头位移上限1.5。
- `orientation-run/*.png`：成功 runId `498200709502510` 的全部60帧；文件名为设备elapsedRealtime捕获时间。任务输入、命令、回执与逐点结果见 [`orientation.json`](../../../../docs/validation/reports/issue-18/orientation.json)。入口回放测试读取原始JSON字节及这些真实像素，核对全部52个输入决策；不向任务直接注入已识别的位置或朝向。

`derived-ground-orientation.json` 是明确标记的派生验收文件，SHA-256 `704adb4a00c6e955a43cdd3fd451856bde06de4c6d8194db75a7a5e99628d8e6`。坐标和原id来自固定上游样本 `bettergi/route-1.json`（来源提交、路径及SHA见[原样本说明](../bettergi/README.md)），按原点2、3、4保留 path/path/target，追加朝向原点1坐标的 orientation。它不包含传送，验证地面跑图和只朝向语义；它通过不表示上游7点原路线通过。

导航和定位算法为项目实现；点位语义对照 BetterGI 执行器固定提交 `08b34f9823aa5dd2236f72a184c554c2a4b80aac` 的 `PathExecutor.cs`（orientation 只调用旋转），没有移植其桌面端输入和缩放常数。
