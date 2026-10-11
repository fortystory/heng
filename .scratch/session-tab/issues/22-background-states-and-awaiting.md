# 22: 后台三态与待答挂住

Type: implement
Status: ready-for-agent
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
