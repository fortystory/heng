# 架构深化（wayfinder map）

Label: `wayfinder:map`
Tracker: local markdown —— 见 [`docs/agents/issue-tracker.md`](../../docs/agents/issue-tracker.md)
Charting: **已完成**（2026-10-18，二轮 `/improve-codebase-architecture` 后落出的四张 task 票；charting 一轮 grilling：destination 的形状与编排图、依赖与优先级、`03` 的 spec 前置、`02` 与左栏四页各自成 module 的接缝、本图与 implementation 的分工）。本图只做**规划**，不产代码改动。

> **来源与历史**：[`seed.md`](seed.md)（2026-10-12 起点 + 2026-10-18 二轮），目录式摘要 [`report-2026-10-18.md`](report-2026-10-18.md)，HTML [`report-2026-10-18.html`](report-2026-10-18.html)。第一轮的「`View` 是投影的 seam」「宿主表的投影只算一处」「字面量 `chars().take`」「左栏四页各自成 module」四条在这个 effort 里升级、合并或保留；`pane` 平行表已自愈，从候选池移走。

## 目的地

四块**具体的 seam 落地**（不是一份 spec —— 每个 ticket 的验收已经能直接对着代码做）：把 10-18 报告里挑出的「**形态问题已经升到架构问题**」那几条候选打到 `src/render/` 上各自的位置 —— `key()` 的 11 段早期 return 收成一张 dispatch 表、`TuiState` 上同一会话的 5 张 `HashMap` 收成 1 张、`HostFacts` 同时被 cli 与 tui 订阅、`width` 模块成为唯一的截断口。**走到这张图的尽头时，候选池里那四条再也没有需要先决定的事**，剩下的只是接线与断言。

**范围**：`src/render/tui.rs`（18563 行、`key()` 1006 行）、`src/render/width.rs`（97 行）、`src/cli/interactive.rs` 的 80ms 轮询、`ConsoleHandle` 与 `ConsoleRequest`（`src/render/input.rs:311`）、`src/render/transcript.rs`、`src/render/wording.rs`、`src/render/changes.rs`、`src/render/files.rs`（六处 `chars().take`）、[`CONTEXT.md`](../../CONTEXT.md) 的相关词条、[`docs/render.md`](../../docs/render.md)。**事件 schema、模型可见文本、`FileIndex` 的遍历规则、权限门与沙箱边界、plain / headless 的可见输出** 一律不动。

**不在范围内**（留在 seed 那两个"暂缓"里，等这次 chart 之后再决定）：措辞层回到「值 → 短语」、`headless` 做 `Block` 的第三个 adapter、左栏四页各自成 module、命中测试两套机制（`Regions` 与十几处几何重算）。

## 笔记

- **领域**：衡（heng）的 TUI 渲染器 + cli 的事实投影。栈 = `ratatui` + `crossterm` + `tokio`，不加新 crate（`src/tools/process.rs` 现成的「跑子进程、收 stdout、带超时」足够 host-facts 用）。
- **起点**（charting 实测，每条带证据）：
  1. **`key()` 已经是事实上的「按层持有」，只是没显式成 trait/struct**：`tui.rs:7593-8599`（1006 行）的 11 段早期 return 顺序读 `picker` / `replay` / `questionnaire` / `drag` / `detail` / `file_viewer` / `keymap mode` / `suspend` / `trace_keyboard` / `sidebar_keyboard` / 「落到输入区」；`keyboard_in_the_input`（`:15133`）的判据与它**几乎同形但不重合**（差 `right_wanted && trace_keyboard` 那条）⇒ `key()` 加一层就改一处、判据光标加一处、又改一处。`click_at`（`:4957`）/ `wheel_at`（`:5024`）/ `take_input_keyboard`（`:6267`）/ `take_sidebar_keyboard`（`:6076`）/ `take_trace_keyboard`（`:6132`）/ `release_trace_keyboard`（`:6141`）六个入口各持一份字段子集（`src/render/tui.rs:4957/5024/6076/6132/6141/6267`、`tui.rs:15133`）。
  2. **`TuiState` 上同一会话的 5 张 `HashMap`**：`views` / `states` / `light` / `catalogs` / `read_only_by`；`View` 57 字段 / `SessionLight` 18 字段；`stash_light` / `load_light` 写在 `tui.rs:4393/4421`（手写搬运清单）。
  3. **`HostFacts` 现在是事实的影子**：`ConsoleHandle` 12+ `pub fn`（`src/render/input.rs:311`）+ `ConsoleRequest` 22 变体；`cli/interactive.rs:572` 一条 80ms 轮询线程算上一帧快照；`TuiState` 各会话一份承接字段。三条规则（未读寿命、`N` 只算后台、单会话标题不变）今天只能走真机走查（[`docs/tui-manual-checklist.md`](../../docs/tui-manual-checklist.md) ㊶）。
  4. **`width` 已收住三件**：`truncate_columns` / `ellipsize_line` / `slice_columns`（`src/render/width.rs`，97 行），上层（`panel.rs` / `todo.rs`）已走它；下层仍按 `chars().take` 拼 6 处：`transcript.rs:663`、`wording.rs:222/963`、`changes.rs:589`、`files.rs:118`、`tui.rs:15059`（10-18 新发现）。`markdown::wrap_code` 按列自折一份。
  5. **既有 seam 形态**：[ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)（问卷的「按区分发」，一级分派是 `Zone`）；trace-ledger 票 18 把「覆盖层 / 左栏 / 输入区 / 右栏」写成四级阶梯（`tui.rs:8527` 的 `if self.right_wanted && self.trace_keyboard` 是这层阶梯在 `key()` 阶梯里的入口）——**新增的 seam 不要再把这两条决定重开**。
