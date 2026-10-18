# 架构深化：候选池（2026-10-12 起点 + 2026-10-18 第二轮）

## 来源

- **2026-10-12** 第一轮 `/improve-codebase-architecture`：九条候选，两条已落地（[`session-tab` 票 33 / 34](../session-tab/issues/34-attach-one-action.md) 与 [`cli-modules` spec](../cli-modules/spec.md)）。原文留作历史快照在 [`seed-2026-10-12.md`](seed-2026-10-12.md)。
- **2026-10-18** 第二轮（见 [`report-2026-10-18.md`](report-2026-10-18.md) 与 HTML 版 [`report-2026-10-18.html`](report-2026-10-18.html)）：扫描 2026-10-12 之后的热点（`src/render/tui.rs` 涨 6000 行、session-panes 30+ 张票落地）。**新增三条**（键盘归属升级、会话状态双轨升级、事实投影一处新增），**标记一条自愈**（pane 平行表）。其余条目行号与现象同步到 10-18 现状。

**这是意向池，还没有折成 spec**：想推进哪一条，就走一轮访谈把它折成 spec 再拆票。

判据一句话：**locality 与 leverage** —— 改动能不能集中到一处，一处 interface 能不能供多个 caller
与测试使用。每条末尾括号里是报告给的推荐强度。

## 渲染层

- **把截断收回 `width` module**（Strong → Worth exploring）：折行只有 `pane::wrap_line` 一处，
  截断六处按 `chars().take` 拼 —— `transcript.rs:663`、`wording.rs:222` / `:963`、
  `changes.rs:589`、`files.rs:118`、`tui.rs:15059`（10-18 新发现）—— 中文界面里字符数不等于
  列数；`markdown::wrap_code` 仍自折一份。`panel.rs / todo.rs` 已走 `width::truncate_columns`，seam
  是「半截收」。与 [ADR 0008](../../docs/adr/0008-markdown-parsing-by-pulldown-cmark.md) 同向。
- **左栏四页各自成 module**（Strong）：`SessionsPage` 已有 `impl` 一段（`tui.rs:10288-10360`），
  `FilesPage` / `ChangesPage` 仍是裸字段 + 三个 draw fn；互清 `rect` 从 10-12 时的两处涨到 12+
  处（`tui.rs:9219/9220/9319/9322/.../9726`）。seam 放在「一页」。
- **谁持键盘，一处判据**（Strong → Strong，**已升级到架构级**）：`key()`（`tui.rs:7593-8599`，1006 行）
  是 11 段早期 return；`trace_keyboard` 的注释（`tui.rs:2639`）自陈「仓库里**第四套**「键盘在谁那里」
  的机制」 —— 之前的左栏布尔、两页焦点索引、问卷反显还没退，又叠一套。
  `click_at` / `wheel_at` / `keyboard_in_the_input` 各持一份字段子集。seam：「层」——
  每层自己声明是否持有。先例 [ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)。
- **`headless` 做 `Block` 的第三个 adapter**（Strong）：`headless.rs:103` 自养叙述，`transcript.rs`
  32 个、`tui.rs` 画家 53 处；同一句话两处拼（`headless.rs:141` 与 `plain.rs:115`）。
  代价：`BlockId` 身份记账（[ADR 0021](../../docs/adr/0021-line-identity-comes-from-the-event-envelope.md)）
  进机器模式。
- **同一会话状态收到一处**（Worth exploring → Strong，**已升级**）：session-panes 票 23 / 25
  落地后，`TuiState` 多了 5 份按 `SessionId` 平行的 `HashMap`（`views` / `states` / `light` /
  `catalogs` / `read_only_by`）。`View`（57 字段）与 `SessionLight`（18 字段）各持一份会话
  切片，`stash_light` / `load_light`（`tui.rs:4393/4421`）仍是手写搬运。seam：「一个会话的投影」——
  `SessionProjection` module 把 `View` + `SessionLight` 收一起，`TuiState` 仅持
  `HashMap<SessionId, SessionProjection>`。
- **措辞层回到「值 → 短语」**（Worth exploring）：`wording.rs` 4032 行，200+ 条 `pub fn`；
  一半纯短语、一半排版决策（`status_row:2983`、`hint_line:8497`、`bottom_rows:8583`）。
  seam：「哪几段、按什么顺序放弃」一处。

## 命中与平行表

- **命中测试两套机制**：`Regions`（`layout.rs`）只覆盖一部分动作，另一半各自重算几何（`tui.rs:5024/7848/4957/6696/6735`）。2026-09-17 又因 `trace_rect` 与 `layout::Regions` 同时存在多碰一处（10-18 加深）。
- ~~**`pane` 的平行表**~~（**已自愈**，10-18 转 ADR 引用）：`Pane::push() -> usize`（`pane.rs:124`）
  把「丢了几条」收进 signature，`trace_links` / `trace_block_ids` / `TurnRail::prune(dropped)` 三处按
  同一个返回值裁。2026-09-23 那次「历史从屏幕上消失」之后的修复已稳定，这条从候选池挪走。

## cli 与会话

- **host → cli → tui 的事实投影只算一处**（Strong，**10-18 新增**）：原 `seed-2026-10-12` 那条
  「宿主表的投影只算一处」升级而成。「宿主表现在长什么样」算成三处 —— `cli/interactive.rs:572` 80ms
  轮询算上一帧快照、`ConsoleHandle` 12+ 推命令、`TuiState` 各会话一份快照。`ConsoleRequest`
  22 变体。渲染层要认的领域事实从 6 种涨到 10+ 种，每种一条 push 命令。
  seam：`HostFacts` module 同时被 cli 与 tui 订阅，三条规则（未读的寿命、`N` 只算后台、
  单会话标题不变）落到单元测试。
- **命令名的知识两处**：对拍测试自己写着「两边各有一份真相」（拆分前在 `cli.rs:6681`）。
- **渲染器要认识的领域事实有六种碎片**（10-18 已涨到 10+ 种）。
- **会话级轻状态是一张手写搬运清单**：被「同一会话状态收到一处」合并。

## 欠账

- **可注入材料的测试装置**（票 34 留下）：`cli::loading::attach` 的端到端依赖真 `AssemblyKit`（真
  provider、真 MCP）。把材料里那几样拆成可替换的输入之后，装载路径才能在没终端、没网络的测试里断言
  「装完说哪几句」。
