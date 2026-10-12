# 27: 出口提示按新语义拆开，并跟着眼前那一场走

Type: implement
Status: done
Blocked by: 22, 23
Part of: ../map.md

**要打通的行为**：提示行末尾那个出口条目说**键盘现在会做什么** —— 票 23 把 `Ctrl-C` 与 `Ctrl-D`
拆开之后，它得跟着拆：`Ctrl-C` 取消、`Ctrl-D` 退出；空闲时只有 `Ctrl-D` 有事可做。而它说哪一档，
判据是**眼前这一场**的忙闲，不是整个进程的。

**来源**：维护者报告（2026-10-12）；[spec.md](../spec.md) §1；决策票 [`04`](04-session-host-and-concurrency.md) §3、票 [`23`](23-exit-gate-and-ctrl-c.md)。

## 验收

- [ ] 空闲档 `ctrl-d 退出`，忙碌档 `ctrl-c 取消 · ctrl-d 退出`；空闲档**不宣传 `Ctrl-C`**
- [ ] 「忙」取自**眼前这一场**的会话三态（`Idle` 以外都算，含 `AwaitingHuman`）与循环推来的
      一次运行边沿两者的并 —— 切到后台正在跑的那一场，提示换成忙碌档，切回来换回去
- [ ] 出口是**预留**的：窄到只剩它时两档都还在，位置不变
- [ ] 主路径上「已经举手」那两档（`再按一次 ctrl-c/ctrl-d 退出` / `已取消 · 再按一次 ctrl-c 退出`）
      **从出口表里退役** —— 那把举手现在只活在历史重放
- [ ] 阶梯的硬编码宽度重算（出口短了九列、忙碌档长了十四列）
- [ ] 测试：两档文案、切场跟随、阶梯各档

## 实现记录（2026-10-12）

- **症状**：状态行与提示行一直写着旧语义 —— 空闲是 `ctrl-c/ctrl-d 退出`（并排两个键）、忙碌是
  `ctrl-c 退出`（而那时 `Ctrl-C` 其实是取消）。于是读的人按着一个写着「退出」的键，得到的是
  「heng: 这一场现在没有在跑的东西」（维护者报告的第 2、3 条：提示没拆开、按了像是坏了）。
- **`busy()` 改成并集**：`self.running || self.states.contains_key(&self.front)`。
  两个来源是**并起来**的，不是二选一 —— `RunState` 盖得住**不经过宿主表**的运行（一次性的
  `discuss` 没有 task，发不出会话三态），而三态那半分得清「切过去那一场正跑着、而这一场没有」。
  `conversation_live` 的等待提示同样改读 `busy()`，否则切场后等待行会跟着消失。
- **举手的两档退役**：`exit_hint` 收成单参数 `exit_hint(busy)`，`status_line*` / `trace_status_line*`
  / `viewer_status_line*` 一并去掉 `raised`。`EXIT_HINT_REPLAY_RAISED` 留着（`replay_key` 里
  `Ctrl-C` 仍走双击，那是重放唯一的出路）。这清掉了票 23 欠账里的「出口段那几行」。
- 测试：`tests/wording.rs` 重写两条阶梯断言 + 新增 `the_exit_hint_separates_cancelling_from_exiting`；
  `tests/session_switch.rs` 新增 `the_way_out_follows_the_session_that_is_in_front`；20 处用
  `RunState { running: true }` 表达忙碌的测试改成 `SessionState { RunningTurn }`（那才是会话级
  事实）；`src/render/tui.rs` 三条目标循环 Esc 测试走同一条产品路径。

## 欠账

- **问卷里的退出回执还在**：页脚那句「已取消 · 再按一次 ctrl-c 退出」（`questionnaire_exit_raised`）
  与 `docs/render.md` 的问卷一节仍按「`Ctrl-C` 举手退出」写。票 23 拆的是**主路径**那一把，
  问卷这条还没定 —— 归票 09 的文档回改一起处理，或另记一张。
- 真机走查（切场时提示跟着换、空闲按 `Ctrl-C` 的那句回话）记在票 09 的清单里。