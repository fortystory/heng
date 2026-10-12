# 31: 切换前台后，会话页的前台标记跟着走

Type: implement
Status: done
Blocked by: 20, 29
Part of: ../map.md

**要打通的行为**：点了会话页上别的一行之后，那一行**自己**变成前台该有的样子 —— 标记列的
`▍`、标题的 `LIVE` 色都落在新前台那一行上，旧前台退回 `HOSTED`。切完还指着启动那一场，
人就没法从屏幕上判断「现在在哪一场」。

**来源**：维护者复测（2026-10-12：「为什么我切换别的会话时，当前会话没有变？」）；[CONTEXT.md
的**前台**词条](../../CONTEXT.md)；票 [`20`](20-click-to-switch.md)、[`29`](29-session-titles-say-where-a-session-is.md)。

## 验收

- [x] 切换之后 `▍` 落在新前台那一行，旧前台不再带它
- [x] 标题色跟着换：新前台 `LIVE`、旧前台 `HOSTED`（仍在宿主表里时）
- [x] 真机复验：点第一行 → 第二行 → 第三行，`▍` 依次在第 11 / 12 / 13 行
- [x] 测试：`tests/session_page.rs` 新增一条（列表快照不动、只换 `SwitchSession` 时的标记归属）

## 实现记录（2026-10-12）

**根因**：会话页那一行的 `▍` 与 `LIVE` 色读的是 `SessionRow.foreground`
（[`src/render/tui.rs:8811`](../../src/render/tui.rs)），而那份 listing 是**启动时**扫出来、
**只推一次**的（[`src/cli.rs:503`](../../src/cli.rs)），传进去的前台是启动那一场的 id。切换
时更新的是渲染侧的 `TuiState::front`（`SwitchSession` 到达时），画列表时却没读它 —— 于是
标记、颜色、以及「前台所在检出组钉顶」的排序全部停在启动那一场。同一份状态里还自带矛盾：
`sessions_click` 判断「点的是不是当前前台」用的正是 `self.front`（对的），只有画的时候用了
过期快照。

**修法**：在 `SwitchSession` 里**就地**把行快照的前台改到新前台，`session_line` 一字不动
（它继续只认 `row.foreground`，单一真相）：

```rust
for row in &mut self.sessions.shown {
    row.foreground = row.id == self.front;
}
```

否决的两条路：**重推 listing**（要碰 store，票 03 实测全量解析 155–172ms、只读前缀 3.7–4.3ms
—— 为一个标记付这个价不划算）；**在 `session_line` 里认 `self.front`**（`row.foreground`
一旦是启动那一份，`||` 兜底会让旧前台**永远也带着** `▍`，而「切没切过」没有状态位可判）。

**真机复验**（pty + SGR 鼠标点击，三场会话）：启动 `▍` 在第 11 行 → 点第二行后第 12 行 →
点第三行后第 13 行；页顶读数 1 场 → 2 场 → 3 场（装载照旧）。

## 欠账

- **「前台所在的检出组钉在最顶」仍滞后**：排序出自 listing 的 `sort_rows`，而这一轮只改了
  行快照的前台归属，没重排。跨检出切换时会出现「`▍` 在中间某行、它所在的组不在顶上」。
  要修就得在渲染侧重排（`shown` 是渲染侧持有的，`group_key` 也在行里），或让主循环在切换后
  重推 listing。
- 主列的**身份行**（`heng：会话 <id> · 模型 …`）是启动回执、不落事件流，切过去不会重现 ——
  同工作区的两场切换后，屏幕上只剩标记与状态行可分辨。要不要给主列一个常驻的会话身份段，
  归票 09 定。
