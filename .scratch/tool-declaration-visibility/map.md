# 工具声明在界面上看得见（wayfinder 决策图）

Label: `wayfinder:map`
Status: 6 resolved + 2 done + 2 ready-for-agent
Tracker: local markdown —— 见 [`docs/agents/issue-tracker.md`](../../docs/agents/issue-tracker.md)
Charting: **已完成**（2026-10-21）。两张事实票当场收回；四张决策票同日在维护者面前收口
（原型帧**没有出** —— 形态被直接拍定）。
本图只做**规划**，不产代码改动。

## 目的地

一份**可执行的 spec**（交给 `/to-spec` 折成构建计划），像
[`trace-ledger`](../trace-ledger/map.md) 那样：让**工具声明**（provider 收到的那份 `tools` 表，
特别是每个工具的 `description` 用法说明）在**界面上看得见**。

一次请求由三样组成：`system`（拼好的身份）/ `messages`（投影）/ `tools`（工具表）。
前两样各有通道 —— 注入各有 `ContextSource` 变体，身份则由渲染层单独摆一条不进事件流的
`RenderEvent::Identity`。**第三样一条都没有**：工具声明唯一露头的地方是一次工具调用详情覆盖层的
`Schema` 面（`src/render/tui.rs` 的 `FaceId::Schema`），而它**只画 `parameters`**，没画
`description`。于是**模型一直看得见完整声明**（它随 `tools` 发出），**看不见的只有人** ——
这是可观测性缺口，不是功能缺陷。

**范围**：那本账的抄取点（`src/cli/probe.rs`）、渲染层（`src/render/tui.rs`、
`src/render/wording.rs`）、可能的新通道（`src/events.rs` 的来源 / `src/render/mod.rs` 的
`RenderEvent`）、`docs/render.md` 的「检视器的面」与「系统提示词」两节、`CONTEXT.md`。
**模型可见文本零变化**；事件 schema 动不动、界面上是哪种形态，由
[工具声明在界面上以什么形态可见](issues/04-grilling-visible-shape.md) 拍。

## 笔记

- **领域**：衡（heng）的 TUI 外壳与渲染层。术语以 [`CONTEXT.md`](../../CONTEXT.md) 为准
  （**工具表** / **元工具** / **动态工具** / **详情覆盖层** / **面** / **会话事实**）。
- **起点**：[`seed.md`](seed.md) —— 2026-10-20 维护者在 TUI 里看「请求组成」，注意到注入清单只有
  `AGENTS.md` / 技能清单 / MCP 加载 / 系统提示词四行，**没有工具声明**。排查结论：不是渲染层漏了
  一条记录，而是**从来没有为工具表开过会话级通道**。
- **要咨询的 skills**：`/research`（两张事实票，已收回，产物在 [`research/`](research/)）、
  `/prototype`（一张形态票）、`/grilling` + `/domain-modeling`（三张决策票）。
- **既有决定必须先读**：
  [`trace-ledger/spec.md`](../trace-ledger/spec.md) §6（详情覆盖层的面与那张七行分派表）、
  [`trace-ledger` 票 24](../trace-ledger/issues/24-docs-and-close-out.md)（`Schema` 面是收口时
  补上的第六面，它**只画 `parameters`**）、[`mcp-support` 票 19](../mcp-support/issues/19-mcp-catalog-in-context.md)
  （`DetailKind::Context` 通吃所有注入来源）、[`session-panes` 票 25](../session-panes/issues/25-session-facts-per-session.md)
  （会话事实每会话一份）。
- **文档纪律**：新增或改写的 `docs/**` 要过
  [`scripts/check-doc-size.py`](../../scripts/check-doc-size.py) 的压表达规则。
- **每张票的答案必须自足**：`/to-spec` 会在别处的会话里读它，看不到本图与 charting 对话。

### 冻结项（维护者已定，票里不得重开）

1. **终点是一份可执行的 spec**（交给 `/to-spec` 折成构建计划），像 [`trace-ledger`](../trace-ledger/map.md)
   那样。本图**只产决定**，不产代码改动。
