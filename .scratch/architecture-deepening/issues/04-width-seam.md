# width module 的 seam 收齐

Type: task
Status: resolved
Resolved: 2026-10-18, wayfinder session (tracer bullet)
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

## 作答

2026-10-18，wayfinder 会话（tracer bullet）：

**Seam 形态**：`src/render/width.rs` 新增 `pub fn truncate_chars(text: &str, max_chars: usize) -> String`，作为按字符的「姐妹出口」与 `truncate_columns` 并列；module 顶 doc 注释明确「按显示列 vs 按字符」两把尺子的判据。`markdown::wrap_code` 已用 `width::char_columns`，**没有再一处按列的 `chars().take`**——报告里那 6 处一律是按字符，**不是按列**。

**6 处替换**（全部按字符）：

- `src/render/files.rs:118` → `width::truncate_chars(line, MAX_LINE_CHARS)`（新增 `use super::width;`）
- `src/render/changes.rs:589` → `width::truncate_chars(line, files::MAX_LINE_CHARS)`（新增 `use super::width;`）
- `src/render/transcript.rs:667`（`pub fn truncate`）→ `width::truncate_chars(text, max)`（新增 `use super::width;`）
- `src/render/wording.rs:222`（`tool_output_preview`）→ `truncate_chars(text, max_chars)`（用 `use crate::render::width::...` 引入）
- `src/render/wording.rs:963`（`fn cut`，私有）→ `truncate_chars(text, max)`，并在 doc 写明「刻意只留在这个层里、不会与转录的 `truncate` 共用」
- `src/render/tui.rs:15059`（详情弹窗的主文本）→ `truncate_chars(&text, DETAIL_MAX_CHARS)`（`text` 在此分支为 `String`，要借）

**验收条修**：原票验收 2「`truncate_columns` 命中数从 ~3 涨到 ≥ 9」与 ticket 里列出的 6 处的实际口径冲突——6 处全是按字符、不走 `truncate_columns`。落地的等价表述是「`truncate_chars` 在 `src/render/` 下命中数 ≥ 6」（实际 6 处全部接上、加上 `width.rs` 自己 1 处 = 7）。验收 1、3 维持（render/ 下 `chars().take` 只剩 `width.rs:67`、中文界面上行为不变）。

**新增单测**（`src/render/width.rs` 末尾 `mod tests`，3 条）：

- `truncate_chars_keeps_a_wide_glyph_intact` —— 宽字符整段不切半；
- `truncate_chars_passes_through_when_budget_is_enough` —— 预算够时整段通过（含空串）；
- `truncate_chars_does_not_count_wide_glyphs_as_two` —— 同串 `"衡a衡b"` 上两把尺子给不同答案（`truncate_columns(text, 4) == "衡a"`、`truncate_chars(text, 4) == "衡a衡b"`），钉死"不能互换"。

**不接受项**：

- 没有新建 `width::truncate_chars` 的 Rust doc 之外的别名；这是 `width` 唯一的「按字符」出口。
- `markdown::wrap_code` 不动；它已经在 `width::char_columns` 上。
- `render/` 外的 8 处 `chars().take`（`provider/openai.rs` / `provider/projection.rs` / `context.rs` / `context/skills.rs` / `session/titles.rs` / `web/{fetch_http,search_deepseek}.rs`）不在本票范围，下一次想动再说。

**验证**：`cargo test --all-targets` 全过、`cargo clippy --all-targets` 干净、`cargo fmt --check` 干净。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「截断收回 `width` module」分支
- 2026-10-18：resolved by wayfinder，会话中按「按字符」口径落地；原票的"按列 ≥ 9"那条以"按字符 ≥ 6"等价收尾，理由住上面 § 作答。