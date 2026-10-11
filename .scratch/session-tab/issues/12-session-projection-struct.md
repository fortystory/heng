# 12: 把 TuiState 的会话投影抽成子结构

Type: implement
Status: ready-for-agent
Blocked by: —
Part of: ../map.md

**要打通的行为**：渲染器里「这一场会话的投影」与「与哪一场无关的进程状态」分成两块，切换会话时投影可以被**整块换掉**。这是纯重构 —— 用户看到的东西一个像素都不变。

**为什么先做**：切换重建要换的正是投影那一块，而现在 `TuiState` 的 98 个字段里**有 10 个投影字段根本没有清空入口**（`transcript` 无 `clear`、`painted` 用 `mem::take` 保内容、`timeline` 轴域只增不减、`block_at` 以 `BlockId` 为键…）。靠人工 reset 清单必漏，靠类型分块才守得住。

**来源**：[spec.md](../spec.md) §6 与 §10。

## 验收

- [ ] 会话投影住进一个子结构，形状 = 今天构造清单里投影那部分；它自带 `new(facts)` 与 `apply(event)`
- [ ] 进程级字段（左栏页签选择、主列页、`Ctrl-O` 意愿、举手槽、脉冲、宽度档、`area` / `dirty` / `quit`）留在 `TuiState` 本体，不参与切换
- [ ] `facts` 与发言者配色在子结构里，可整份换
- [ ] **行为零变化**：`tests/render_layout.rs`、`tests/render_tui.rs`、`tests/render_timeline.rs`、`tests/todo.rs`、`tests/render_changes.rs` 全绿，无一处断言需要改
