# 08: 工具调用的详情里能读到用法说明（per-call · 通道三）

Type: implement
Status: done
Blocked by: 07, 04, 05
Part of: ../map.md

> 规格：[`.scratch/tool-declaration-visibility/map.md`](../map.md) 的**目的地**；形态由
> [04](04-grilling-visible-shape.md) 拍，内容（哪张表、哪个时刻的那张）由
> [05](05-grilling-which-table.md) 拍。
> **本票存在的前提**：04 拍的是「补 `description` 到 Schema 面」这一档（或它与 09 的组合）。
> 若 04 排除了这一档，本票随之关闭 —— **不实现**。

## 目标

- 工具调用详情覆盖层的 **`Schema` 面**先画**用法说明**、再画参数 schema —— 于是
  「**这次调用的那个工具声明是什么**」在界面上读得到（今天那个面只有参数 schema，`description`
  一个字都读不到）。
- 不新增入口、不新增事件：那个面已经在那里，只是少画了一段。

## 现状（开工前先复核）

- 那个面是 `FaceId::Schema`（[`src/render/tui.rs:14500-14502`](../../../src/render/tui.rs)）——
  工具那一行的**第三面**（六面：参数 / 输出 / Schema / 计时 / 来源 / 概述）。
- 它画的是 `parameters` 那一份，`pretty` 打印后按正文宽折行，与参数面同一套排法
  （[`src/render/tui.rs:14996-15009`](../../../src/render/tui.rs)）。
- 查不到时写一句「工具表里没有「<名字>」，这次调用的声明查不到」
  （[`src/render/wording.rs:549`](../../../src/render/wording.rs)），**不画空面、不编一份 schema
  出来** —— 这一条照旧。
- 用法说明最长的一条 896 字符（`ask_user_question`），最短的 63 字符（`write_file`）
  （研究 01 第 2 节）—— **两种长度都要读得下去**。
- 数据来自[票 07](07-account-carries-the-whole-declaration.md)（账带上整份声明）。

## 具体行为

1. 那个面的顺序：**用法说明在前，参数 schema 在后**；用法说明按正文宽折行（与参数面同一套排法）。
2. 长声明（896 字符）与短声明（63 字符）两种长度下都读得下去 —— 具体形状照
   [03 的帧](../prototype/03-shapes.txt)（若 03 已过帧）。
3. 「今天这张表」这句话按 05 的结论说（在哪儿说由 05 定）；查不到时**仍是那一句**，不新造措辞。
4. 不改那个面的**名字**与位置（`Schema` 面已经在六面分派表里）。

## 验收

- [ ] 点开一次工具调用，切到 `Schema` 面：**先**读到用法说明，**后**才是参数 schema。
- [ ] 最长那条（`ask_user_question`）与最短那条都读得下去。
- [ ] 查不到的工具仍是那一句「工具表里没有「<名字>」…」，**不画空面**。
- [ ] 模型可见文本零变化；事件流零变化。
- [ ] `cargo test` 全绿；`cargo clippy --all-targets` 无警告；`cargo fmt --check` 干净。
- [ ] 逐面文档与真机走查条目跟上（并进[票 10](10-docs-glossary-and-walkthrough.md)）。

## 评论

