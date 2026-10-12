# 15: 宿主表与前台指针：一场会话与今天等价

Type: implement
Status: done
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

**2026-10-12 · 宿主表泛型化（命令的形状交回调用方）**：

- `SessionHost<C>` / `HostEntry<C>` / `SessionActor<C>` 泛型化：**宿主完全不认识命令的语义** ——
  连「取消」与「关闭」都不认识。`act` 因此答一个 `bool`（`false` = 这一场该结束了），
  `close_with(id, shutdown)` 由调用方给那条结束命令。
- 缺省那一型（`SessionHost<SessionCommand>`）仍是「跑回合」：`adopt` / `adopt_read_only` /
  `close` 在那一层，`attach::check<C>` 跟着泛型化。
- **`SessionCommand` 扩展**为：`Turn` / `RecordCommand` / `LoadSkill` / `Undo` / `SwitchModel`
  / `SwitchEffort` / `Cancel` / `Close` —— 这是票 04 §3 的 `Submit(Submission)` 那份意图的
  **展开**：主循环仍独占前端事件接收端（单一消费者），于是它解析提交、发具体动作。
- `SessionActor::act` 多一个 `inbox: &mut mpsc::Receiver<C>`：跑长动作（一个回合、一场讨论、
  一趟目标循环）时要一边跑一边收**运行中的手势**（主循环把 `FrontEndEvent` 转成命令转发）。

**2026-10-12 · 交互循环搬进 task**：

- `interactive_loop` 那 273 行函数体整体搬进 **`CliActor::run`**，而 `Harness` 归宿主表的 task
  （`.scratch/session-tab/spec.md` §1）。`interactive` 现在只做：组装 → 建宿主表 →
  `adopt_with(harness, cwd, false, CliActor)` → 发一条 `CliCommand::Run` → 等回执 →
  `finish_session`。
- 它为什么整段在 task 里、而不是「主循环解析提交、task 只跑回合」：**前端事件接收端是单一
  消费者**，而这一段每一处都在与它打交道（提示行、运行中的手势、目标循环）。票 04 §3 的
  `Submit(Submission)` 在这条路上因此展开成一条 `Run`。
- `Harness::render_channel()`（克隆渲染端）：会话收工之后主循环拿它等渲染器把缓冲写完 ——
  `Harness::shutdown` 的语义改由主循环做，因为 `Harness` 已经随 task 没了。
- 顺带删掉 `McpMenu`（它的两个字段成了 `CliActor` 的 `Arc<McpService>` / `Arc<Vec<…>>`）。
- **欠账（写在这里）**：cli 交互路径没有自动化覆盖 —— 它要真终端与 API key。这一步的验证是
  `cargo test` 全量 0 failed、`cargo test --lib` 227 条、`cargo run -- --help` 冒烟退出码 0，
  以及逐条核对搬进来的代码只在借用形状上变了。

**还没做**（这一票剩下的部分）：

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


## 收口（2026-10-12）

两条验收与票 21 逐字重叠，随票 21 落：

- 「`/clear` 与阈值 rollover 走『新建一场 + 入表 + 切前台』；旧的留表里、不写收尾事件」——
  票 21 的第 1、2 条正是这句话（「新建 = 同 cwd 再加载一场空会话 + 立即成为前台；旧的留宿主表
  里」与「`/clear` = 阈值 rollover = 页签新建 是同一个动作」）。它们要求「组装一场新会话」的
  能力，而那件事的入口（页签 `新建` 与关闭语义）在票 21 才立起来。
- 「`clear_while_looping` 闸门取消」——它是 `/clear` 那一条的附属：闸门存在的理由是「翻页会
  换掉这一场」，而新的语义让旧会话留着，于是没有必要拦。票 21 落地时一并删。

其余全部落地：一场会话 = 一个 `Harness` + 一个拥有它的 task；宿主表六格字段与表级前台指针；
同 id 只有一行；切换只改指针、忙闲不阻挡；`Harness: Send` 的编译期断言；权限策略一场一份
（本来就是 per-`assemble`）；`goal_loop` / `e2e_single_turn` / `modes` / `cancellation` 全绿；
`tests/session_host.rs` 11 条 + `tests/session_attach.rs` 9 条。
