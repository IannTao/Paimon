# 定位参照资源

本版是 **璃月港地表局部** 的屏幕定位准备，不是全 Teyvat 或分层地图支持。

- 地图来源：[huiyadanli/bettergi-map/public/1024_map.jpg](https://github.com/huiyadanli/bettergi-map/blob/1cd1c95738e6a1aeda51ce556bb1edba9fd623ed/public/1024_map.jpg)，提交 `1cd1c95738e6a1aeda51ce556bb1edba9fd623ed`，原图 22528×19456，原始 SHA-256 `41d348de419d2b50f2d8d65d37fc71353eb1bac8ac2bc31defa821c6280e6bbc`。
- `liyue-harbor.gray`：从原图 `(15744,12544)` 裁取 1024×1024；逐像素 `(77R+150G+29B)>>8` 得到行优先灰度8位数据，未缩放。SHA-256 `18440d0f39978cfed02b6750be1a5702706ccedfc5e09683a6efae8eae5f0cab`。
- 坐标定义依据上述地图仓库同提交 `mapConfig.js` / `coordinateConverter.js`，对照 BetterGI 执行器 `08b34f9823aa5dd2236f72a184c554c2a4b80aac` 的 TeyvatMap / SceneBaseMap：1024级原点 `(16384,12288)`，路线 x/y 分别向左/上递增。局部图像 `(u,v)` 映射为 `x=640-u`、`y=-256-v`；手机图像到参照的比例、旋转和平移由特征匹配估计，不用桌面 UI 像素比例。
- 只接受参照内中心 `(60..964,60..964)`，对应路线坐标 `x∈(-324,580)`、`y∈(-1220,-316)`；区间端点以代码为准。全图之外、水下/地下/分层或未知配置未验证，不能用路线首点兜底定位。
- `mobile-player-arrow.rgb`：基准 Android 2480×1116 HUD 的 40×40 RGB 原始色彩模板，从 `tests/fixtures/localization/harbor-128.png` 的 `(112,112)` 裁取；标定方向约95°（北0，顺时针），模板只用于手机角色箭头识别，摄像机视野扇形不代表角色朝向。SHA-256 `63642a7ca9808450ef62df33e0300c2944277b32fc3052837d27dd9799f5014c`。另一参考位置与实时画面用于复核，不把匹配分数本身当作角度真值。

原始地图及游戏画面权利归其原权利人；以上记录用于固定来源、坐标基线与派生资源身份。原始整图仅用于开发准备，没有打包到 APK。
