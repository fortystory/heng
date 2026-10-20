# 种子材料：同目录的会话互相通信（agent teams）

> **这不是 spec，也不是票。** 它是 2026-10-17 在一轮 `/ask-matt` 里记下的意向：
> 同一个目录下活着的几个会话能互相通信，于是几个会话能组成一个团队。
> 还没被访谈、也没有票；想推进时走 `/grill-with-docs` 把它折成 `spec.md`，再 `/to-tickets` 拆票。
> **写就于 2026-10-17**；下面《现状》一节核实于同一天。
>
> **2026-10-20 补了一轮一手调研**（Claude Code / DSH / Codex / 其余各家）：结论与选型在
> [`research/05-comparison-and-recommendation.md`](research/05-comparison-and-recommendation.md)，
> 原始笔记在同目录的 `01`–`04`，大白话图解在
> [`eli5-form-b.html`](eli5-form-b.html)。调研给下面《待谈的分叉》逐条给了初判。
>
> **2026-10-21 决定暂缓**：先把别的功能的优先级排完。**材料原样放着，一行代码没动** ——
> 想推进时从《调研给出的形状》与《待谈的分叉》接着往下走。

## 它要什么

同一目录（同一个**会话桶** / 同一个**检出**）下的几个会话之间有一条通信通道：能互发消息、能看见
彼此的状态、能共用一份待办或任务板 —— 合起来就是一支 agent team。

前端上要能同时看见团队成员。这件事与「多会话平铺」共用一个承载：先有并排的多个会话，才有「团队
在屏幕上」。

## 现状

- **多会话是既有能力**：同一个进程里一个会话 = 一个 `Harness` 加一个拥有它的 tokio task
  （[`src/session/host.rs`](../../src/session/host.rs)）；主循环能做的只有三个动作 —— 跑一次动作、
  取消、关闭；前台指针是全进程一个 `watch<SessionId>`，切换只是改它，`Ctrl-C` 只取消前台那个会话。
- **会话之间没有任何通道**：没有消息投递，也没有跨会话共享的待办；会话之间的「看见」全靠人点左栏
  会话页。
- **「多 agent」今天指的是另外两件事**：
  - **委派链**：讨论者通过 `task` 工具派出的**执行者**
    （[`src/agent/executor.rs`](../../src/agent/executor.rs)，`fs-agent-v1` spec §16）—— 嵌套跑在
    一次 `run_turn` 里、与派发者共用同一份事件流（`ExecutorSpawned` / `ExecutorFinished`），
    **不是**一条独立会话，宿主表里没有它，也不能再派发。
  - **讨论会话**：一个会话里多角色轮流发言，仍是单流，靠 speaker 标名与上色区分
    （[`src/discussion.rs`](../../src/discussion.rs)）。
- **「同目录」有两个现成判据**：会话**桶**（按 cwd 的 slug，
  [`src/session/store.rs`](../../src/session/store.rs)）与**检出**（按 git common dir，跨 worktree
  算同一个）。取哪个是决策，不是事实。
- **一个会话是一个可搬运的目录**：一份只追加的 JSONL 事件流加 `outputs/`。跨会话的消息往哪儿落，
  与这条形状正面相撞。

## 调研给出的形状（初判，非结论）

- **形态 B = 四个零件**：成员（各自独立上下文）、信箱（成员之间直接说话）、共享任务板（谁都能
  看、谁都能领）、lead（普通成员 + 三样治理权）。上游三家先例 —— Claude Code 的 agent teams
  （实验性、默认关、只对交互式会话生效）、DSH 的 `dsh-experimental-agent-team`（实验包）、
  Cline 的 Agent Teams —— **都还是实验性/新功能**。
- **原理**：编排权在模型（lead 调工具决定分工），运行时只提供三件机制，**没有一家做运行时自动
  调度**。形态 B 相对形态 A（工具派生，主流）的增量只有：**成员间直接通信 + 共享板 + 可寻址成员**。
- **收益有硬边界**：Anthropic 内部研究评测上多 agent 比单 agent 高 **90.2%**，但那是研究类
  breadth-first 查询，代价是 **token ×15**；他们自己写明「**大多数编码任务可并行的部分比研究
  少得多**」，且「agent 还不擅长实时互相协调」。反向证据：mini-SWE-agent 无 subagent 仍有
  >74% SWE-bench verified。详见
  [`research/05…`](research/05-comparison-and-recommendation.md) §3.4。
- **对 heng 的增量**：现有**执行者**已经拿走了「上下文隔离 + 并行执行 + 写互斥」这块；形态 B 的
  增量要在**大范围排查 / 跨天长任务 / 一个会话写、另一个会话只读 review** 这三类场景才划算。
- **候选形态**：**形态 B′** —— 团队作用域按**工作区**（而不是按会话），成员是 heng 已有的一批
  同工作区会话（不是 lead 派生的子 agent）。三家上游都没做这一条（它们的成员都得由 lead 生成），
  所以它是 heng 自己的选择，不是照抄。

## 待谈的分叉

> 下面七条在调研里各有初判，见
> [`research/05…`](research/05-comparison-and-recommendation.md) §8 —— **那些是待议建议，不是结论**。

1. 「同目录」按**桶**（cwd）还是按**检出**（跨 worktree 同一个）？
2. 消息落在哪儿：进收件方那一份 JSONL（于是可搬运、可重放、进上下文），还是另开一份团队级的目录？
3. 收件方怎么被唤醒：排到边界、立刻打断，还是只在空闲时投递 —— 与
   [`interjection-flow`](../interjection-flow/seed.md) 相接。
4. 团队的形态：对等的信箱加共享任务板，还是 `spawn` / `send` / `wait` 那一套？现有的**执行者**
   要不要升格成团队成员？
5. 人怎么看见：多个会话同时在屏上时，团队关系（谁给谁发了什么、任务板谁在动）画在哪一层。
6. 与**讨论会话**的分界：那是同一份流的多个发言人，团队是多份流。
7. 要不要一条「这个目录下现在有谁」的清单 —— 以及它跟左栏会话页那份列表的关系。

相关：[`multi-role-view`](../multi-role-view/seed.md)、[`interjection-flow`](../interjection-flow/seed.md)、
[`session-tab` spec](../session-tab/spec.md)（宿主表与前台指针）。
