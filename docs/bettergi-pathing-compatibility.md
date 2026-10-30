# BetterGI 路径点兼容方案

2026-10-30 核查。用户指定的来源为 [bettergi-scripts-list/repo/pathing](https://github.com/babalae/bettergi-scripts-list/tree/main/repo/pathing)。这是开发计划和兼容合同，不表示导入器、定位或路线执行已经实现。

## 上游基线

本次只读核查固定以下提交；main 后续更新不自动扩大 Paimon 的兼容声明。实现导入器时以固定提交和真实样本建立回归基线，升级单独记录。

- 路线仓库：[84c7e15e6096eb6c313b1b525ca7b38298971425](https://github.com/babalae/bettergi-scripts-list/tree/84c7e15e6096eb6c313b1b525ca7b38298971425/repo/pathing)。
- 执行器源码：[08b34f9823aa5dd2236f72a184c554c2a4b80aac](https://github.com/babalae/better-genshin-impact/tree/08b34f9823aa5dd2236f72a184c554c2a4b80aac/BetterGenshinImpact/GameTask/AutoPathing)。
- [官方格式说明](https://www.bettergi.com/dev/pathing-dev.html)用于对照；实际字段拼写和缺省行为以固定版本源码与样本为依据。

## 已核查的文件与语义

[PathingTask](https://github.com/babalae/better-genshin-impact/blob/08b34f9823aa5dd2236f72a184c554c2a4b80aac/BetterGenshinImpact/GameTask/AutoPathing/Model/PathingTask.cs)包含路线信息、配置和有序点列。[PathRecorder](https://github.com/babalae/better-genshin-impact/blob/08b34f9823aa5dd2236f72a184c554c2a4b80aac/BetterGenshinImpact/GameTask/AutoPathing/PathRecorder.cs)使用 snake_case，并允许注释及尾随逗号；不能只依据文档里的 camelCase 字段实现读取。

| 部分 | 兼容计划 |
| --- | --- |
| `info` | 读取名称、任务类型、地图、版本与作者等信息；真实样本使用 `bgi_version` 和 `authors` 数组，保留原始元数据，不只读取单个 `author`。 |
| `positions` | 保留数组顺序、点位编号与重复位置；可选 `id` 作为原始标识，不能按 id 重排或按坐标去重。运行进度关联数组序号和原始 id。 |
| `x`、`y` | 保留带符号的浮点游戏坐标精度，禁止按屏幕像素、经纬度或摇杆位移直接解释；缺失、非法数值与非有限坐标不能进入执行。 |
| `type` | 识别 `teleport`、`path`、`target`、`orientation`，分别表达传送、途经、精确经过和朝向语义；`target` 不自动等同于物品，也不保证到点就有拾取提示。 |
| `move_mode` | 识别并保留移动模式。上游源码列有 walk、run、dash、climb、fly、jump、swim；手机执行能力逐项验证，不能把所有模式改成步行。 |
| `action`、`action_params` | 区分无动作和具体到点动作，保留参数及顺序。未知或未实现的行为不能静默忽略。 |
| `config`、扩展字段 | 保存原数据；先区分展示元数据与会改变执行的配置。执行相关配置未理解或不支持时阻止执行，不能直接套用桌面端参数。 |

上游 [Waypoint](https://github.com/babalae/better-genshin-impact/blob/08b34f9823aa5dd2236f72a184c554c2a4b80aac/BetterGenshinImpact/GameTask/AutoPathing/Model/Waypoint.cs)在省略类型和移动模式时采用 path / walk；导入器需要显式记录沿用的缺省规则，不能用同样方式为缺失坐标或未知枚举猜值。[PathingTaskInfo](https://github.com/babalae/better-genshin-impact/blob/08b34f9823aa5dd2236f72a184c554c2a4b80aac/BetterGenshinImpact/GameTask/AutoPathing/Model/PathingTaskInfo.cs)默认地图为 Teyvat，其他地图或分层位置必须检查支持情况。`bgi_version` 是上游版本信息，不与 Paimon 应用版本直接比较；执行资格由已验证的格式和能力矩阵决定。

[WaypointForTrack](https://github.com/babalae/better-genshin-impact/blob/08b34f9823aa5dd2236f72a184c554c2a4b80aac/BetterGenshinImpact/GameTask/AutoPathing/Model/WaypointForTrack.cs)在执行前还会将游戏坐标转换为地图图像坐标。这说明 07 已验证的局部小地图平移不是绝对路线定位：仍需地图参照、坐标变换、朝向和定位置信度的真机验证，不能把桌面图像缩放参数直接移到手机。

## 两条真实样本

| 上游样本 | 核查结果与用途 |
| --- | --- |
| [01-琉璃百合-璃月港右-3个](https://github.com/babalae/bettergi-scripts-list/blob/84c7e15e6096eb6c313b1b525ca7b38298971425/repo/pathing/地方特产/璃月/琉璃百合/01-琉璃百合-璃月港右-3个.json) | 7 个点，使用 walk，动作字段为空，首点为 teleport，之后有 path 和 target。优先作为基础兼容和采集候选；不能因缺传送能力直接删除首点。 |
| [02-琉璃百合-璃月港左-7个+霓裳花4x2个](https://github.com/babalae/bettergi-scripts-list/blob/84c7e15e6096eb6c313b1b525ca7b38298971425/repo/pathing/地方特产/璃月/琉璃百合/02-琉璃百合-璃月港左-7个%2B霓裳花4x2个.json) | 20 个点，含 fly、stop_flying，且相邻点可有相同坐标但不同动作。用于验证重复点保留、完整能力预检及“能读取但暂不能执行”的报告。 |

样本中的资源名称和点位不代表手机上资源当前仍可采集；验收仍以实际游戏画面和取得物品证据为准。第一条完整路线的资源可用性、地图数据和位置误差阈值在实现前确认。

## 兼容分层与能力门槛

1. **导入与预检**：从手机文件选择器读取本地路线原文件，展示名称、地图、有序点位和全部所需能力；给出合法且当前可执行、合法但能力未齐、格式错误三类结果。此阶段不发出游戏输入，不要求用户先录制 Paimon 专用路线。
2. **坐标定位**：先验证 Teyvat 基础场景的屏幕定位与游戏坐标对应。显示当前位置、目标点和可信状态；失去定位时不盲走，也不由玩家在执行中标注。
3. **按点移动与传送**：以导入路线驱动 path / target / orientation 导航；自动处理路线所需的 teleport 起点，通过地图画面确认传送完成再继续。地面导航和自动传送可在坐标定位完成后分别推进。
4. **到点拾取**：依据路线的精确目标点或已支持的采集动作，到点后调整视角、识别真实拾取提示并确认结果；资源缺失或仅路过不计为拾取成功。08 先使用预检通过的基础路线，09 将导入、传送、导航和拾取整合为完整任务。
5. **扩展与验收**：按能力矩阵扩展移动模式和动作，整合多资源点、有界恢复和诊断；不宣称已兼容整仓任意路线或 BetterGI 的全部运行时。

能力矩阵最初全部按实现证据标记：格式读取、坐标定位、walk/path/target/orientation、teleport、各 move_mode、各 action、地图支持分别列状态。每次启动前检查整条路线的要求；存在不支持的必需行为时，导入仍可预览，但整条任务不得开始。不能等运行到危险点才首次发现必需动作不支持，也不能自动截断路线或让玩家补操作。

基础路线把自动传送纳入计划；fly、stop_flying、技能采集、战斗、采矿等先识别并报告，执行实现独立扩展。原有不自动战斗、不执行对话/宝箱/调查的边界继续适用。需要这些行为的路线在能力齐备前不能作为首版成功样本。

## 测试与后续开发入口

沿用完整采集任务的主要测试边界，路线导入和预检作为执行前准备的一部分，从应用导入入口检查预览、排序、缺省规则、字段保留及拒绝原因；后续仍从同一个任务入口验证游戏侧结果。不要只测试 JSON 能反序列化就判为兼容通过。

准备固定版本真实样本的回归集合，覆盖负数/小数坐标、重复点、不同作者字段、无动作、传送、朝向、飞行及动作参数，再补格式错误、空点列、非有限坐标和未知行为。路线下载和样本更新属于准备阶段；已经导入的路线在手机上离线解析与执行，不要求运行时联网。

执行测试必须验证路线要求的点位语义、传送后定位、当前点完成证据和有序推进。导入通过、07 移动通过、定位通过和整条采集通过分别记录，不能互相替代。实现与验收顺序见[开发方案](auto-pickup-design.md)。
