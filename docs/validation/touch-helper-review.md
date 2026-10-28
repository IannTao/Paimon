# 单流触屏 helper 实现审查

日期：2026-10-28。固定基线：`36553d5696bd9f82db4e2b107182f6b2253f0061`；初审提交：`2241f48`。比较命令：`git diff 36553d5...HEAD`。基线是本轮实现前已有原型的首次仓库快照，`.scratch` 中的本地材料没有加入提交。

规格：[输入方案研究](input-coexistence-research.md)。本轮按独立限时 helper 范围实现，不接 APK、不执行真机创建或抓取。依照 code-review 技能，由两个独立子代理分别执行 Standards / Spec 审查，并复核修复。

## Standards

规范硬违规：未发现。实现符合两份 ADR 的 Rust 核心、同机系统输入边界，文档保留了未验证结论。

初审发现 3 项：

1. **P1 正确性：尾部报告可能丢失。** supervisor 在 `try_wait()` 得到 worker 退出时直接返回，输出线程可能尚未转发 `destroyed` / `closed`，随后整个进程退出。已增加输出排空状态，正常返回前等待排空，等待仍受心跳和硬期限限制；输出异常单独报告。新增进程级末尾报告回归测试。
2. **Mysterious Name / Primitive Obsession（判断项）。** Rust 中直接使用 `Event::new(3, 47, ...)`、`48..=61` 等数字隐藏 MT 协议含义。已改用具名 Linux UAPI 常量，测试仍保留独立数字作为协议预期。
3. **Repeated Switches / Primitive Obsession（判断项）。** CLI 与设备循环反复比较阶段字符串。已改为 `Mode` 枚举，并集中表达是否转发。

复核结论：以上 3 项全部处理，当前 Standards 未解决发现为 0。

## Spec

初审发现 3 项：

1. **P1 停止信号冻结真实输入。** 规格要求“转发仍健康时继续保留真实手指流，待触点全部抬起后正常交接”。原 `ph_poll` 在收到停止信号后直接返回停止位，不再报告物理事件。已将停止位与真实输入就绪位合并。
2. **P1 读取配额耗尽可能误判空闲。** 规格要求“正常退出也应在真实触点全部抬起时释放”。每轮读 256 个事件后，只看已转发状态不能证明队列后面没有新的 DOWN。已增加内核队列、实时 MT 槽 / BTN_TOUCH、再次检查队列的交接条件。
3. **P2 工具坐标轴范围未核对。** 规格要求核对“轴范围、分辨率与坐标变换”。将 POSITION_X/Y 直接写入 TOOL_X/Y 前，现已要求对应原始范围一致；不一致设备明确拒绝。

复核又发现 2 个补全项，均已修复：grab-only 阶段现在会有界消费但不转发物理历史，防止队列永久非空；停止信号中断健康 uinput 写入时继续重试 EINTR，不把停止请求误判为转发失效。

同时补充 BTN_TOUCH 与真实 MT 槽的一致性校验；不完整状态禁止继续合成。相应测试已经历失败再通过。

复核结论：原问题与补全项全部关闭，未发现新增阻塞；没有越出本轮范围的功能。当前 Spec 未解决发现为 0。

## 最终验证

- `cargo test --workspace --locked --offline`：23 项通过；4 个 ignored 项是父测试实际启动的子进程夹具。
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`：通过。
- `cargo clippy -p paimon-touch-helper --target aarch64-linux-android --locked --offline -- -D warnings`：通过。
- `cargo fmt --all -- --check`、`git diff --check`：通过。
- `scripts/build-touch-helper.ps1`：NDK 27.0.12077973 / API 34 release 构建通过，C 适配层启用 `-Wall -Wextra -Werror`；readelf 确认为 AArch64 ELF64。

未运行真机创建 / grab / 转发 / 合成、显示映射或游戏拾取实验；无 Android 原生设备集成测试证据。停止与交接的 native 修复经过编译及静态复核，仍需后续真机阶段验证。操作步骤与限制见 [helper 说明](touch-helper.md)。原有工单 01 的 shell tap 失败门槛保持不变。

Standards：共 3 项初审发现，最严重为尾部报告丢失，均已修复；Spec：共 5 项初审及复核发现，最严重为停止信号冻结转发及过早交接，均已修复。