2. **缺口是「人看不见」**：模型一直看得见完整声明（它随 `tools` 发出，`src/provider/openai.rs:441-455`）。
3. **三个方向一个都不许在 charting 里选掉**：补 `description` 到 Schema 面 / 加一条会话级记录
   （仿 `RenderEvent::Identity`）/ 只改文档措辞 —— 选哪个是
   [形态](issues/04-grilling-visible-shape.md) 的活，不是画图时的活。
4. **charting 期间不改任何代码或文档**：`src/`、`docs/`、`CONTEXT.md`、
   `docs/render.md` 的措辞都不许动。

### Tracker 事实与降级（本图适用）

- map = `.scratch/tool-declaration-visibility/map.md`，child = `.scratch/tool-declaration-visibility/issues/NN-*.md`；
  阻塞 = 票面 `Blocked by: NN`；claim = 把 `Status:` 写成 `claimed`；
  resolve = 票底写 `## 作答` + `Status: resolved` + 本文 `已定的决定` 追加一行。
- **没有 native sub-issue / 依赖边**，回退到正文约定：本文的 `## 任务清单` 逐条引用子票
  （条目数 == 子票文件数），每张子票顶部写 `Part of: ../map.md`。
- 校验：`python3 scripts/wayfinder-check.py .scratch/tool-declaration-visibility/map.md`。
  宣布图走完之前必须 PASS。
- 每个 session 至多 resolve 一张票（research 票除外）。用户可能并行跑 unblocked 的票。
- **frontier**：扫 `issues/` 找 open、未阻塞、未认领的票，编号最小的当选；`Type: implement` 一律跳过
  —— 那种票不属于本图。

## 已定的决定

<!-- 索引：每条一行，够判断相关性即可；细节住在票里。按名字引用，不写裸编号。 -->

- [工具声明在界面上以什么形态可见：三个方向的取舍](issues/04-grilling-visible-shape.md) ——
  **两张都做**：**per-call**（那个面先画用法说明、再画参数 schema）与**会话级**（转录上一条可点开的
  记录，点开是全表）。两者**各答各的**：per-call 答「这次调用的那个工具声明是什么」，会话级答
  「这个会话有哪些工具、每个工具的用法说明是什么」。**通道一（把整张表当上下文注入推进去）明确
  排除** —— 它让模型再读一遍它已经有的 `tools` 数组并撑大缓存前缀，而这次缺口的方向相反
  （**人**看不见，模型一直看得见）。**方向三（只改文档措辞）不选** —— 它不堵缺口（那部分措辞
  照样要改，并进票 10）。
- [看得见的是哪张表、哪个时刻的那张](issues/05-grilling-which-table.md) ——
  **主会话那张**（讨论 CLI 是**同一张**，不用分别说；执行者那张是**子集**，不另显）。**今天那张**
  （老会话重开时重新建表）—— **写进来源名**，照系统提示词那条：`工具声明（按当前代码拼，不进事件流）`。
  MCP server 自己的工具**永远不进表**，动态工具与内建**一视同仁**。三条前提被写死（表在组装期
  只读、这张表 = 这个会话用的工具、per-call 查到的是今天的表）。
- [这次改动的连带面与纪律：账的形状、桩、文档措辞、词表与 ADR](issues/06-grilling-boundaries.md) ——
  账改成**渲染层自己的小类型** `{ description, parameters }`（键是名字；不去依赖 provider 模块的
  公开类型）。**改名**：`tool_schemas()` → `tool_declarations()`，字段与函数一并改 —— 今天
  「声明」这个说法失准的源头就在那儿。那个面与标签改成**「声明」**。**词表立一条**
  （工具声明 / 工具表）；**不立 ADR**（三条判据不齐）。
- [三个方向各出一帧：工具声明在界面上看得见，长什么样](issues/03-prototype-shapes.md) ——
  **帧没有出**：维护者直接拍定了形态，而本票的目的是提高讨论保真度。它留下的那条体积事实作数
  （一页多 → 会话级那一档是「一行 + 点开一块可滚的详情」）。

