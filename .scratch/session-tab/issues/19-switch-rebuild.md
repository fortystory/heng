# 19: 切换重建：`SwitchSession` 与 `seq` 补齐

Type: implement
Status: done
Blocked by: 12, 14
Part of: ../map.md

**要打通的行为**：给渲染器一条「换到这一场」的指令，屏幕上就整块变成目标会话的内容；切回来的会话在后台期间产生的东西**补上、且不重不漏**；切换期间屏幕不残留上一场的任何状态。

**来源**：[spec.md](../spec.md) §6；决策票 [`08-front-view-isolation.md`](08-front-view-isolation.md)。

## 验收

- [ ] `ConsoleRequest` 加变体（会话事实 + 该会话的事件流**快照**）；**启动那条一次性重放被收编成同一个机制的特例**（`TuiOptions.reopened` 门闩被吸收）
- [ ] 投影**整块换**（复用 `12` 的子结构）；`facts` 与发言者配色整份换
- [ ] 会话级轻状态另存一张表（键 = `SessionId`）：草稿、两个 pane 的视口意图（`follow` / `holding` / `top_source`）、转录滚动位置、浮层（详情 / 文件查看器 / picker / `/` 菜单 / 轨迹搜索与折叠）；切换 = 先收走当前那份、再装入目标那份
- [ ] **页签选择、主列页、`Ctrl-O` 意愿、会话页过滤态**不在这张表里（进程级）
- [ ] 补齐：以快照最大 `seq` 为界，收尾只 apply `seq > S`（与 `14` 的 `live_buffer` 改动合起来，保证不重不漏）
- [ ] 复用重放视觉（逐批预算照旧），**不插**「以下是历史」分隔行；末尾回到底部
- [ ] 在飞的旧重建先收尾再开新的
- [ ] 重建**必须走 `apply`**（`links` / `thinking_*` / `colors` / 命中索引 / `painted` / `block_at` / `timeline` / `panel` / `todo` / `turn_*` 都是它的副作用）
- [ ] 测试（`ConsoleRequest` 注入接缝，照 `tests/history_replay.rs`）：连切两场后帧里只有第二场的内容；切换期间注入的实时事件在收尾时补上且不重复；轻状态换入换出（草稿与滚动位置切回还在）

## 实现记录

- **`ConsoleRequest::SwitchSession { facts, events }`**（+ `ConsoleHandle::switch_session`）：带的是
  那一场的**会话事实**与它的事件流**快照**。plain 前端忽略它（一个进程只跑一场）。
- **切换 = 四步**：在飞的旧重建先收尾（它缓冲的实时事件属于上一场）→ `stash_light()` 把当前
  轻状态收进表 → `self.view = View::new(facts)`（投影**整块换**，配色由 `View::new` 自己算）→
  `load_light()` 装入目标那一份 → 清 `live_buffer` → 若快照非空就开一条 `Replay`
  （`divider: false`）。
- **会话级轻状态表** `HashMap<SessionId, SessionLight>`：草稿（整份 `Input`）、两个窗格的
  **视口意图**（`pane::Viewport { top, top_source, follow, holding }`，`Pane` 多了
  `viewport()` / `restore_viewport()`）、详情覆盖层、文件查看器、选择器、`/` 菜单高亮、轨迹搜索、
  折叠两件套、轨迹索引与选中块。页签选择、主列页、`Ctrl-O` 意愿与会话页过滤态**不在**表里
  （进程级）。
- **`Replay` 多一个 `divider: bool`**：启动那次插「以下是历史」那条接缝，**切换不插** —— 切过去
  看到的是一整场会话，不是「历史 + 本段」。其余照旧（逐批预算、收尾 `to_bottom`、只 apply
  `seq > S`）。
- **`TuiOptions.reopened` 保留**，票面原写「门闩被吸收」：这里的落地是「启动与切换**共用**同一套
  `Replay` 机制」（切换只是 `divider: false` 的那个特例），而门闩本身管的仍是「启动时先收快照、
  在此之前一个渲染事件都不许画」—— 快照走 `ConsoleRequest` 是异步到达的，所以那道闩不能去掉。
- 新增 `tests/session_switch.rs` 5 条：连切两场后只剩第二场的内容、切换不插接缝、草稿换入换出、
  切换期间到达的实时事件在收尾时补上且不重复、上一场的实时事件不泄过来。
