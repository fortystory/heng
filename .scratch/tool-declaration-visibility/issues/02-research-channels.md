# 三条既有通道各能承载什么、要付什么代价

Type: research
Status: resolved
Part of: ../map.md
Blocked by: —

## 问题

`system` / `messages` 各有一条通道，`tools` 一条都没有。要开通道就得知道**衡今天有哪些通道
能用**，以及每条**能承载什么、要付什么代价**。要查清八件事：

1. **通道一 · 进流的上下文注入**：`ContextSource` 有哪些变体、今天哪些还在产出、投影怎么对待
   它们、注入在模型上下文里的位置（钉住的前缀 vs 会话中途）。
2. **通道二 · 不进流的渲染通道**：`RenderEvent::Identity` 的完整生命周期 —— 从发射到落到转录上
   那一行、到点开详情；它在哪些渲染器里被忽略；它能承载什么形状。
3. **通道三 · 详情覆盖层的面**：`FaceId` 与分派表的形状、面挂在哪一块详情上、能不能承载
   会话级的东西。
4. **每条通道的代价**：进不进模型上下文、落不落盘、重放时看到的是哪张表、承载什么形状、
   新增几件东西。
5. **先例**：`mcp-support` 票 19 那条通吃所有注入来源的 `DetailKind::Context`；
   `trace-ledger` 票 24 给工具补的那一面。
6. **种子列的三个方向各自落在哪条通道上**。

**这是只读调研，不改任何代码。** 结论一律给 `文件:行号`。

## 产物

[`../research/02-channels.md`](../research/02-channels.md)。写完在票底给 `## 作答`。

## 作答

衡今天有**三条**能承载工具声明的通道，粒度与代价完全不同：

- **通道一 · 进流的上下文注入**（`ContextSource` + `ContextInjected`，`src/events.rs:225-268`、
  `:360-363`）：**10 个变体**，今天产出 9 个（`PlanMode` 只为读老流留着）。**会话级**，
  会进模型上下文、落 `log.jsonl`、可重放 —— 而重放看到的是**当时**那张表。
  投影不区分来源（`src/provider/projection.rs:92-117`，只有 `Persona` 一条例外）。
- **通道二 · 不进流的渲染通道**（`RenderEvent::Identity`，`src/render/mod.rs:138-141`）：
  **会话级**，不进模型上下文、不落盘，**按当前代码拼**（重放看到的是**今天**那张表）。
  它**借用注入那条记录的形状**（`src/render/transcript.rs:294-301` → 转录上一行 `▸`、点得开）。
  **关键：「不进流」是那条通道的性质，不是 `ContextSource::Identity` 这个变体的性质。**
  承载能力是**一段文本**；会话开跑时摆**一次**，在横幅之前（`src/cli/interactive.rs:409`）；
  headless 明确忽略它（`src/render/headless.rs:53`）。
- **通道三 · 详情覆盖层的面**（`FaceId::Schema`，`src/render/tui.rs:14500-14502`、
  `DETAIL_FACES` `:14540-14571`）：**per-call**，承载**结构化**数据，**只有 TUI 有**。
  面挂在一块详情上 —— 「一次看全表」在它里面**没有落点**。

**种子的三个方向分别落在通道三、通道二、通道一（不动）上**；第三个方向其实是「承认今天只有
通道三那一块」。

**通道一不是免费的那条**：它让模型**再读一遍**它已经有的东西（provider 请求里已经有 `tools`
数组，`src/provider/openai.rs:441-455`），并撑大缓存前缀（工具数组本身就是前缀的一部分，
`src/tools/registry.rs:88-90`）。除非明确决定「这张表要进模型上下文」（那与本次缺口的方向
相反 —— 缺口是**人**看不见），否则它应当在决策票里**被明确排除**，而不是留给实现期临场决定。

**数据管道只有一处**：`src/cli/probe.rs:325` 的 `tool_schemas()`。通道二与通道三都要先动它，
通道一不用。**两个 `SessionFacts` 同名**（`src/render/tui.rs:372` 带 `tool_schemas`，
`src/session/facts.rs:18` 不带）—— 「改 `SessionFacts` 的形状」要写死是哪一种。

产物：[`../research/02-channels.md`](../research/02-channels.md)。