<!-- 索引：每条一行，够判断相关性即可；细节住在票里。按名字引用，不写裸编号。 -->

- [工具表这份账：形状、规模与它今天露过面的地方](issues/01-research-tool-table-shape.md) — **表在组装期建好，此后只读**（全仓没有运行时注册点；`docs/lifecycle.md:488` 同一条不变式），所以「一张会话级的表」这件事在衡里成立。**抄给渲染层的账只抄了 `parameters`** —— 抄取点只有一处（`src/cli/probe.rs:325`），读者只有渲染层内部两处；**字段自己的注释已经把 `parameters` 说成「声明」**（`src/render/tui.rs:407-408`），`docs/render.md:544` 那句措辞失准是它的下游。**规模**：19 条内建声明、`description` 合计 5881 字符（最长 `ask_user_question` 896）、参数 schema 在 1.5–2 万字符量级；动态工具条数随配置而变、**不受代码控制**；MCP 侧只有四个元工具，**server 自己的工具永远不进表**。**今天露过面的地方只有一处**（工具详情的 `Schema` 面），所以 `description` 在界面上一个字都读不到。**重放老会话看到的是今天的表**，不是当时那张。产物 [`research/01-tool-table-shape.md`](research/01-tool-table-shape.md)。
- [三条既有通道各能承载什么、要付什么代价](issues/02-research-channels.md) — 衡今天有**三条**能承载工具声明的通道，粒度与代价完全不同：**通道一 · 进流注入**（会话级、会进模型上下文、落盘、重放看到**当时**那张表）；**通道二 · 不进流的渲染通道**（`RenderEvent::Identity`，会话级、不进模型上下文、不落盘、重放看到**今天**那张表；它**借用**注入那条记录的形状，且「不进流」是**那条通道**的性质而不是那个变体的性质）；**通道三 · 详情覆盖层的面**（**per-call**、结构化、**只有 TUI 有**，而面挂在一块详情上 —— 「一次看全表」在它里面没有落点）。**种子的三个方向分别落在通道三、通道二、通道一（不动）上**。**通道一不是免费的那条**：它让模型再读一遍它已经有的东西，并撑大缓存前缀 —— 除非明确决定「这张表要进模型上下文」（与本次缺口方向相反），否则应当在决策票里被**明确排除**。**数据管道只有一处**（`src/cli/probe.rs:325`）。产物 [`research/02-channels.md`](research/02-channels.md)。

## 尚未明确

<!-- 通往目的地、但还不够清晰到能成票的视野。解决一张票会把它前方的一片 fog 升级成新票。 -->

- **工具声明进了转录正文之后，它进不进搜索索引？** 轨迹页有全文索引
  （[`trace-ledger` 票 09](../trace-ledger/issues/09-grilling-search-and-folding.md) 建的，每块一条
  索引，摘要 + 正文全文）。若会话级记录走通道二，那一整张表会**进**索引：一篇 5881 字符的
  声明进索引有什么后果（索引多大、命中是不是噪声、搜索结果里出现一大段声明）？现在说不清 ——
  它取决于形态先定成哪一个。
- ~~**多会话宿主下那条会话级记录挂在哪？**~~ **已收掉**：照**系统提示词**那条的先例 ——
  每个会话在装载时推自己那条（`src/cli/interactive.rs:409` → `src/lib.rs:1187`），于是它**属于
  那个会话**，摆一次就够（表在会话中途不变）。
- **「看得见」的读者是人，还是未来的自动化？** 若只是**人**要读模型手里有什么，那么它只是 TUI 的
  一件事；若将来要「导出一份工具表」（`sessions show` 那一类），那 plain / headless / CLI 导出
  都可能要开口子，而那是另一个范围。这一条影响**范围**，要在形态定了之后回来问。

## 超出范围

