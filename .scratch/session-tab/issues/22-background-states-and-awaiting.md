# 22: 后台三态与待答挂住

Type: implement
Status: done
Blocked by: 20
Part of: ../map.md

**要打通的行为**：切走的那几场在列表上看得见状态 —— 在跑、在等人答、空闲；跑出需要人答的东西时它**停在那一步**，切过去时问题原样弹出来；**永不自动作答**。

**来源**：[spec.md](../spec.md) §1、§3；决策票 [`04`](04-session-host-and-concurrency.md) §3、§4、§6。

## 验收

- [ ] 每场一个 `watch<SessionState>`（`Idle` / `RunningTurn` / `AwaitingHuman`），保最新不丢；每个状态变更点另往渲染通道发一条带 session id 的状态事件
- [ ] 列表标记：前台 `▍`、待答 `?`+`WARN`、活跃 `●`+`PLAIN`、空闲留空；页顶读数 `N 场 · M 场在跑`（`M` = `RunningTurn` 的场数）
- [ ] asker 按会话包装（持 `SessionId` + 读前台指针）：**不是前台**就不往前端送请求、把 future 挂在 `watch` 上并置 `AwaitingHuman`；**是前台**才送进既有的请求通道
- [ ] **切到前台即唤醒**：请求从没被消费过 → 切过去原样弹出，前端不需要记 N 个悬浮请求
- [ ] 回答走既有的 reply 通道，**不经宿主表命令通道**（宿主表不代答）
- [ ] 多场同时待答：各挂各的，谁切到前台谁弹
- [ ] 后台会话的「需人答」**永不自动作答**
- [ ] 测试（宿主接缝）：后台跑出询问 → 状态变待答且请求没被消费；切前台 → 请求弹出；作答 → 回到 `RunningTurn`


## 进度（2026-10-12）

**已落地（渲染侧那一半）**：

- **`RenderEvent::SessionState { session, state }`**：带 session id 的第三态事件。四类消费者各
  自表态 —— TUI 收下、headless 与 transcript 忽略（忙闲是界面的事）、`event_at` 给它「现在」。
- **`ConsoleRequest::SessionState { id, state }`** 与 `HostCount { sessions, running }`：
  `TuiState` 按**会话 id** 存三态（切走切回都还在），列表标记与页顶读数读它。
- **列表标记**：内存三态优先（`?`+`WARN` / `●`+`PLAIN` / 留空），**没报过的退回列表自己那份**
  （store 扫出来的「上次没跑完」仍有意义）；前台 `▍` 压过一切。
- **页顶读数** `N 场 · M 场在跑`：`M` 取宿主表的 `running()`。
- **推送**：主循环每 80ms 把宿主表里每一场的 `watch<SessionState>` 读一遍，变了才发一条
  ——**轮询**而不是在变更点推送，因为变更点散在宿主表 task、应答者与循环三处，而主循环这里能
  看到全部。

**还没做（这一票的核心）**：**asker 按会话包装**（`SessionAsker`）—— 不是前台就不往前端送请求、
把 future 挂在 `foreground` 的 `watch` 上并置 `AwaitingHuman`，切到前台再原样送出去（请求从没
被消费过）。它是「后台那一场跑出询问时停在原地、永不自动作答」这句话的实现，也是「切到前台即
唤醒」的实现。留待下一轮。

## 收口（2026-10-12）：按会话包装的应答者

`SessionAsker`（`src/session/asker.rs`）—— 票 22 的核心那一半落地：

- 它知道自己属于哪一场，也持一份**前台指针的订阅**（`SessionHost::subscribe_foreground`）。
  **不是前台就不往前端送请求** —— 那会把一场后台会话的问题弹到另一场的脸上；它把 future 挂在
  `watch::for` 上、把这一场置成 `AwaitingHuman`。
- **切到前台即唤醒**：请求**从没被消费过**，所以前端不需要记 N 个悬浮请求 —— 谁切到前台谁弹
  自己那一个。唤醒后状态回到 `RunningTurn`，回答仍然走既有的 reply 通道（宿主表不代答）。
- 三态的发送端是一个**槽**：应答者在组装期就建好了，而那一对通道由宿主表在 `adopt` 里创建。
  调用方在入表之后用 `SessionAsker::attach` 填进去（`HostEntry` 因此多暴露一个 `state_tx`）。
- 第一场与每一条装载路径（`open_session`、`open_new`）都用 `AssemblyKit::session_asker`
  按 id 现包一个 —— **多场同时待答时各挂各的**。
- 测试：`tests/session_asker.rs` 2 条 —— 后台发问挂住（问题没到前端）、切前台唤醒并回到在跑；
  前台的询问直接穿过、不进「待答」那一档。

**欠账**：状态行还没有把 `AwaitingHuman` 写成一句话（列表上的 `?` 有）；那一条与票 25 的终端
标题一起做更合适。
