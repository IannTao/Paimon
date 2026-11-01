# #19 传送真实画面

本轮在 nubia NX712J/Z50 Ultra、Android14 / MyOS14.5.33、原神7.1.0中文、2480×1116、ROTATION_90 上采集。PNG/JPEG均来自真实手机画面，未绘制地图或注入识别坐标。

- `world / map-zoom-3 / map-selected / map-detail / loading / arrived`：开发准备过程的六类真实画面；`*-mini.png` 是原图 `(180,28,264,264)` 原像素裁片。其余PNG为1240×558概览，像素取全尺寸 `(2x+1,2y+1)`；概览最后14行置黑以屏蔽UID。
- `runtime-world-127.jpg`、`runtime-map-127.jpg` 及各自mini：首轮真实任务 runId `553487698062041` 的捕获 `553490775/553493246`。复现透明缩放控件受不同地图背景影响，程序当时未通过地图识别并超时释放。
- `runtime-loading.jpg` / mini：runId `554003380586115`，捕获 `554011919`；复现深灰加载页未被原黑色过渡判据识别。该轮确已移动到另一位置，但程序未取得加载证据，因此未报通过。
- `phone-run/`：成功首点传送 runId `554252247812687` 的全部15组原始JPEG概览及无损小地图。文件名为设备elapsedRealtime捕获时间。JPEG由手机Bitmap压缩quality95，并在编码前遮盖UID；测试按实际JPEG解码，不替换成无损概览。时序、命令、回执、原SHA与定位结果见[原始报告](../../../../docs/validation/reports/issue-19/teleport.json)。完整入口回放核对全部4次地图输入，并确认真实起终点相差超过150个游戏坐标单位。

任务始终读取 `bettergi/route-1.json` 的原始字节，SHA `1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2`，来源见[固定上游样本](../bettergi/README.md)。`FirstTeleport` 只验证该原文件首点，没有删改文件；报告 `original_route_passed=false`。

GUI模型均为行优先RGB8，从上述半尺寸PNG裁取，原点/大小见[资源说明](../../../assets/README.md)。角色、物品、任务等画面属于游戏显示，不是程序输入；本轮没有拾取能力通过证据。开发准备时的ADB传送不算应用自动传送证据，任务启动后只允许程序自身的屏幕观察和Android输入。