<!-- scope 边界，不是路线的一步：封闭的结论，永远不 graduate。 -->

- **让模型看得见工具声明**：模型一直看得见（`tools` 数组，`src/provider/openai.rs:441-455`）。
  反过来的需求（把这张表注入给模型）是另一件事 —— 那正是通道一，而它已被
  [形态](issues/04-grilling-visible-shape.md) 明确处置。
- **改 `description` 的内容**：那张表是**给模型读的用法说明**。改它的措辞会动缓存前缀与
  所有会话的行为，是另一个 effort。
- **改工具表本身**（加一个工具、删一个工具、换一张表）：那是别的 effort。
- **把 MCP server 自己的工具搬进表**：MCP 的规矩是 server 的工具永远不进表
  （`CONTEXT.md` 的「元工具」词条、`src/events.rs:230-233`），这一条**不因这次改动而动**。
- **`/to-spec` 之后的实现切片**：`Type: implement` 的票由 `/to-tickets` 拆、由 `/implement`
  认领 —— **不属于这张图**。设计票全部关掉，图就算走完，哪怕实现票还开着。
- **真机走查与验收**：属于实现票的收尾，不是这张图的票。
- **把工具表导给 CLI / 自动化**：这次的读者是**人**（[04](issues/04-grilling-visible-shape.md)：
  缺口是「人看不见」，模型一直看得见）。若将来要 `sessions show` 那一类导出，plain / headless /
  CLI 都要开口子 —— 那是另一个 effort。

## 任务清单

<!-- 条目数必须等于 issues/ 下的子票数（scripts/wayfinder-check.py 校验）。frontier 的权威查询仍是扫 issues/ 里 open + unblocked + unclaimed 的票。 -->

**事实票**（AFK，charting 当天派出并已收回，产物在 [`research/`](research/)）：

- [x] [工具表这份账：形状、规模与它今天露过面的地方](issues/01-research-tool-table-shape.md)
- [x] [三条既有通道各能承载什么、要付什么代价](issues/02-research-channels.md)

**形态与决策票**（HITL，prototype 先提 fidelity、grilling 收口）：

- [x] [三个方向各出一帧：工具声明在界面上看得见，长什么样](issues/03-prototype-shapes.md)
- [x] [工具声明在界面上以什么形态可见：三个方向的取舍](issues/04-grilling-visible-shape.md)
- [x] [看得见的是哪张表、哪个时刻的那张](issues/05-grilling-which-table.md)
- [x] [这次改动的连带面与纪律：账的形状、桩、文档措辞、词表与 ADR](issues/06-grilling-boundaries.md)

共 **6** 张票（2 research + 1 prototype + 3 grilling），**全是决策票** —— 产出是**决定，不是交付物**。
图走完之后会被折成 spec（**交棒**），而图**留着当决策记录**。

**frontier 现在是谁**：03（编号最小、未阻塞、未认领）。

### 构建切片（2026-10-21 `/to-tickets` 拆出）

**四张实现票全部**被决策票挡着** —— 它们是那张图走完之后要落的形状，形态由 04 拍、内容由 05 拍、
账的形状由 06 拍。决策票一改形状，对应的票就改或关（map 的规矩：被排除的方向**关闭**，不实现）。

- [x] [07: 工具声明的账带上整份声明（prefactor）](issues/07-account-carries-the-whole-declaration.md)
- [x] [08: 工具调用的详情里能读到用法说明（per-call）](issues/08-per-call-usage-description.md)
- [ ] [09: 转录上一条「工具声明」记录，点开是全表（会话级）](issues/09-session-level-declaration-record.md)
- [ ] [10: 文档、词表与真机走查收口](issues/10-docs-glossary-and-walkthrough.md)

共 **10** 张票（6 张决策票 + 4 张构建切片）。**构建切片一条都不许在决策票走完之前动手** ——
`/to-spec` 折出来的 spec 是它们的共同依据；04 若排除了某一档（方向三「只改文档」、或 08 与 09
里的一个），对应的票**关闭**，不实现。
