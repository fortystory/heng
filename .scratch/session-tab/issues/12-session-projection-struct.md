# 12: 把 TuiState 的会话投影抽成子结构

Type: implement
Status: done
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

## 实现记录

- `pub struct View` 落在 `src/render/tui.rs`（没有新开模块：它和 `TuiState` 同生共死，多一条
  `mod` 只是把两个必须一起读的类型拆到两处）。字段 = 构造清单里投影那 39 项，加上轨迹折叠 /
  搜索 / 索引那 5 项（`folded` / `fold_before_all` / `fold_handles` / `trace_search` /
  `trace_index`）—— 判据是「重放这一场的事件流能不能重建出它」。
- `View::new(facts)` 自己算发言者配色；`View::apply(event)` 收下 `apply` 的投影部分。
  `TuiState::apply` 留在本体，只做两件投影外面的事：「工作区变了」那条静默信号（两个
  `*_wanted`）与整帧的 `dirty`，然后转调 `View::apply`。
- 47 个重建路径上的方法跟着搬进 `impl View`（`View::apply` 要调它们）；它们原来置
  `self.dirty` 的两处改成由 `TuiState` 那 5 个调用点自己置 —— `dirty` 是整帧的账，不是投影的。
  为搬它们还引入了 `trace_search` / `trace_index` 与闭包里的 `is_preamble`。
- 判据之外的一处例外：`trace_tier_width`（轨迹页自己的宽度档）曾试过放本体，但
  `prefix_style_of` / `targets` 这两条投影路径要读它，跨块借用会把签名撑开；它随绘制重算、
  也随投影走，于是留在 `View`。
- 行为零变化：`render_layout` / `render_tui` / `render_timeline` / `todo` / `render_changes`
  一条断言都没动。
