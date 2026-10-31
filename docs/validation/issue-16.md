# GitHub #16：BetterGI 原文件导入与预检

2026-10-31，应用版本 `0.3.0-route-import`。目标是只读准备路线，不证明世界坐标定位、导航、传送、拾取或完整采集可用。

## 行为与来源

首页进入独立路线页，用系统 `ACTION_OPEN_DOCUMENT` 读取本地原文件；Rust `route_import::import_route` 统一解析、验证、保留原始数据并预检整条路线。能力矩阵分别表示已识别和可执行；当前所有执行能力为否，报告状态只会是 `invalid` 或 `unsupported`。`executable` 是协议中的第三种状态，本版本没有可产生它的真实执行证据或绕过开关。

坐标是有限 f64 游戏坐标，开启浮点往返解析；顺序与重复点不变，省略 type/move_mode 分别按 path/walk，省略 action 按无动作。JSONC 支持注释和尾随逗号，支持 UTF-8 BOM；重复键、尾随垃圾、非法 UTF-8、非有限/缺失坐标、空点列、非法结构均拒绝。上限 2 MiB、10000 点，保留解析器递归上限。未知行为、非空执行配置和未解释扩展字段保留且阻止执行，诊断包含数组下标和人类点序号。地图缺省 Teyvat；bgi_version 不与本应用版本比较。

两份原始样本来自路线仓库固定提交 `84c7e15e6096eb6c313b1b525ca7b38298971425`，语义基线为执行器 `08b34f9823aa5dd2236f72a184c554c2a4b80aac`。路径、作者及完整 SHA-256 见 [样本清单](../../probe-core/tests/fixtures/bettergi/README.md)。仅这两份字节身份完全匹配的文件标注已核验提交，其他文件显示未核验；不凭名称伪造提交。

合法导入使用 AtomicFile 保存一个信封：原始字节的 Base64、名称、来源 URI、导入时间、SHA-256 及已核验提交。原始字节完整保留，不把解析后 JSON 当作原文件。离线读取校验身份并重新预检，不重开 URI、不信任历史能力报告。无 Internet 权限、仓库订阅、JS 引擎或整仓路线包。原样本仅在测试 APK，debug 构建提供非导出的只读测试适配器；release 不含适配器。

## 测试入口与真机

沿用采集任务准备的公开入口：输入原文件，观察预览和拒绝结果。Rust 测试调用公开导入入口；Android 检查从首页按钮进入，拦截系统文件选择器这一外部边界并提供 content URI，继续经过真实 ContentResolver、原始流、Rust JNI、界面和本地保存。没有伪造内部解析报告。测试不替代系统 DocumentsUI 的人工/ADB 冒烟检查。

设备：nubia Z50 Ultra / NX712J，Android 14 / MyOS 14.5.33，序列号 `D123133103AF`。

- 固定提交 7 点原文件：预览名称、authors、游戏坐标与身份，显示能力未齐。
- 固定提交 20 点原文件：保留重复位置和 fly / stop_flying，显示不可执行。
- 自定义 JSONC：注释、尾逗号、默认 path/walk、旧 author、反序 id、重复坐标、未知动作及参数、config 均保留并诊断。
- 缺失 x：显示格式错误及 `positions[0].x`，清除前一合法预览。
- 离线重读：恢复最近合法 JSONC 路线，重新预检，继续拒绝执行。
- 大配置：2000 个未知配置键、50000 字符名称；能力与诊断各 2004 项，当前页只展示 50 项，翻页后能看到后续字段，单个文本截断显示。
- 大量错误点：1000 个缺失有限坐标的点生成 2000 条诊断；格式错误仍可分页，下一页显示 `positions[25].x`，没有一次创建全部错误控件。
- 全程没有启用路线执行按钮；导入模块不引用游戏输入后端。

最终应用入口检查输出 `INSTRUMENTATION_RESULT: result=PASS: original 7/20-point files, JSONC/defaults/metadata/duplicate ids, invalid input, offline reload, execution refused`、`INSTRUMENTATION_CODE: -1`，含上述大配置和错误分页场景。

真实系统选择器另做冒烟：将第一份原文件复制到手机 `Download/Paimon-issue16-pinned-7.json`，从首页进入路线页，点击选择原文件，在 DocumentsUI 的“下载”选中该文件。UI 层级确认“格式合法，执行能力未齐”、7 个路径点、1182 字节、完整样本 SHA-256 和固定提交；来源为 `content://com.android.providers.downloads.documents/document/raw%3A%2Fstorage%2Femulated%2F0%2FDownload%2FPaimon-issue16-pinned-7.json`。无外部测试报告注入该次预览。

## 复现

首次 `cargo fetch --locked`，然后设置 JDK 17、Android SDK、NDK 和 Gradle 环境，执行：

```powershell
cargo test -p paimon-probe --test route_import --locked --offline
gradle -p android :app:assembleDebug :app:assembleDebugAndroidTest :app:lintDebug '-PdeviceCheckRunner=dev.paimon.probe.RouteDeviceChecks' --offline --console=plain
```

逐条执行以下 ADB 命令（实际环境使用项目授权的完整 adb 路径）：

```powershell
adb -s D123133103AF install -r android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
adb -s D123133103AF install -r android/app/build/outputs/apk/debug/app-debug.apk
adb -s D123133103AF shell am instrument -w dev.paimon.probe.test/dev.paimon.probe.RouteDeviceChecks
```

必须看到 `INSTRUMENTATION_RESULT: result=PASS: ...` 和 `INSTRUMENTATION_CODE: -1`；adb 命令自身退出码为零不等于测试通过。省略 Gradle 的 runner 属性时仍使用原有 MovementDeviceChecks，不会自动运行移动或进入游戏。

## 最终检查

- Rust 全量测试：43 项通过；4 个 ignored 为 watchdog 父测试实际启动的子进程夹具。新增导入入口测试 5 项。
- `cargo check --workspace --locked --offline`、`cargo fmt --all -- --check`：通过。
- workspace 全目标 Clippy 与 paimon-probe Android 目标 Clippy（`-D warnings`）：通过。
- Android `assembleDebug`、`assembleDebugAndroidTest`、`lintDebug`：通过。既有显示 API 弃用警告仍在旧移动/触控代码中。
- ADB 项目规则以实际绝对路径检查，结果 allow；其他权限设置未改动。
- 原文件 `.gitattributes -text` 禁止 Git 换行转换，避免跨机器检出改变身份。

交付 APK：`android/app/build/outputs/apk/debug/app-debug.apk`，版本 0.3.0-route-import，SHA-256 `2a39209bac8a0936de71269f3186ea118e164bdde3f1a6ffc5286f55f7b92575`，已安装到基准设备。

双轴审查见 [审查记录](issue-16-review.md)。定位、自动传送、路线移动和实际拾取均未实现，也未以本票导入通过代替其验收。