- **要咨询的 skills**：`/grilling`（04 的 `truncate_chars` 出口口径）、`/prototype`（01 的 seam 形状与 `KeyOutcome` 三态落点）。
- **每张票的答案必须自足**：本 effort 的 ticket 验收已经写到代码级；`/implement` 在别处的会话里读它，看不到本图与 charting 对话。
- **前置 spec**：[`03 — host → cli → tui 的事实投影只算一处`](issues/03-host-facts.md) 的「注意」段自陈在动手前要先在 spec 里钉死「宿主表空 / 单会话 / 多会话」三态的读数（与 `model-switching/spec.md` 单会话默认行为耦合）—— chart 这一层标「前置 spec 是开工前的子任务」，**不**拆成额外 ticket（spec 的内容是 ticket 03 验收的一部分，复制形状跟着它走）。
- **验收基线**（charting 时**未跑**，落地前先实测一次）：`cargo test --all-targets`、`cargo clippy --all-targets`、`cargo fmt --check`。
- **改动键盘归属的既有断言**：`tests/render_tui.rs` 里 7+ 个测试直读 `page.trace_keyboard`（`tui.rs:16566-16668`、`tui.rs:17953-18164`、`tui.rs:18321-18466`）—— 新的 seam 不能简单去掉这个字段，否则既有断言全部崩塌；seam 在「字段名还在，但内部实现换了」这一档。
- **同一会话状态收拢后的既有断言**：`session-panes` 票 23 / 25 落地的 5 张 `HashMap` 在测试里被直读的位置要逐条清掉（`tests/render_tui.rs` + `tests/session_panes.rs`）；落地时一并改。
- **width 截断既有断言**：[`tests/wording.rs`](../../tests/wording.rs) 三张 + `tests/render_markdown.rs` 一张，断言「按列」是 fixture 的实际写法，不能改成「按字」断言。

### 冻结项（charting 的 grilling 定下，票里不得重开）

1. **destination 是「在代码上把四个 seam 落地」**，本图只产决策与依赖关系，「做」发生在 `/implement`。
2. **键盘归属 seam 走 ADR 0010 同形**：`KeyboardLayer` trait（`owns_keyboard() + handle_key() -> KeyOutcome`），`KeyOutcome { Handled, NotMine, BubbleUp }` 三态；`key()` / `click_at` / `wheel_at` 共一张 dispatch 表（不另开一份"持指针者"分支）。**不**改`keyboard_in_the_input` 的字段名（仍叫 `trace_keyboard`）。
3. **同一会话状态走一份 `SessionProjection` module**：`View` + `SessionLight` 收一处；`TuiState` 只持 `HashMap<SessionId, SessionProjection>`；`stash` / `load` 收成一份迁移函数；新增字段只动这一个 module。
4. **host 事实投影走 `HostFacts` module + 一份订阅**：cli 与 tui 都从它读；`ConsoleRequest` 22 变体收到 ≤ 5 个，剩 3 条规则（未读寿命、`N` 只算后台、单会话标题不变）落到单元测试。
5. **width seam 是「按列截」唯一出口**：6 处 `chars().take` 一律换 `width::truncate_columns`；带样式的换 `width::ellipsize_line`；`width` 加 `truncate_chars`（真的按字符截的少数几处出口），但「按列 / 按字」两把尺子的判据写进 module 的 doc 注释。
6. **04 是 01 的 tracer bullet**：01 是首选候选，但 04 的改动半径最小、最先能给"整条 seam 形态落到了代码上"的回执；其他三张票（01 / 02 / 03）开工前先确认 04 已经把 `width` 的接缝真打通了。
7. **优先级 = 04 → 01 → 02 → 03**：04 最小、最先动；01 是首选候选、第二位；02 是 10-17 才冒出来的形态问题、趁没长更大；第三是 03（额外要做 spec 那一段）。
8. **范围不扩**：四个 seam 之外的形态问题（措辞层、`headless` 第三个 adapter、左栏各页成 module、命中测试两套机制）继续留在 seed 的「暂缓」段。
9. **本图的执行**：本图只产决策、依赖、优先级与一个 spec 前置说明。「做」发生在 `/implement`。

