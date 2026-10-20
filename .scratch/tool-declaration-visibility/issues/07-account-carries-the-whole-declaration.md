# 07: 工具声明的账带上整份声明（prefactor）

Type: implement
Status: ready-for-agent
Blocked by: 04, 06
Part of: ../map.md

> 规格：[`.scratch/tool-declaration-visibility/map.md`](../map.md) 的**目的地**；形态由
> [04](04-grilling-visible-shape.md) 拍，账的形状由 [06](06-grilling-boundaries.md) 拍。
> 本票是后面两张形态票（[08](08-per-call-usage-description.md) 与
> [09](09-session-level-declaration-record.md)）的**共同前置** —— 它**不新增任何界面**。
>
> **形状不是本票拍的**：本票给一个**可换的最小形状**（用法说明 + 参数 schema 两样，名字已经在
> 键上），若 06 拍的是别的形状（整份 `ToolSpec` / 两张表），改这一处即可。

## 目标

- 渲染层那本账从「**只抄参数 schema**」变成「**抄整份声明**」：名字（键）、**用法说明**、参数
  schema 三件套都在账里 —— 于是界面第一次**拿得到**那段用法说明（今天它在抄取那一步就丢了）。
- 抄取点仍是一处；模型可见文本**零变化**；不新增任何界面、不新增任何事件。

## 现状（开工前先复核）

- 抄取点只有一处：[`src/cli/probe.rs:325`](../../../src/cli/probe.rs) 的 `tool_schemas()` ——
  `registry.specs().into_iter().map(|spec| (spec.name, spec.parameters)).collect()`，
  **用法说明（`description`）在这一步就丢了**（研究 01 第 3 节）。
- 抄本形状：`BTreeMap<String, serde_json::Value>`，字段名 `tool_schemas`
  （[`src/render/tui.rs:416`](../../../src/render/tui.rs)）。
- **字段的注释已经把 `parameters` 说成「声明」**
  （[`src/render/tui.rs:407-408`](../../../src/render/tui.rs)），函数的文档注释也这么写
  （[`src/cli/probe.rs:319-324`](../../../src/cli/probe.rs)）—— **这两处是措辞失准的源头**，
  本票一并改准（研究 01 第 3 节、研究 02 第 2 节）。
- 读者只有两处，都在渲染层内部：[`src/render/tui.rs:8926`](../../../src/render/tui.rs)（宽度重放后
  重排面）与 [`:15711`](../../../src/render/tui.rs)（打开详情时造面）。
- 写入点三处，装配链逐字同形：[`src/cli/interactive.rs:230`](../../../src/cli/interactive.rs)、
  [`src/cli/discuss.rs:187`](../../../src/cli/discuss.rs)、
  [`src/cli/loading.rs:139`](../../../src/cli/loading.rs)。
- 测试桩要跟着改：单元测试里的 `schemas()` 桩（[`src/render/tui.rs:18856`](../../../src/render/tui.rs)）
  与六处 `tool_schemas: Default::default()`（`tests/common/mod.rs`、`tests/render_changes.rs`、
  `tests/session_switch.rs`、`tests/render_tui.rs`、`tests/ask_user_question_tui.rs`、
  `tests/render_answer.rs`）—— 研究 02 第 2 节点名了这一串。
- **两个 `SessionFacts` 同名**：[`src/render/tui.rs:372`](../../../src/render/tui.rs) 那个带
  `tool_schemas`，[`src/session/facts.rs:18`](../../../src/session/facts.rs) 那个不带 ——
  改的是**前者**（研究 01 第 6 节）。

## 具体行为

1. 账的值能装下三件套：键是工具名，值里除参数 schema 外还**带着用法说明**。形状按 06 的结论；
   本票给最小可换的那一种。
2. 抄取点、写入点、读者三处一起改（**一处管道**，不许在别处另抄一份）。
3. 字段与函数的**注释改准**：它们今天把参数 schema 说成「声明」，而 provider 收到的是
   `name` + `description` + `parameters` 三件套（研究 01 第 1 节）。
4. 桩与夹具跟着改形状（上面点名的那一串），**不许**为了让旧桩继续编译而把新字段做成可选。

## 验收

- [ ] 那本账里能读到某个工具的**用法说明**（不只是参数 schema）。
- [ ] 模型可见文本**零变化**（`tools` 数组发给 provider 的内容一个字节没动）。
- [ ] 界面**一个字没多**（这一票只是让后面两票拿得到东西）。
- [ ] 字段与函数的注释不再把参数 schema 说成「声明」。
- [ ] `cargo test` 全绿；`cargo clippy --all-targets` 无警告；`cargo fmt --check` 干净。

## 评论

（空 —— 开工时补：落地与实测）
