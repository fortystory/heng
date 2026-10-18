# width module 的 seam 收齐

Type: task
Status: ready-for-agent
Part of: ../map.md
Blocked by: —

## 背景

[report-2026-10-18.md](../report-2026-10-18.md) **Worth exploring**。`src/render/width.rs` 已经收住量宽、按列切、保住样式三件（`truncate_columns` / `ellipsize_line` / `slice_columns`），但截断在 6 处仍按 `chars().take` 拼：

- `src/render/transcript.rs:663`
- `src/render/wording.rs:222`、`:963`
- `src/render/changes.rs:589`
- `src/render/files.rs:118`
- `src/render/tui.rs:15059`（10-18 新发现）

`markdown::wrap_code` 按列自折一份。`panel.rs` / `todo.rs` 已走 `width::truncate_columns`，seam 是「半截收」。[ADR 0008](../../docs/adr/0008-markdown-parsing-by-pulldown-cmark.md) 说「换行归我们」，这一截没收完。

## 想要的形态

把 6 处 `chars().take` 一律改成 `width::truncate_columns`；带样式的改 `width::ellipsize_line`。`width` 加一个 `truncate_chars`（真的按字符截的少数几个出口）显式区分「按列」和「按字」两把尺子。

## 验收

1. 全文 grep `chars().take` 在 `src/render/` 下零命中（除 `width` module 自己）。
2. `width::truncate_columns` 的命中数从当前 ~3 涨到 ≥ 9。
3. 中文界面上同一段宽字符文本的截断结果与未截断结果列数一致。

## 关联

- 来源：[report-2026-10-18.md §width module 的 seam 收齐](../report-2026-10-18.md)。
- HTML：[report-2026-10-18.html 第 3 张卡片](../report-2026-10-18.html)。
- ADR：[ADR 0008](../../docs/adr/0008-markdown-parsing-by-pulldown-cmark.md)（同向，没走完）。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「截断收回 `width` module」分支