### 会撞的既有决定（落地前回看，不在本图改）

- **[ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)**：问卷的「按区分发」是 01 同形的先例；落地时拿两段对照评审。
- **[`trace-ledger/spec.md`](../trace-ledger/spec.md) §4 与票 18**（keyboard_layer_and_selection）：覆盖层 / 左栏 / 输入区 / 右栏的四级阶梯是它写下的——seam 不要把它重开为一条更长的清单，而是在它的四个层上加 `owns_keyboard()`。
- **[`session-panes/spec.md`](../session-panes/spec.md) 票 23 / 25**：5 张 `HashMap` 的当前签名与 `stash` / `load` 的形状是它写下来的——02 把它收进 `SessionProjection` 时不要碰这两张票给出的对外契约（提供 `SessionId` 的接口、订阅流、测试断言线）。
- **[`model-switching/spec.md`](../model-switching/spec.md) §10 / Picker**：`Picker` 通道是它写下的——01 的「覆盖层」一支里 `picker.is_some()` 先于其他覆盖层读的那段（`:7674` 的 `if self.picker.is_some() && self.pending.is_none() { ... }`）照旧保留，不并入 seam 的 `KeyOutcome`。换句话说：Picker 是「已经按层持有」的一份现成实现，seam 把它的形态抬进 trait，但**不改它对外的语义**。
- **[`tui-feedback/spec.md`](../tui-feedback/spec.md) §5**（`Esc` 撤选）：「撤掉手上这一手」是它写下来的——`key()` 起始那条 `if key == Key::Esc && self.drag.is_some()`（`tui.rs:7606`）在 01 的阶梯里被层 `Drag` 取代，但**这条 Esc 的早返回优先级要保留**（seam 不把它当 `BubbleUp` 让上层再读一次）。
- **[`suspend-gesture/spec.md`](../suspend-gesture/spec.md) §2**（`Ctrl-Z` 一律先于一切）：seam 的最前一层仍然是「suspend」，**`Ctrl-Z` 不进 dispatch 表**。
- **[`exit-gesture/spec.md`](../exit-gesture/spec.md)**（双击 `Ctrl-C` / `Ctrl-D`）：`Ctrl-C` / `Ctrl-D` 仍走输入区那条路（第一份 dispatch 表之后才进），不是 seam 的覆盖层。
- **[`docs/render.md`](../../docs/render.md)**：文档即契约，01 / 02 / 03 落地要同步改它对应的形态词条；[`check-doc-size.py`](../../scripts/check-doc-size.py) 管着单元 ≤500 字符。
- **[`docs/tui-manual-checklist.md`](../../docs/tui-manual-checklist.md) ㊶**：host 事实投影的三条规则目前只能真机走查；03 落地后这里 ㊶ 要被改写成「单元测试覆盖，真机这条作废」或「规则不变但测试已钉」。
- **[ADR 0008](../../docs/adr/0008-markdown-parsing-by-pulldown-cmark.md)**：`markdown::wrap_code` 按列自折正是它那句「换行归我们」的同一档；04 落地把它一并收进 `width`。

### Tracker 事实与降级（本图适用）

- map = `.scratch/architecture-deepening/map.md`，child = `.scratch/architecture-deepening/issues/NN-*.md`；阻塞 = 票面 `Blocked by:`；claim = `Status: claimed`（`task` 票被 agent 认领后改这个状态）；resolve = 票底 `## 作答` + `Status: resolved` + 追加一行到本文 `已定的决定`。
- **`task` 票的 `Status:` 术语**：charting 阶段 4 张子票暂写 `ready-for-agent`（沿用实现票的话术，方便把它们丢给 `/implement`）。`/implement` 认领后改成 `claimed`，写完走 `resolved`（按 wayfinder 的 `claimed → resolved` 套，不走实现票的 `done`），并在 map 的 `已定的决定` 收一条。
- **没有 native sub-issue / 依赖边**，回退到正文约定：本文 `## 任务清单` 逐条引用子票（条目数 == 子票文件数），每张子票顶部写 `Part of: ../map.md`。
- 校验：`python3 scripts/wayfinder-check.py .scratch/architecture-deepening/map.md`。宣布图走完之前必须 PASS。

