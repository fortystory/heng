# 15: 宿主表与前台指针：一场会话与今天等价

Type: implement
Status: ready-for-agent
Blocked by: 13
Part of: ../map.md

**要打通的行为**：进程里跑的那一场会话有了一个**宿主**（`Harness` 就是它）、一张**宿主表**与一个**全进程前台指针**；而用户看到的界面与行为**与今天一模一样**。`/clear` 与阈值 rollover 从「换掉这一场」变成「新建一场、旧的留在表里」。

**来源**：[spec.md](../spec.md) §1、§5、§10。

## 验收

- [ ] 一场会话 = 一个 `Harness` + 一个拥有它的 tokio task；启动组装 → 入表 → 前台指针指向它 → 主循环从表里驱动它
- [ ] 宿主表每行 = `id` / `cwd` / `state: watch<SessionState>` / `cmd: mpsc<SessionCommand>` / `CancelSignal` / `JoinHandle`；命令只有 `Submit` / `Cancel` / `Close`
- [ ] 表级 `foreground: watch<SessionId>`；**切换只是改它，忙闲不阻挡**
- [ ] 同一 `SessionId` 在表里只有一行（有断言或测试）
- [ ] `/clear` 与阈值 rollover 走「新建一场 + 入表 + 切前台」；**旧的留表里、不写收尾事件**
- [ ] `clear_while_looping` 闸门取消：loop 在跑也能新建，旧会话连它在跑的 loop 一起留后台
- [ ] 权限策略从脚手架级共享变成**一场一份**（沙箱探测 / 技能 / 工具表 / `AGENTS.md` 同理，按票 04 §1）
- [ ] `tests/goal_loop.rs`、`tests/e2e_single_turn.rs`、`tests/modes.rs`、`tests/cancellation.rs` 全绿；新增宿主接缝测试（假 provider）覆盖「入表 / 取回 / 状态三态流转」
