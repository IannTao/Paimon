# GitHub #17：路线准备中的屏幕定位

2026-10-31，版本 `0.4.0-localization`。交付固定手机布局下 **璃月港地表局部** 的位置与角色朝向准备，未实现导航、传送、拾取，不宣称全 Teyvat 定位。

## 数据与坐标

地图来自 bettergi-map 提交 `1cd1c95738e6a1aeda51ce556bb1edba9fd623ed` 的22528×19456原图，只保留璃月港1024×1024灰度裁片。坐标定义与 BetterGI 执行器 `08b34f9823aa5dd2236f72a184c554c2a4b80aac` 对照。1024级图像原点 `(16384,12288)`，路线坐标x向左、y向上；本裁片原点在整图 `(15744,12544)`，所以局部图像 `(u,v)` 对应 `(640-u,-256-v)`。手机图像到参照的尺度/旋转/平移通过图像拟合获得，不复用桌面截屏缩放常数。

完整来源、原图与派生资源SHA-256见 [参照资源](../../probe-core/assets/README.md)，真实回放与独立真值见 [画面清单](../../probe-core/tests/fixtures/localization/README.md)。参考真值来自固定版本 `tp.json` 的出生位置，仅作验收断言；不传入定位核心，不用导入路线首点代替当前位置。

设备NX712J / nubia Z50 Ultra，Android14 / MyOS14.5.33；原神 `7.1.0_48052158_48145775` / versionCode1241，中文，2480×1116横屏。按实际Android HUD裁取 `(180,28,264,264)`，不改尺寸。保留已有UI设置；画质档位未重新逐项核验（旧准备记录为“极低”，仍记quality_unverified），本票不声称画质/性能覆盖。PNG回放保留原像素，参照灰度与角色箭头模板随核心编译，运行时不用网络。

## 匹配与可信门槛

Rust任务入口接收真实RGB小地图及捕获时间：lowe-sift 0.2.1提取尺度/旋转不变特征，排除中心箭头与外部HUD区域；保留响应最高400项，以0.75距离比筛选，最多80组对应。两点假设拟合相似变换，样本间距至少30手机像素，参照/手机尺度须在0.25–3之间；至少8项特征在3参照像素内一致。参照中心限定在裁片内部，第二个相近票数但相差15坐标单位以上的解会拒绝。位置不是07的相对平移结果。

角色朝向使用手机实测彩色箭头模板，基于色度与旋转形状匹配，北为0°、顺时针。低质量或多个方向同样接近时不提供可信定位；不同于摄像机扇形。模板分数上限0.8，距离最佳方向25°以外的候选分数须至少差0.05。方向标定是人工粗读，验证只覆盖清单中的朝向，不承诺全角度精度或导航可用。

画面只在捕获后1500ms内有效，包含定位计算耗时；重复/未来时间戳、过期、模糊、无箭头、未知地图或无法匹配时清空位置。跨帧位移超过 `20 + 15×间隔秒数` 坐标单位会锁存为跳变，必须重新开始准备。预检、核心、回放和实时入口共用支持门槛：只开放局部范围且无未知地图/分层配置的路线，未知root/info/point扩展和非空config仍阻止定位。所有报告的navigation_allowed始终为false。

## 应用入口与真机证据

从首页路线导入页选择原文件，历史截图检查经过系统文件选择结果、真实ContentResolver、Bitmap原尺寸解码和Rust图像入口；结果明确标记“历史画面”，不能当作当前位置。实时入口保存原路线身份，绑定独立只读Shizuku服务，自行进入原神并取帧，悬浮显示当前位置、路线首点、朝向及可信状态；AIDL没有移动/点击方法。截图进程有1200ms watchdog，准备会话上限45秒。停止、失去游戏前台、锁屏或权限/布局改变时撤销实时位置，只留单独的历史lastFix。截图默认不保存，只保存JSON记录。

