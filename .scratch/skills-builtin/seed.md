# 种子材料：技能内置与 tracker 落点

> **这不是 spec，也不是票。** 它是 2026-10-12 一轮 `/wayfinder` charting 里记下的意向：
> 「把 matt 技能做成 heng 内部工具」以及「做成之后能不能把 `.scratch/` 的文档放进数据库或单独项目」。
> 那天只探完事实、定下目的地，第二轮提问被取消，**没有访谈完、没有票**；想推进时走
> `/grill-with-docs` 把它折成 `spec.md`，再 `/to-tickets` 拆票。
> **写就于 2026-10-12**；下面《现状》一节核实于同一天。

## 它要什么

两件事，动机上是一条线：技能随 heng 走；技能依赖的 tracker 数据从产品仓库里出去。

1. **技能内置 + 触发更硬**：把现在装在 `~/.agents/skills/` 的那套 matt 技能收进 heng ——
   正文随仓库 / 二进制分发（离线可用、版本固定），并让触发不再只靠「模型自己想起来」。
   2026-10-12 那轮勾的是这两项，形态与机制都未定。
2. **tracker 落点外迁**：把 `.scratch/` 的 markdown（spec / map / 票）搬去数据库或**单独项目**。
   那轮的结论是「两者都要评估，先定判据再选」，而判据本身还没定。

## 现状（2026-10-12 核实）

- **技能已经被 heng 读进来了**：发现根是项目级 `<repo>/.heng|.agents|.claude/skills` 与用户级
  `~/.config/heng|.agents|.claude/skills`（`src/context/skills.rs:46`、`docs/skills.md:32`）。
  本机 `~/.agents/skills/` 有 40 个目录，含 `wayfinder` / `to-tickets` / `grill-with-docs`，
  **不内置也能用**。缺的是「随 heng 版本固定 / 离线 / 中文真源唯一」：那套中文 fork 来自
  `vinvcn/mattpocock-skills-zh-CN`，由外部 `skills` 安装器写 `~/.agents/.skill-lock.json`。
- **DSH 侧那个插件**（`dsh-mattpocock-skills-deck`）本质是「技能包 + 三后端 tracker 适配层 +
  右侧面板 + 刷新预算闸」；tracker 契约在 `lib/tracker/contract.js`，后端有 markdown（落
  `.scratch/<effort>/`）、github、gitlab，统一实体的边只有 `parentKey` 与 `blockedBy`。
  它内置的技能是上游英文快照 v1.2.3，与中文 fork 逐文件不同。
- **tracker 的三条现成承载路**：
  - **内建 Rust 工具**：撞两条既有纪律 —— 工具表在组装期定死、它就是请求前缀的一部分
    （`src/tools/mod.rs:113`、`src/agent.rs:629`），以及「加内置工具先要用量证据」
    （`.scratch/tool-coverage/spec.md` 的「明确不做」一节）。
  - **MCP stdio server**：不动 Rust（`docs/mcp.md:75`），但模型侧只有 `mcp_call` 一类元工具、
    `readonly` 档默认拒、写入不进 `ReadSet` 与路径护栏。
  - **动态工具**（`[tools.<ns>.<tool>]`）：`effect()` 恒为 `Exclusive`（`docs/custom-tools.md:36`）。
- **搬走 `.scratch/` 会打断四条自动化**：`scripts/check-doc-size.py`（`DOC_FILES` 含
  `.scratch/README.md`，缺文件非零退出）、`scripts/check-language.py`（`SCRATCH_DIR = ".scratch"`，
  一份都没有即报问题）、`scripts/wayfinder-check.py`（按 `map.md` 的任务清单与 `issues/*.md`
  目录形状对账）、`/goal-new`（`src/goals.rs:226` 直接读票，`tests/goals.rs` 同源）。
  另有几十处文档指针（`AGENTS.md`、`docs/agents/issue-tracker.md`、`CONTEXT.md`、`docs/**`）。
- **写着的禁令**：`README.md:320` 的「这一版不做」含 **SQLite** 与「把工具打包进 skill」；
  `mcp-support` 写着「把 server 的工具铺进工具表永不做」；`fs-agent-v1/spec.md` 要求动这些
  先改 spec。留着的口子只有一处：`.scratch/docs-slim/spec.md:254` 把「入口文档的状态段该不该由
  `.scratch/` 派生」留给后续 effort，无结论。

## 待谈的分叉

1. **痛点是哪一个**：分发与版本不可控 / 模型想不起来用 / 流程无法强制 / 技能之间没有编排 ——
   选哪个，决定了是只动分发还是动机制。
2. **内置形态**：仓库内 `skills/` 目录（零代码改动），编进二进制（新增一个内建发现根 + 兜底
   provider），还是只固定版本引用（submodule / vendor）。
3. **「触发更硬」到什么程度**：清单措辞、显式触发规则、hook 式强制（hook 今天是 Rust trait、
   无外部挂载口，`src/hooks.rs:169`），还是技能链编排。
4. **外迁判据**：先定消费者（四条自动化与 `/goal-new` 怎么办），还是先定存储形态（独立仓库 /
   数据库 / 远端）。
5. **读写入口**：继续直读文件只换落点、内建工具、MCP server，还是远端 tracker。
6. **落点身份与挂法**：独立 git 仓库怎么跟 heng 并排（兄弟目录 / submodule / 另一份检出）；
   数据库要么推翻 SQLite 禁令，要么换形态。
7. **范围**：DSH 侧那套是退役、并存，还是抽成两边共用的契约层。