- 2026-10-21 落地：**那个面先画用法说明、再画参数 schema** —— 两节，各带一个小标题
  （`── 用法说明 ──` / `── 参数 schema ──`），用法说明按正文宽折行（与参数面同一套
  [`folded_text`] 排法），参数 schema 仍**原样** `pretty` 打印。

  **面与标签改名**（06 的连带面清单点名的那一处）：`FaceId::Schema` → `FaceId::Declaration`，
  标签 `wording::detail_schema_face()`（显示为「Schema」）→ `wording::detail_declaration_face()`
  （显示为「**声明**」）。理由写进注释：它画的是**整份声明**（用法说明 + 参数 schema），而
  「Schema」只对了其中一半。分派表那一行（工具那六面：参数 · 输出 · **声明** · 计时 · 来源 ·
  概述）与面名一起改，位置不动 —— 界面不新增入口。

  **查不到时照旧**：工具名不在表里时仍是那一句「工具表里没有「<名字>」，这次调用的声明查不到」
  （`wording::detail_schema_missing`，**措辞一个字没动**）—— 不新造措辞、不画空面、不编一份
  schema 出来。查得到时那一面是**两节**，于是两节的小标题都在（面里的 section 小标题只在
  ≥ 2 节时画）；查不到时只有一节，于是那一面**一个字的小标题都不画**。

  **模型可见文本零变化、事件流零变化**：新增的三处措辞只在 TUI 详情覆盖层里用，`messages`
  与 `log.jsonl` 一个字节没动。

  **动了哪些件**：`src/render/tui.rs`（面 id、标签、分派表那一行、画法与它的单元测试）、
  `src/render/wording.rs`（**只**改这个面相关的措辞：标签与两段小标题；别的措辞一个字不动）、
  `tests/render_layout.rs`（那一节的用例）。**另有一处范围外的必改**：`tests/history_replay.rs`
  里那条标签条常量 `TOOL_FACES`（`"参数┆输出┆Schema┆…"` → `"参数┆输出┆声明┆…"`）——
  面名改了它就过时，不改则那条测试必红，所以一并改了（只改那一个字符串）。

  **测试**（先红后绿，一个 slice 一条）：
  - 单元测试（`src/render/tui.rs` 的 `#[cfg(test)]`）**新增 2 条**：
    `the_declaration_face_reads_the_usage_description_before_the_parameters`（两节都在位，且用法
    说明那一节的正文排在参数 schema 那一节之前）、`the_declaration_face_wraps_the_usage_description_to_the_body_width`
    （320 字符的说明在正文宽 40 上折成多行，且每一行都不宽于正文宽）。
    **改写 2 条**：`the_schema_face_prints_the_declaration_verbatim` →
    `the_declaration_face_prints_the_parameter_schema_verbatim`（参数 schema 那段仍原样 `pretty`
    打印）、`a_call_with_no_started_event_reports_its_schema_as_missing`（查不到时仍是那一句）。
    另加一条测试帮手 `face_body`（把面**连小节标题**一起摊成文本 —— `face_text` 不带标题）。
  - 集成测试（`tests/render_layout.rs`）**新增 2 条**：
    `the_declaration_face_reads_the_usage_description_before_the_parameters`（在画出来的那一帧上，
    两节标题的先后与正文的先后都对）、`the_declaration_face_reads_a_long_usage_description_and_a_short_one`
    （896 字符与 63 字符两种长度：开头可见、折成多行、滚到底读得到末尾）。
    **改名 2 条**：`the_schema_face_shows_the_declaration_of_the_tool_this_call_used` →
    `the_declaration_face_prints_the_parameter_schema_verbatim`（这一面仍不是参数面）、
    `the_schema_face_says_so_when_the_tool_is_not_in_the_table` →
    `the_declaration_face_says_so_when_the_tool_is_not_in_the_table`（**断言一个字没改**）。

  **实测**（本机）：

  | 命令 | 结果 |
  | --- | --- |
  | `cargo test --lib render::` | ok. **211** passed; 0 failed（改动前 209，新增 2） |
  | `cargo test --test render_layout` | ok. **310** passed; 0 failed（改动前 308，新增 2） |
  | `cargo test --test history_replay` | ok. **38** passed; 0 failed（只改了那一个常量字符串） |
  | `cargo test --no-fail-fast` | ok. **1939** passed; **0** failed（89 个测试二进制） |
  | `cargo clippy --all-targets` | Finished，无警告 |
  | `cargo fmt --check` | 干净 |
  | `python3 scripts/check-language.py` | OK |

  **逐面文档要跟着改的**（并进[票 10](10-docs-glossary-and-walkthrough.md)）：
  `docs/render.md` 的「检视器的面」那张分派表里「工具」那一行（`Schema` → **声明**）、
  那一节里「**Schema**：……」那条 bullet（要写清它**先画用法说明、再画参数 schema**，两节各带
  小标题；用法说明按正文宽折行，参数 schema 原样 `pretty` 打印），以及「系统提示词」那一节
  （09 的先例）。**「声明」与「参数 schema」在文档里要分清**：前者是整份三件套（用法说明 +
  参数 schema），后者只是其中一样。

