# 20: 点一行切过去（接入）

Type: implement
Status: done
Blocked by: 17, 18, 19
Part of: ../map.md

**要打通的行为**：会话页上**点一条就切过去** —— 已经在宿主表里的零延迟取回、不在的就装载一场；切换期间原会话照跑；切回来时它的现场（草稿、滚动、浮层）还在。这是第一条端到端的「多会话」用户路径。

**来源**：[spec.md](../spec.md) §3、§6；决策票 [`04`](04-session-host-and-concurrency.md) §2、[`06`](06-session-page-shape.md) §7、[`08`](08-front-view-isolation.md) §6。

## 验收

- [ ] 点击任一行：已在宿主表 → 取回并切前台；不在 → 装起来（`18`）再切前台；两种都发一条「换到这一场」（`19`）
- [ ] 单击 = 交键盘 + 落焦点 + **能切就立刻切**；点当前前台那一行只做前两步
- [ ] 切不了的两行（cwd 目录不存在 / 流损坏）不切、焦点照落、各回一句提示行
- [ ] 装载期间提示行一句；**不新增列表标记态**
- [ ] 切换**不因为目标正在跑回合而阻挡**；切走的那一场继续跑
- [ ] 会话页焦点按**会话 id** 保位（重扫后按 id 找回，不按下标）
- [ ] 端到端验证：从 A 点 B 之后主列是 B、状态行是 B 的模型与档位、A 仍在宿主表里继续跑；点回 A 时它的草稿与滚动位置还在
- [ ] 宿主接缝 + `ConsoleRequest` 注入接缝的联调测试各一条

## 进度

**2026-10-12 · 上报路径就位，但这一票卡在一个架构前置上。**

已落地（无行为变化的部分）：

- `FrontEndEvent::OpenSession { id }`：点会话页上**别的**那一行时上报一条；点当前前台那一行
  只做「交键盘 + 落焦点」两步（验收的第 2 条已满足）。切不了的两行由循环侧回答，渲染层不替
  它决定。
- 循环侧收到它暂时忽略（`=> {}`）—— 所以**行为与票 17 落地时一样**。

**卡在哪**：票 15 把整段交互循环搬进了会话自己的 task，而**前端事件接收端
（`ConsoleEvents`）是单一消费者**。切到第二场之后，那一场的循环拿不到它 —— 没有人读输入。
换句话说：「谁读输入」现在绑在「哪一场的循环在跑」上，而切换正要把这件事挪走。

**出路**（下一轮的第一步）：把**输入段**与**执行段**分开 ——

- 主循环持 `ConsoleEvents` 与 `console.prompt()`，解析出 `Submission` 之后**发命令**；
- task 侧只执行（这正是决策票 04 §3 的 `Submit(Submission)` 本意，票 15 当时按「整段搬」走了
  另一条路）；
- 运行中的手势由主循环**转发**成命令（它独占那个接收端）；
- 于是每一场都是「只执行」的，而输入只有一处 —— 切换前台就是换一个命令的收件人。

**2026-10-12 · 架构前置已解决**：输入段从会话的 task 搬回了主循环。

- 主循环持 `ConsoleEvents` 与 `console.prompt()`，解析出 `Submission` 之后发命令；task 侧只
  执行 —— 这正是决策票 04 §3 的 `Submit(Submission)` 本意（票 15 当时按「整段搬」走了另一条
  路，这一轮改回来）。
- `CliCommand { Submit { submission, raw }, Gesture { event, running }, ResumeGoal, Close }`；
  `CliActor` 只剩 `execute`（原 `match submission` 那段）/ `gesture`（原 `events.recv()` 那几支）/
  `resume_goal`（原「恢复（§10）」那一段）三个入口。
- `run_one_turn` / `discuss_in_session` / `run_goal_loop` 改为从命令通道收**运行中的手势**
  （`CliCommand::Gesture`），不再吃 `ConsoleEvents`。
- **切换前台于是只是「换一个命令的收件人」** —— 每一场都只执行，输入只有一处。
- 行为与拆分前一致：`Submission::Quit` 仍走 `ExitCode::SUCCESS`，举手退出仍走 `quit.code()`。

**这一票剩下**：`open_session`（三条结局：已在表里取回 / 装载后切 / 拒绝）、装载那条路要的
`AssemblyKit`、以及「切换不因为目标在跑回合而阻挡」的联调测试。

## 实现记录（2026-10-12）

- **`CliRequest::Open(id)` + `ClipActor.requests`**：点击发生在会话的 task 里，而宿主表在主循环
  手里 —— 于是上报经一条请求通道交给主循环处理（票 17 那条 `FrontEndEvent::OpenSession` 到
  这里闭合）。
- **`AssemblyKit`**：装载另一场要的那批**可重复取用**的材料（配置 / 模型 / 发言者名 / store /
  锁表 / 渲染端 / asker / 问卷 / MCP / home / 模式 / 目标环境），外加 `load`（走票 18 那条共用
  路径）与 `ui_facts`（拼渲染侧那份会话事实：模型与权限模式从流上读，读不出才落回启动那套）。
- **`open_session` 三条结局**：已在宿主表 —— 只改指针（零延迟、忙闲不阻挡）；不在 —— 先走
  `attach::check`（cwd 不在 / 流读不出各回一句提示行），通过则「正在装载…」一句 + `load` +
  入表 + 切前台；两条路都发一条 `ConsoleRequest::SwitchSession`（票 19 的重建）。
- 装载起来的那一场用同一个 `requests` 发送端，所以它自己也能再切走。
- **顺带修掉宿主表的一个真 bug**：`watch::Sender::send` 在**一个接收者都没有**时是失败的 ——
  而 `SessionHost::new` 把接收端丢掉了，于是「没有 task 订阅时 `switch` 静默无效」。这与
  `forward_gesture`（按前台指针分派手势）正面冲突。现在表自己保住一个接收端。
- 测试：`tests/session_host.rs` 新增 1 条（装载第二场 → 入表 → 切前台，旧的留表里），加上既有的
  11 条宿主接缝与 `tests/session_switch.rs` 5 条注入接缝。

**欠账（写在这里）**：cli 的 `open_session` **逐行路径**没有自动化覆盖 —— 它要真终端与 API key。
两条接缝由 `session_host.rs`（装载 + 切前台）与 `session_switch.rs`（注入 + 重建）分头覆盖，
真机走查记在票 09 的清单里。