## 任务清单

<!-- 条目数必须等于 issues/ 下的子票数（scripts/wayfinder-check.py 校验）。frontier 的权威查询仍是扫 issues/ 里 open + unblocked + unclaimed 的票。 -->

**task 票**（四张，10-18 报告挑出的四块 seam 落地）：

- [x] [键盘归属收到一处](issues/01-keyboard-owner.md)
- [x] [同一会话状态，收到一处](issues/02-session-projection.md)
- [x] [host → cli → tui 的事实投影只算一处](issues/03-host-facts.md)
- [x] [width module 的 seam 收齐](issues/04-width-seam.md)

**优先级与依赖**：

- 04（width）无前置、半径最小、最先动；**它是 01 的 tracer bullet**（先打穿 width 的 seam 形态，其余三张照着做）。
- 01（key dispatch）无前置、第二位；是报告里的首选候选。
- 02（`SessionProjection`）无前置、第三位；10-17 才冒出来的形态问题，趁没长更大。
- 03（`HostFacts`）有**前置 spec**（在 ticket 内 `## 注意` 段自陈）；开工前要先在它的 §前置 spec 里钉死「宿主表空 / 单会话 / 多会话」三态的读数；之后才动 `ConsoleRequest` 与订阅流。

**frontier（2026-10-19，四张全部 resolved）= 空 —— 本图走完。** 03 在动手前先钉了「宿主表空 / 单会话 / 多会话」三态读数（票内 §前置 spec），随后把宿主事实收成 `HostFacts` 一份快照、`ConsoleRequest` 20 → 4 变体、80ms 轮询收成一处。

## 已定的决定

<!-- 索引：每条一行，够判断相关性即可；细节住在票里。按名字引用，不写裸编号。 -->

- [任务清单](issues/) 这一节本身 = **destination 是 change 不是 spec**：四张 ticket 已经把验收写到代码级，每张可直接丢给 `/implement`；chart 这一层只判 destination 的形状、依赖、优先级与 03 的 spec 前置，不写新的 spec。
- [键盘归属收到一处](issues/01-keyboard-owner.md) — **resolved 2026-10-19**（上半 10-18 骨架、下半本轮接完）。seam 同 [ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md) 形状（`KeyboardLayer` trait + 三态 `KeyOutcome`），7 个 layer 收在 `tui.rs` 的 `keyboard_layers` 模块，`key()` / `click_at` / `wheel_at` 走同一张 dispatch 表；`TuiState::key()` 从 267 行降到约 30 行，只剩 `Ctrl-Z` 与 `Esc` 撤选两条 seam 外早返回；保留 `trace_keyboard` / `sidebar_keyboard` 字段名与既有断言线，`keyboard_in_the_input` 改为从层表派生；前置两条早返回保留在 seam 外，不进 `KeyOutcome`。
- [同一会话状态，收到一处](issues/02-session-projection.md) — **resolved 2026-10-19**。`View` + `SessionLight` 并入 `SessionProjection` module，`TuiState` 只持 `HashMap<SessionId, SessionProjection>`；`stash` / `load` 收成一份迁移函数；不碰 `session-panes` 票 23 / 25 给出的对外契约；测试断言的 5 张 `HashMap` 读法逐条改写。
- [host → cli → tui 的事实投影只算一处](issues/03-host-facts.md) — **resolved 2026-10-19**。`HostFacts` 同时被 cli 与 tui 订阅；`ConsoleRequest` 22 变体收到 ≤ 5 个；**前置 spec** 钉「宿主表空 / 单会话 / 多会话」三态读数，后做订阅改造；三条规则（未读寿命、`N` 只算后台、单会话标题不变）落到单元测试，[`docs/tui-manual-checklist.md`](../../docs/tui-manual-checklist.md) ㊶ 同步改写。
- [width module 的 seam 收齐](issues/04-width-seam.md) — **resolved 2026-10-18（tracer bullet）**。`src/render/width.rs` 新增 `pub fn truncate_chars` 作为按字符出口（与 `truncate_columns` 并列），module 顶 doc 写清「按显示列 / 按字符」两把尺子的判据；6 处 `chars().take`（`files.rs:118` / `changes.rs:589` / `transcript.rs:667` / `wording.rs:222` / `wording.rs:963` / `tui.rs:15059`）一律换 `truncate_chars`；`markdown::wrap_code` 已在 `width::char_columns` 上、不动。**修原票验收 2**：原写「`truncate_columns` ≥ 9」，实际 6 处全是按字符口径——以「`truncate_chars` 在 `src/render/` 命中 ≥ 6」等价收尾。**新增单测 3 条**钉这两把尺子不能互换（同串 `"衡a衡b"` 上 `truncate_columns(_, 4) == "衡a"` / `truncate_chars(_, 4) == "衡a衡b"`）。`render/` 外的 8 处 `chars().take` 不在本票范围。

