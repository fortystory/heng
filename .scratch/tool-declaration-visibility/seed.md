# 种子材料：工具声明在界面上看不见

> **这不是 spec，也不是票。** 2026-10-20 维护者在 TUI 里看「请求组成」，注意到注入清单只有
> `AGENTS.md` / 技能清单 / MCP 加载 / 系统提示词 四行，**没有工具声明**。排查结论：不是渲染层
> 漏了一条记录，而是从来没有为工具表开过会话级通道。未访谈、未拆票。

## 现象

模型每一次请求由三样东西组成：`system`（拼好的身份）、`messages`（投影）、`tools`（工具表）。
前两样在 UI 上都有入口 —— 流里的注入各有 `ContextSource` 变体，身份则由渲染层单独摆一条
`RenderEvent::Identity`（不进事件流）。第三样没有。

具体缺口在 `description`：工具表里每个工具的**用法说明**住在那段文字里（`ask_user_question` 的
「何时问、答案 JSON 怎么读、题面宜短、(推荐) 放第一个」全在 `src/tools/ask_user.rs` 的
`spec().description`），而 UI 里**没有任何地方**能看到它。

## 现在的落点：per-call 的 Schema 面，且少了 description

工具声明唯一的可见处是一次工具调用的详情覆盖层的 `Schema` 面（`src/render/tui.rs` 的
`FaceId::Schema`，`trace-ledger` 票 24）。它读 `SessionFacts::tool_schemas`，而那本账只抄了
`parameters`：

```rust
.map(|spec| (spec.name, spec.parameters))   // src/cli/probe.rs 的 tool_schemas()
```

于是有两处与事实不完全一致，都是这个缺口的下游：

- 那个面画的是**参数 schema**，不是完整声明（provider 收到的是 `name` + `description` +
  `parameters` 三件套）。
- [`docs/render.md`](../../docs/render.md) 把它写成「声明（发给 provider 的那份 JSON Schema）」，
  严格说只对应三分之一。

## 为什么算缺口，而不是取舍

- `ContextSource` 每个变体都写了存在理由（`src/events.rs`），**没有**「工具表不进来」那条理由；
  `SessionFacts::tool_schemas` 的注释也只解释「为什么组装期抄一次」，没说「为什么只抄参数」。
- 系统提示词那一侧有明文理由（「不是事件、也永远不进事件流」），工具表这一侧没有对应的一句。
- 这条通道有过先例：`mcp-support` 票 19 把「注入的上下文可查看详情」做成 `DetailKind::Context`
  通吃所有来源，而工具声明在这个模型之外。

## 三个方向（都不便宜，选哪个要先想清楚）

1. **补 description 到 Schema 面**：`tool_schemas` 改存整份 `ToolSpec`，那个面先画用法描述再画
   参数。改动最小、仍在 per-call；代价是 `SessionFacts` 的数据形状变一次，`tests/tui.rs` 里的
   `schemas()` 桩与 `docs/render.md` 那一节都要跟着改。
2. **给轨迹加一条会话级记录**：仿 `RenderEvent::Identity`，组装期在 `Harness::identity()` 旁边摆
   一条「工具声明（按当前代码拼，不进事件流）」，点开看全表。好处是与系统提示词对称、一次看全；
   代价是多一条不进流的记录类型，还要定「画一行还是画 N 行」以及重放老会话时的含义（今天这张表，
   不是当时那张）。
3. **只记文档**：把 `docs/render.md` 那句「声明」改成「参数 schema」，承认今天看得到的就是参数。
   不新增任何界面。

## 没做的

- 没有动代码，也没有改 `docs/render.md` 那句措辞 —— 那两件事属于上面三个方向里的选择。
- 没有判断「模型看不看得到」这一侧的后果：模型**一直**看得到完整声明（它随 `tools` 发出），
  看不见的只有人。这一点决定了这不是功能缺陷，而是可观测性缺口。
