# 这次改动的连带面与纪律：账的形状、桩、文档措辞、词表与 ADR

Type: grilling
Status: resolved
Part of: ../map.md
Blocked by: 04, 05

## 问题

前两票拍定了**形态**与**内容**。本票拍**这次改动落在哪些件上**、**哪些纪律要立**。
它是折 spec 之前的最后一张：spec 要照着它写，所以它必须把「要动哪些件」与
「不许动哪些件」都写死。

### 1. 那本账改成什么形状

`src/cli/probe.rs:325` 的 `tool_schemas()` 今天返回
`BTreeMap<String, serde_json::Value>`，值是 `parameters`。要装下 `description`，形状有几种可能：

- **整份 `ToolSpec`**（`src/provider/mod.rs:96-100` 的三件套）—— 抄本变成
  `BTreeMap<String, ToolSpec>`。好处：provider 那边收到什么就是什么，不二次加工。
  代价：`ToolSpec` 是不是该进渲染层的那一份依赖？（它是 `provider` 模块的公开类型。）
- **三元组** `(name, description, parameters)` —— 渲染层自己定一个形状。
- **两张表** —— 一张存 `description`，一张存 `parameters`；还是一张存参数、一张存别的？
- **一条记录一条**（`Vec` 而不是 `BTreeMap`）—— 但表的顺序是稳定的（`src/tools/registry.rs:88-90`），
  所以「按名字查」这件事今天靠 `BTreeMap` 保着。

**另外**：`tool_schemas()` 这个函数名还叫得对吗？它今天返回的**不是** schema，
是**参数 schema**。

**还有**：两个 `SessionFacts` 同名（`src/render/tui.rs:372` 带 `tool_schemas`，
`src/session/facts.rs:18` 不带）。本票要写死「改哪一个」。

### 2. 连带面（每一处都要点名，不许「顺带改」）

- **读者**：`src/render/tui.rs:8926`（宽度重放后重排面）与 `:15711`（打开详情时造面）。
- **写入点**：`src/cli/interactive.rs:230`、`:258`；`src/cli/discuss.rs:187`、`:218`；
  `src/cli/loading.rs:139`。
- **测试桩**：`tests/render_layout.rs:31`、`tests/history_replay.rs:29`、
  `src/render/tui.rs:18856`（单元测试里的 `schemas()` 桩），以及六处 `tool_schemas: Default::default()`
  （`tests/common/mod.rs:32`、`tests/render_changes.rs:37`、`tests/session_switch.rs:27`、
  `tests/render_tui.rs:51`、`tests/ask_user_question_tui.rs:40`、`tests/render_answer.rs:29`）。
- **文档**：`docs/render.md:544-551`（Schema 面那一节 —— 今天那句「**声明**（发给 provider 的
  那份 JSON Schema）」只对了三分之一）；如果形态变了，「检视器的面」那张分派表
  （`docs/render.md:534-540`）与「系统提示词」那一节（`:493-501`）也可能要跟着动。
- **源头的注释**：`src/render/tui.rs:407-408`（字段自己的注释把 `parameters` 说成「声明」）
  与 `src/cli/probe.rs:319-324`（函数的文档注释）。**这两处是措辞失准的源头**，
  不改它们，下一个人还会接着说「声明」。

### 3. 词表与 ADR

- **`CONTEXT.md` 立不立新词条？** 候选：**工具表**（今天在 `CONTEXT.md` 里只以「元工具」与
  「动态工具」两条的 `_Avoid_` 出现过，没有自己的条目）、**工具声明**（provider 收到的那个
  三件套）。按仓库的纪律，**新概念才立条目** —— 若这次只是把已存在的东西显示出来，
  可能一条都不用立。
- **要不要一条 ADR？** ADR 只记「当初为什么」（见 `CONTEXT.md` 的「spec」词条）。
  候选的决定：「工具声明只对人可见，不进模型上下文」——这**是一条**值得钉住的边界
  （它挡住了「顺手把它注入进去」这个诱惑），而且 [ADR 0005](../../docs/adr/0005-model-visible-text-in-chinese.md)
  那一类也正是模型可见文本的纪律。本票要拍：**写还是不写**，以及**写哪一句**。

### 4. 明确不做的