## 尚未明确

<!-- 在范围内、但现在还说不精确的雾。随前沿推进毕业成票。不要预先切成票那么大。 -->

- **`KeyboardLayer` 的层枚举位置**：候选项（a）放 `tui.rs` 内部 `mod`、（b）放 `tui.rs` 之外一个新 module（`src/render/key_layer.rs`）、（c）拆四层进各自的 page module（trace / sidebar / overlay）。倾向 (a)，但落地由实现期写第一刀时定。
- **`SessionProjection` 的迁移函数签名**：`stash(&mut self, Projection)` vs `stash(Projection) -> Stash`（带不带自持指针）—— 跟 `session-panes` 票 23 / 25 给出的对外契约如何对齐，**01 / 02 / 04 落地后**再决定；不是本图的决策。
- **`HostFacts` 的"事件"shape**：22 个变体收到 ≤ 5 个之后，那 5 个是什么（候选：`Snapshot` / `New` / `Gone` / `TitleChanged` / `UnreadChanged`）—— 跟 `ConsoleHandle` 现在的 12 个 push 在语义层一一映射的哪里，留给 03 的 spec 钉。
- **`width::truncate_chars` 的真出口有哪些**：现在 `chars().take` 不全是想按列截，**有几处是真的按字截**（比如 identifier 的安全截断）—— 04 落地时先把 6 处挨个标"按列 / 按字"，再决定 `truncate_chars` 接口要不要暴露。这件事是 04 自己的活，不另开 ticket。
- **「左栏四页各自成 module」与 02 的关系**：那条与 02 都是「把 TuiState 的子结构收进 module」，但前者说的是 view、后者说的是 session。两件事的 seam 形状是否能同形（同一套 macro / trait）—— 留在 chart 之外，下次报告再判。

## 明确不做

<!-- 有意识排除在本次 effort 之外的工作；永不毕业。 -->

- **措辞层回到「值 → 短语」**：留在 seed 的「暂缓」。
- **`headless` 做 `Block` 的第三个 adapter**：同上。
- **左栏四页各自成 module**：同上，但与 02 同形这件事可能会在下次报告里再出现，那时单独立图。
- **命中测试两套机制**：`Regions` 与十几处几何重算并存；本次 chart 不动候选。
- **新增 `KeyboardLayer` 在 Picker / Suspend 之外再建新覆盖层**：seam 的形态要在「已存在的四 / 五层」上立，新覆盖层不是本图的产物；想加的 ticket 单独开。
- **`ConsoleHandle` / `ConsoleRequest` 的轮询线程替换**：`tokio` 的 `process` 组已经在用，时钟不动；03 改的是事实投影的形状、不动 `tokio` 的时钟。
- **`FileIndex` 与 `HostFacts` 合并**：两件事的职责不重合（前者是 `bash` / `glob` / `grep` 的入口流量，后者是宿主进程表的形状）；不合并。
- **本图的执行**：本图只产决策、依赖与优先级。「做」发生在 `/implement`；每张子票一次 `/implement`、写完走 `/code-review` 双轴。

## 进度

**4/4（2026-10-19，四张全部 resolved）—— 本图走完。** 最后一张 [03 — host → cli → tui 的事实投影只算一处](issues/03-host-facts.md)：新增 `src/session/host_facts.rs`（`HostSample` / `HostFacts` / `HostTracker`），三条规则（未读寿命、`N` 只算后台、单会话标题不变）由四条单测钉死；`ConsoleRequest` 从 20 个变体收成 4 个（`Prompt` / `Ask` / `Facts` / `Notify`）；`cli/interactive.rs` 的 80ms 轮询只调 `HostTracker::update` 一处，`loading.rs` / `close_session` 的直接推送删掉。`docs/tui-manual-checklist.md` ㊶ 第 4 条补注「规则已由单测钉住」。全量测试 1805 passed / 0 failed、clippy 0 warning、fmt 干净。

四张 seam 全部落地（01 键盘归属、02 会话状态、03 宿主事实、04 width）。frontier 为空；
「做」到此收束，本图留作决策与验收记录。