| 检查 | 独立参考 `(x,y)` | 图像定位 `(x,y)` | 位置误差 | 朝向 |
| --- | --- | --- | --- | --- |
| id128真实图回放 | (267.9441,-665.0128) | (267.2709,-664.5455) | 约0.82 | 93.36°，人工约90°±12° |
| id127真实图回放 | (366.71155,-502.4061) | (365.9834,-501.9131) | 约0.88 | 76.20°，人工约65°±15° |
| id127实时连续帧 | 同上 | (365.9815,-501.9394) | 约0.87 | 80.14°，与人工粗读相差约15°，未用于导航 |
| id128实时连续帧 | 同上id128 | (267.3546,-664.2825) | 约0.94 | 94.21° |

位置验收上限8个游戏坐标单位；上表是有限参考点误差，不能外推整片地图或其他场景的精度。实时检查要求本次runId、至少2帧、新鲜有效期及独立参考坐标，不能用旧记录或仅进程退出码代替。id127与导入路线首点不同，证明没有把路线首点当作角色坐标。

- [id127主动停止](reports/issue-17/live-127-stopped.json)：2帧；停止后active=false、position/heading为空、目标保留；lastFix仅为历史。
- [id127切离游戏](reports/issue-17/live-127-foreground-lost.json)：先取得本轮定位再切桌面，原因“原神不在前台”，实时位置撤销。
- [id128主动停止](reports/issue-17/live-128-stopped.json)：第二参考点2帧通过，导航始终禁用。
- 地下矿区、未知地图、非空分层配置、未知root/info/point字段、过期/重复、模糊/缺帧/箭头缺失及突跳在真实图回放中拒绝，失败不会继承前一位置。

参考锚点由开发用ADB在测试前通过地图准备；定位开始后不补充角色操作、标点或坐标。首次实时检查因Shizuku未运行没有产生会话；按项目已有ADB流程启动已安装的Shizuku后重测通过，没有将该次失败算成定位通过。

## 复现与检查

先运行 `cargo fetch --locked` 并设置JDK17、Android SDK/NDK、Gradle8.9环境：

```powershell
cargo test -p paimon-probe --test localization --locked --offline
gradle -p android :app:assembleDebug :app:assembleDebugAndroidTest :app:lintDebug '-PdeviceCheckRunner=dev.paimon.probe.LocalizationDeviceChecks' --offline --console=plain
```

逐条执行ADB安装与回放：

```powershell
adb -s D123133103AF install -r android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
adb -s D123133103AF install -r android/app/build/outputs/apk/debug/app-debug.apk
adb -s D123133103AF shell am instrument -w dev.paimon.probe.test/dev.paimon.probe.LocalizationDeviceChecks
```

实时检查先准备相应锚点与shell Shizuku/悬浮窗授权。以下命令分别运行，不合并执行；reference只用于验收断言，不输入定位器：

```powershell
adb -s D123133103AF shell am instrument -w -e scenario live -e reference 127 dev.paimon.probe.test/dev.paimon.probe.LocalizationDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario live-foreground -e reference 127 dev.paimon.probe.test/dev.paimon.probe.LocalizationDeviceChecks
adb -s D123133103AF shell am instrument -w -e scenario live -e reference 128 dev.paimon.probe.test/dev.paimon.probe.LocalizationDeviceChecks
```

必须观察 `INSTRUMENTATION_RESULT: result=PASS ...` 和 `INSTRUMENTATION_CODE: -1`。设备回放拦截的只有外部文件选择器，内部图像定位和真实帧接口不做模拟。

Rust全量50项通过（含新增6项定位与定位范围预检回归）；4项ignored为watchdog父测试启动的子进程夹具。类型/格式检查、host与Android Clippy（-D warnings）、Android构建和lint通过。物理锁屏、旋转和权限撤销未逐项真机复验；实现有锁屏、布局及权限检查，但不扩大为这些条件已真机通过。FPS/温度/长期运行未测量。

APK `android/app/build/outputs/apk/debug/app-debug.apk`，SHA-256 `e0936311728ef28e882b51a9628520292191cfdfc9189f98ec575119db8a472b`，已安装基准设备。双轴审查见 [审查记录](issue-17-review.md)。