- 不改 `src/` 里除了上面点名的那些之外的件。
- 不动事件 schema（除非形态选的是通道一 —— 那要另议）。
- 不做「点开工具表然后复制出去」这类新手势。
- 不给 headless / plain 补新的出口（除非形态选的是通道一 —— plain 会跟着那条记录走）。

## 怎么走

**HITL** 票。开工前调用两次 Skill 工具，分别指定 `grilling` 与 `domain-modeling`。
**不许 agent 自问自答。**

## 产物

票底 `## 作答`：第 1 节的形状与函数名、第 2 节的连带面清单（逐项点名）、第 3 节的词表与
ADR 裁决、第 4 节的「明确不做」定稿。**这一票的答案要自足** ——
`/to-spec` 会在别的会话里读它，看不到本图与 charting 对话。

## 评论

- 2026-10-21 charting：本票**不产代码改动**，它只定「要动哪些件、动成什么样」。
  实现切片是 `/to-spec` 之后由 `/to-tickets` 拆出来的 `Type: implement` 票 ——
  那些**不属于这张图**。

## 作答

2026-10-21 维护者直接拍定（与 [04](04-grilling-visible-shape.md) 同一轮）。

**账的形状**：**渲染层自己的一个小类型**，不去依赖 provider 模块的公开类型：

- 键是工具名；值是 `{ description: String, parameters: serde_json::Value }` —— **三件套里只缺
  名字**（名字已经是键），而 `description` 正是今天丢掉的那一样。
- **为什么不用整份 `ToolSpec`**：那是 provider 模块的公开类型，渲染层依赖它就把两层绑在一起；
  渲染层要的是「用法说明 + 参数 schema」两样，用自己的小类型更窄。
- **为什么不用两张表**：`description` 与 `parameters` 永远一起被读（那张面两样都画），拆成两张
  只是多一次查找。
- **顺序**：仍用 `BTreeMap`（按名字查），但**表本身的顺序**由 `Registry::specs()` 给（它稳定，
  因为工具数组是缓存前缀的一部分）—— 会话级那一档摊开时**按那张顺序**，不按名字排序。

**改名**（今天的措辞失准源头）：`tool_schemas()` → **`tool_declarations()`**，字段
`SessionFacts::tool_schemas` → **`tool_declarations`** —— 它抄的**不是 schema**，是声明。
两处注释（`src/render/tui.rs:407-408` 与 `src/cli/probe.rs:319-324`）一并改准。

**连带面**（一处管道，不许「顺带改」）：

- **写入点**：`src/cli/interactive.rs:230`、`:258`；`src/cli/discuss.rs:187`、`:218`；
  `src/cli/loading.rs:139`。
- **读者**：`src/render/tui.rs:8926`（宽度重放后重排面）、`:15711`（打开详情时造面）。
- **测试桩**：单元测试里的 `schemas()` 桩（`src/render/tui.rs:18856`）与六处
  `tool_schemas: Default::default()`（`tests/common/mod.rs:32`、`tests/render_changes.rs:37`、
  `tests/session_switch.rs:27`、`tests/render_tui.rs:51`、`tests/ask_user_question_tui.rs:40`、
  `tests/render_answer.rs:29`）—— **形状跟着改，不许**为了让旧桩继续编译而把新字段做成可选。
- **那个面的名字**：它画的不再只是 schema（还要画用法说明）—— 面与标签改成**「声明」**
  （`FaceId::Schema` → `FaceId::Declaration`，标签的措辞在 `src/render/wording.rs`）。
- **文档**：`docs/render.md` 的三处（检视器的面那张分派表、系统提示词那一节、那个面自己那一节）
  —— 并进[票 10](10-docs-glossary-and-walkthrough.md)。

**词表**：**立一条** ——「**工具声明**」（provider 收到的那三件套）与它所在的「**工具表**」
（这个会话的那一张）。这次改动让它们成了界面上一等的东西（两张形态、两处读法），而今天它们在
`CONTEXT.md` 里只以「元工具」「动态工具」两条的 `_Avoid_` 出现过。词条照仓库的格式写
（`CONTEXT.md`），**不**把实现细节写进去。

**ADR**：**不立**。三条判据不齐 —— 改变主意的成本不高（加一个界面随时能加）、不令人费解
（模型一直看得见完整声明，这本来就是事实）、也不是一次真正的取舍（通道一被排除的理由是
**成本**而不是两难）。理由写进词条与逐面文档就够了。
