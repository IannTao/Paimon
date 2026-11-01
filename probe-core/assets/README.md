# 定位参照资源

本版是 **璃月港地表局部** 的屏幕定位准备，不是全 Teyvat 或分层地图支持。

- 地图来源：[huiyadanli/bettergi-map/public/1024_map.jpg](https://github.com/huiyadanli/bettergi-map/blob/1cd1c95738e6a1aeda51ce556bb1edba9fd623ed/public/1024_map.jpg)，提交 `1cd1c95738e6a1aeda51ce556bb1edba9fd623ed`，原图 22528×19456，原始 SHA-256 `41d348de419d2b50f2d8d65d37fc71353eb1bac8ac2bc31defa821c6280e6bbc`。
- `liyue-harbor.gray`：从原图 `(15744,12544)` 裁取 1024×1024；逐像素 `(77R+150G+29B)>>8` 得到行优先灰度8位数据，未缩放。SHA-256 `18440d0f39978cfed02b6750be1a5702706ccedfc5e09683a6efae8eae5f0cab`。
- 坐标定义依据上述地图仓库同提交 `mapConfig.js` / `coordinateConverter.js`，对照 BetterGI 执行器 `08b34f9823aa5dd2236f72a184c554c2a4b80aac` 的 TeyvatMap / SceneBaseMap：1024级原点 `(16384,12288)`，路线 x/y 分别向左/上递增。局部图像 `(u,v)` 映射为 `x=640-u`、`y=-256-v`；手机图像到参照的比例、旋转和平移由特征匹配估计，不用桌面 UI 像素比例。
- 只接受参照内中心 `(60..964,60..964)`，对应路线坐标 `x∈(-324,580)`、`y∈(-1220,-316)`；区间端点以代码为准。全图之外、水下/地下/分层或未知配置未验证，不能用路线首点兜底定位。
- `mobile-player-arrow.rgb`：基准 Android 2480×1116 HUD 的 40×40 RGB 原始色彩模板，从 `tests/fixtures/localization/harbor-128.png` 的 `(112,112)` 裁取；标定方向约95°（北0，顺时针），模板只用于手机角色箭头识别，摄像机视野扇形不代表角色朝向。SHA-256 `63642a7ca9808450ef62df33e0300c2944277b32fc3052837d27dd9799f5014c`。另一参考位置与实时画面用于复核，不把匹配分数本身当作角度真值。

原始地图及游戏画面权利归其原权利人；以上记录用于固定来源、坐标基线与派生资源身份。原始整图仅用于开发准备，没有打包到 APK。

## #19 手机地图GUI模型

以下模型来自本轮真实游戏画面的半尺寸1240×558概览，见[画面来源](../tests/fixtures/teleport/README.md)。它们用于确认已显示的控件和可用锚点；目标像素先由地图特征拟合和原游戏坐标投影得到，再匹配附近的青白色已解锁锚点标识，不把游戏坐标当作点击像素。

| 文件 | 来源PNG | 裁片(x,y,w,h) | SHA-256 |
| --- | --- | --- | --- |
| map-close.rgb | map-zoom-3 | 1125,13,36,36 | ddf41bd184c0abefc0b6e41111be01c69c4792b7e40426a24791691449b922bf |
| map-zoom-in.rgb | map-zoom-3 | 70,207,16,20 | 140a35605a4c1ce5fd5b73bbf7ef661876c71714080fac2e66e5ad99fde53690 |
| map-waypoint.rgb | map-zoom-3 | 479,82,24,34 | 7f12dc67b18c17860aec790dbeb1fcbf4dd67f286275f1c20e22cabc876b77d8 |
| map-anchor-choice.rgb | map-selected | 827,321,77,26 | d5a6afc14ea055ad1c388c38187c26a803de4ed64d60f1555d06f98e480fd954 |
| map-anchor-header.rgb | map-detail | 914,10,79,25 | 1ef1ca5913b99bf213c27aaae06f50885525a2931af0ec203e9643d0894e4014 |
| map-teleport-button.rgb | map-detail | 1014,495,60,28 | 3c34291233cc4d256491e155306653e5334d475c6f68055eb59ab8ee78c5bad5 |

缩放符号及重叠选项文字使用不透明亮色笔画，不依赖半透明背景地形；选项同时要求暗底。详情标题及传送按钮检查完整图块。模型只开放当前固定中文布局、地表及已验证璃月港锚点；未知布局、层级或不可用目标不能据此猜点。

## #9 拾取视觉模型

固定布局1240×558概览的原始RGB裁切。准备画面、真实运行帧、裁切矩形及验收限制见[画面来源](../tests/fixtures/pickup/README.md)。模型不构成通用OCR或真机验收通过声明；数量独立检查，未知交互不点击。

| 文件 | SHA-256 |
| --- | --- |
| pickup-obtained-heading.rgb | 4df52eb569926005d13997ec094fec3bfa1cc96e9139d5c59ce7554d81e530f2 |
| pickup-obtained-icon.rgb | 2bf6254f12dc6c5068fc1a71ca0523fdc2b793f8e135d349a7bd95e169d2f6a5 |
| pickup-obtained-quantity.rgb | 156751112ebdc8387819d255291b9209a1d007c907f8cfa92990576599938675 |
| pickup-obtained-text.rgb | 2b53c8d25a31962ed35472dc4b24bfe86ddde4eed02bf95bcfbebbadd7204c21 |
| pickup-seed-icon.rgb | c02300fe4283e1419c22e67b6ea083ba5d2062c8307bc8036f3dd0a2fa65f8e6 |
| pickup-seed-text.rgb | 22624f259b4e94de6bac327188949b0d1bd0df6eccf48e9c2be596bc245a2ec7 |
