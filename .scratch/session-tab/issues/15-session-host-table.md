# 15: 宿主表与前台指针：一场会话与今天等价

Type: implement
Status: in-progress
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

## 进度

**2026-10-12 · 机制层已落地（这一票尚未 done）**：`src/session/host.rs` +
`tests/session_host.rs`（6 条）。已覆盖：

- `SessionState` 三态、`SessionCommand`（`Turn` / `Cancel` / `Close`）、`SessionTurn`
  （`Prompt` / `Skill` / `Injected`）。
- `SessionHost::adopt`：把 `Harness` 交给一个 `tokio::spawn` 的 task，记下
  `id` / `cwd` / `state: watch` / `cmd: mpsc` / `cancel: CancelSignal` / `join: JoinHandle`；
  同一 `SessionId` 第二次入表被拒（`HostError::AlreadyHosted`）。
- 表级 `foreground: watch<SessionId>`：`switch` 只改指针（忙闲不阻挡、不卸载任何一场），
  `follow_foreground` 给 task 一份接收端。
- `running()` 数的是 `RunningTurn` 的场数；`close` 发 `Close`、await task、从表移除。
- 6 条用例：同 id 只能一行、回合在 task 里跑完并落到**那一场自己的流**上、永不结束的流把状态
  停在 `RunningTurn`（取消后回 `Idle`、读数归零）、切换只动指针、关闭真的少一行且 task 结束、
  两场各跑各的流不串。

**2026-10-12 · 动作为解释器 + 前台快照（接线的前置）**：

- **`SessionActor` trait**：宿主自己认识 `Cancel` / `Close`，其余命令交给一个注入的盒子办 ——
  会话级手势（换模型、装技能、撤销、记命令、跑讨论、目标循环）要 `Config` / `ConsoleHandle` /
  `GoalSetup` 这些**进程级**的东西，而库层不认识它们。缺省实现是 `TurnActor`（只跑回合），
  也就是单会话今天的行为；cli 那一侧会换上一个更全的。状态转换仍由宿主管（它持 `watch` 的
  发送端），解释器只负责把命令办完。
- **`Harness::front_facts()` → `FrontFacts`**：主循环解析每一次提交要的那几样（id / 模型 /
  档位 / 技能名 / 目标名）的只读快照。会话的 `Harness` 交给 task 之后主循环就问不到它了，
  于是装好取一份、`/model` 之后重建一份。
- 新增 2 条用例：命令真的送到 `adopt_with` 给的那个解释器手里（且它拿到的 harness 就是那一
  场）、前台快照的字段对得上。

**还没做**（这一票剩下的部分，也是最大的一块）：

- **cli 接线**：`interactive_loop(&mut harness, ...)` 现在直接持有 `Harness`（45 处调用），
  要改成「组装 → `adopt` → 主循环发命令、读状态」。`run_one_turn` / `run_goal_loop` /
  `open_picker` 三处跟着改形状；`Submission` 的解析留在主循环，只有「跑一个回合」那一半变成
  `SessionCommand::Turn`。
- **命令的形状**：票 04 §3 写的是 `Submit(Submission)`，而 `Submission` 借用 CLI 的字符串。
  机制层先用与它等价的 `SessionTurn`（owned）—— 接线时把 `Submission` 的每个分支映射过来。
- `/clear` 与阈值 rollover 从「换一场」变成「新建一场 + 入表 + 切前台」；`clear_while_looping`
  闸门取消。
- 权限策略「一场一份」（现状已经是 per-`assemble`，接线时钉一条断言即可）。
- `RenderEvent::SessionState`（状态变更点推给渲染器，票 22 的标记列读它）。
