# 左栏「会话」页签：历史会话列表与多会话切换（wayfinder map）

Label: `wayfinder:map`
Tracker: local markdown —— 见 `docs/agents/issue-tracker.md`
Charting: **已完成**（2026-10-12，四轮 grilling / 16 问）。本图只做**规划**，不产实现代码。
Status: **图已收口**（10 张设计票 resolved；实现票 21 张 done + 票 32 于 2026-10-12 转 `Type: implement`、`ready-for-agent`）。
Spec: [`spec.md`](spec.md) —— 2026-10-12 由本图折成，已打 `ready-for-agent`，实现交给 `/to-tickets` 切票。

## 目的地

一份 **spec-ready 的「会话页签」决策集**，交给 `/to-spec` 折成 `.scratch/session-tab/spec.md`，再由 `/to-tickets` 切出实现票。

**要解决的问题**：左栏新增第五签 `会话`，列出**本项目及其 git worktree 里跑过的历史会话**（标题、时间、所属工作区），点一条即切过去；标题自动从首条用户消息派生、可手动改名；切走的会话**不卸载**、回合照跑；新建会话是"再加载一个会话空的"，不关旧的。

**为什么它不是"加一个页签"**：现有架构是**一进程一会话**——`Harness` 绑着 cwd、沙箱探测、权限策略、技能、工具表（`src/lib.rs:207-228`），渲染器一进程一个、绝不重建（`src/lib.rs:156-157`），而"切走不卸载 + 后台继续跑"要求同一个进程里同时活着多个会话。所以本图的决策集分两簇：**会话身份 / 列表 / 标题**（对外看得见的那个页签）与**会话宿主 / 切换 / 后台并发**（支撑它的架构）。

**判据**：只产决策，不产实现代码；模型可见文本（ADR 0001／0005）与 `messages` 投影前缀一个字节不动；plain 与 headless 渲染器不受影响。

## 笔记

- **领域**：`heng` —— 自用 coding agent harness（Rust，只追加事件流 + 按 agent 投影）。**词汇表在 `CONTEXT.md`**：**会话**、**会话目录**、**会话桶**、**左栏**、**页签条**、**选项区**、**块**、**转录**。本图新增的词：**检出**与**会话页**由票 02 定、已写进 `CONTEXT.md`；**会话宿主**由票 04 定，定了就写进去。
- **紧邻的既有图与 spec**：`.scratch/tui-sidebar/`（左栏与页签条的几何、宽度两档 40/28、命中矩形、"页签只走鼠标"）、`.scratch/tui-history-replay/`（启动时把历史分帧重播进转录，**同一进程一次**）、`.scratch/continue-by-id/`（`-c <id>` 跨桶续会话、cwd 跟着目标会话走）、`.scratch/goal-loop/`（`Harness::rollover` 已有的"同进程开新会话"、目标额度跨会话）、`.scratch/sidebar-toggle/`（`Ctrl-O`）。
- **既有不变量（票不得违反）**：事件流**只追加**且是唯一真相；**渲染器状态不进流**；模型可见文本冻结；`SCHEMA_VERSION` 随 payload 形状变（`src/events.rs:24`）；一条 `MessageCompleted{role: User}` 是"第一句人话"（`ContextInjected` 不算）；log-only 事件的先例是 [ADR 0019](../../docs/adr/0019-command-records-are-log-only.md)。
- **每张票的答案必须自足**：`/to-spec` 之后，实现会在别的会话里读它，看不到本图与 charting 对话。
- **要咨询的 skills**：`/grilling`（HITL 决策票默认）、`/domain-modeling`（新词与 ADR）、`/research`（research 票 03）、`/prototype`（票 06 的帧）。
- **相关事实（charting 期调研已核实，票里不必重查）**：
  - 左栏现有**四签** `调用量 / todo / 文件 / 改动`（`src/render/tui.rs:8714`、`src/render/wording.rs:2250-2258`）；四签加分隔占 21 列，窄档 28 列再加一个双字签占 26 列，放得下（`tests/render_layout.rs:4456`）。
  - 工作目录只能读每条流的 `SessionStarted.cwd`（桶名是 FNV 哈希、不可逆）；`SessionStore::list_all` 跨桶，`observe::summarize` 会给每条流填 `Listing.cwd`，但**每条流都是全量读 + 全量解析**。
  - 流上**没有记模型**：`SessionStarted { session_id, cwd, schema_version }`。
  - 渲染器状态（transcript / pane / panel / todo / turn_rail / timeline / block_at / turn_totals / trace_*）只在**宽度重排**时从 `painted` 重建，**没有按事件流全量重置的入口**；`ConsoleRequest::Replay` 只追加（票 08 的由来）。
  - `Harness::rollover`（`/clear` 与目标阈值翻页用）已经是"同进程、同渲染器、只换 log 与会话身份"，但它硬编码 `EventLog::create`（只能开**新**会话），且不换 cwd / 沙箱探测 / 技能 / 权限策略。

### Tracker 事实与降级（本图适用）

- 本仓库 issue tracker = **local markdown**：map = `.scratch/session-tab/map.md`，child = `.scratch/session-tab/issues/NN-*.md`，阻塞 = 票面 `Blocked by: NN`，claim = 票面 `Status: claimed`，resolve = `## 作答` + `Status: resolved` + 追加到本文件 `已定的决定`。
  > **核过一次两套 tracker**（2026-10-12）：`deck_context` 报的后端是 GitHub（`fortystory/heng`，0 issue / 0 map），与仓库文档不一致。**本图按维护者的选择落在本地 markdown**，与历史图一致。
- 该后端**没有原生 sub-issue / 依赖边**，所以按既定规则降级：本文件用 `## 任务清单` 逐条引用子票（条目数 == `issues/` 下的文件数），每张子票写 `Part of: ../map.md`。**阻塞关系以票面 `Blocked by:` 为权威**。
- **决策票的 `Status:` 取值**：`open`（本图创建时）→ `claimed`（认领）→ `resolved`（作答后）。实现票（`Type: implement`）另行 `ready-for-agent` → `done`。
- **frontier 判定** = `Status` 不在 `{resolved, done, closed}` + 未被 `Blocked by` 挡住 + 未被认领 + `Type` ≠ `implement`；编号最小者胜出。
- 校验：`python3 scripts/wayfinder-check.py .scratch/session-tab/map.md`。

### 冻结项（charting 四轮 grilling 定下，票里不得重开）

1. **交付物是决策集**（spec-ready），不产实现代码；`/to-spec` 折成 `spec.md` 后再由 `/to-tickets` 切实现票。
2. effort 目录 = `.scratch/session-tab/`，tracker = 本地 markdown。
3. **列表范围 = store 里跑过的全部会话**（`SessionStore::list_all`）。**2026-10-12 被票 02 重开**：原文是「本项目 + 它的 git worktree 里跑过的会话」，那条与冻结项 4 不可兼得 —— 已删 worktree 的 cwd 探测必然失败、桶名又是不可逆的 FNV 哈希，于是「检出已不在」这个标记永远不会出现。代价与连带走 [grilling：会话列表的条目、排序与分组](issues/02-list-rows-and-grouping.md) §0。
4. **聚合依据 = 现场探测**：对候选 cwd 跑 git（`rev-parse --git-common-dir` 一族）认 worktree 家族；**不往 store 里造索引、不改桶结构、不给 `SessionStarted` 之外找第二处真相**。
5. **标题**：自动派生（首条用户消息首行截断，**不落流**）→ 手动改名落一条**新的 log-only 事件**（暂名 `SessionTitled`）；`SCHEMA_VERSION` 2 → 3。**票 03 的实测让这条派生源显得不完整**：147/281 条流没有 `role:User` 消息，另有 4 条落在第 685 / 2549 / 2737 / 13195 行 —— 完整回退链由票 01 定。
6. **宿主 = 同进程多 Harness**：一个会话一份 cwd / 沙箱探测 / 权限策略 / 技能 / 工具表 / `EventLog`；**不引入 IPC、不做独立进程 daemon**。
7. **切走的会话后台继续跑回合**，且**不卸载**；卸载只由手动关闭触发。
8. **后台会话跑出需人答的东西**（权限询问 / 问卷 / `ask_user`）就**停在那一步**，由列表与该页的标记 + 提示行提示，切过去才答（问题不丢）。
9. **切入磁盘上的历史会话时，会话级事实从流上来**：`SessionStarted` 补记 model / effort / 权限模式 / 沙箱档；旧流缺字段则回退「沿用当前前台设置」并如实说明。**2026-10-12 被票 05 定案细化**：补记字段定为 model / provider / effort / 权限模式 / 沙箱 mode 五样（全 `Option` + `serde(default)`）；回退来源改成**进程配置默认**（「当前前台」在启动路径上不存在）；运行中变更另落一条 log-only `SessionFactsChanged`（新范围）；装不成可写会话的降级为**只读预览**（新范围）。
10. **`会话` 页签只走鼠标**（沿用左栏"页签从不给键位"的不变量）；页内是否给键盘焦点（照 `文件`/`改动` 页的先例）由票 06 定。**2026-10-12 票 06 定**：页内给键盘焦点（`↑`/`↓` 移焦点 · `Enter` 切过去 · `r` 改名 · `Esc` 还键盘），`关闭` 只走鼠标、不给键位。
11. **新建会话 = 同 cwd 再加载一个空会话，旧的不关**（不写 `SessionEnded`）。**2026-10-12 票 07 定**：新建后**立即成为前台**；继承前台的模型 / effort / 权限模式，沙箱与技能按同 cwd 重探；入口 = 会话页页顶读数行右端 `新建`（另有既有的 `/clear`，`/loop` 闸门取消）；**可以关掉最后一个会话** → **空宿主**态（页顶 `0 个会话`、页区 `没有会话`、输入区禁用）。
12. **明确不做**：创建 / 删除 git worktree、独立进程 daemon、plain / headless 的会话页、界面里的会话删除（prune）。

## 任务清单

<!-- 条目数必须等于 issues/ 下的子票文件数（scripts/wayfinder-check.py 校验）。frontier 的权威查询仍是扫 issues/ 里 open + unblocked + unclaimed 且 Type ≠ implement 的票。 -->

- [x] [grilling：会话身份与标题](issues/01-session-identity-and-title.md)
- [x] [grilling：会话列表的条目、排序与分组](issues/02-list-rows-and-grouping.md)
- [x] [research：历史会话聚合与列表数据源的事实与成本](issues/03-aggregation-facts-and-cost.md)
- [x] [grilling：多会话宿主与后台并发](issues/04-session-host-and-concurrency.md)
- [x] [grilling：切入历史会话的会话级事实](issues/05-session-facts-on-attach.md)
- [x] [prototype：左栏「会话」页的形态](issues/06-session-page-shape.md)
- [x] [grilling：新建会话与卸载边界](issues/07-new-session-and-unload.md)
- [x] [grilling：前台视图的会话隔离与切换重建](issues/08-front-view-isolation.md)
- [x] [文档回改：lifecycle.md 与逐面文档（实现期执行）](issues/09-doc-revision.md)
- [x] [grilling：会话列表的搜索与过滤](issues/10-session-list-search.md)
- [x] [grilling：后台会话有事时的后台通知](issues/11-offscreen-attention.md)
- [x] [12: 把 TuiState 的会话投影抽成子结构](issues/12-session-projection-struct.md)
- [x] [13: 渲染器与路径锁升进程级、`Harness: Send`](issues/13-process-level-renderer-and-locks.md)
- [x] [14: 渲染事件带 session id 与重放语义修正](issues/14-render-event-session-id.md)
- [x] [15: 宿主表与前台指针：一个会话与今天等价](issues/15-session-host-table.md)
- [x] [16: 会话标题：`SessionTitled`、前缀派生与规格涨版](issues/16-session-title.md)
- [x] [17: 左栏第五签「会话」：列出历史会话（只读）](issues/17-session-page-list.md)
- [x] [18: 装载路径：切入历史会话](issues/18-attach-path.md)
- [x] [19: 切换重建：`SwitchSession` 与 `seq` 补齐](issues/19-switch-rebuild.md)
- [x] [20: 点一行切过去（接入）](issues/20-click-to-switch.md)
- [x] [21: 新建、关闭与空宿主](issues/21-new-session-and-close.md)
- [x] [22: 后台三态与待答挂住](issues/22-background-states-and-awaiting.md)
- [x] [23: 退出闸门与 `Ctrl-C` / `Ctrl-D` 语义](issues/23-exit-gate-and-ctrl-c.md)
- [x] [24: 列表过滤](issues/24-session-list-filter.md)
- [x] [25: 后台通知：终端标题的状态段](issues/25-terminal-title-attention.md)
- [x] [26: 点一行要真的切过去](issues/26-click-a-row-reaches-the-host-table.md)
- [x] [27: 出口提示按新语义拆开，并跟着眼前那个会话走](issues/27-the-exit-hint-follows-the-session-in-front.md)
- [x] [28: `Ctrl-D` 不再被静默吞掉，且退出走双击](issues/28-ctrl-d-never-swallowed-and-asks-twice.md)
- [x] [29: 会话页的标题色分三档（前台 / 已装载 / 只在盘上）](issues/29-session-titles-say-where-a-session-is.md)
- [x] [30: 退出流程不再死锁，提示请求跨轮保活](issues/30-exit-deadlock-and-prompt-liveness.md)
- [x] [31: 切换前台后，会话页的前台标记跟着走](issues/31-foreground-mark-follows-the-switch.md)
- [ ] [32: 关闭的交互：让「要关哪一个会话」在点之前就看得见](issues/32-close-interaction.md)

共 **32** 张子票：**10 张设计票**（01–08、10、11，全部 `resolved`）+ **22 张实现票**（`09` 与
12–31 全部 `done`；32 由 2026-10-12 那一轮访谈从 `grilling` 转成 `implement`、`ready-for-agent`）。
设计票的依赖边：`02 ← 03`、`05 ← 04`、`06 ← 02, 04`、`07 ← 04`、`08 ← 04`；其余票的边写在每张票
自己的 `Blocked by:` 里。**决策 frontier = 空**（唯一那张 grilling 票已转成实现票）。

## 已定的决定

<!-- 每张 resolved 的票一行：一句话 gist + 指向票的链接。细节只存在于票里。 -->

- [research：历史会话聚合与列表数据源的事实与成本](issues/03-aggregation-facts-and-cost.md)：worktree 家族键 = `rev-parse --path-format=absolute --git-common-dir`（单次 1.0–2.3 ms）；真实 store 281 个会话 / 88.5 MB / 74,803 事件，`list_all` + 全量解析 155.6–172.2 ms、只读前 8 行前缀 3.7–4.3 ms；`SessionStarted` 恒为首行（281/281），而 147/281 条流没有首条用户消息、另有 4 条落在第 685 / 2549 / 2737 / 13195 行；失效会话 9 类与「cwd 在但不是仓库」这一态已成表。
- [grilling：会话身份与标题](issues/01-session-identity-and-title.md)：标题 = `SessionTitled { title }`（log-only、按散文打码、空串即撤回）+ 前 8 行前缀派生（3.7–4.8 ms、进程内缓存、兜底 `MM-DD HH:mm`）；改名 = 页内焦点单键、复用常驻 `editor`，另给 CLI `heng sessions title <id> <text>`；`SCHEMA_VERSION` 2 → 3 且与票 05 同一次；终端标题改四段 `<路径> · <标题> · <状态> · <目标名>`，丢序 = 目标名 → 标题 → 状态词。
- [grilling：会话列表的条目、排序与分组](issues/02-list-rows-and-grouping.md)：行 = 标题（票 01 权威链、尾 `…`）+ 右对齐定宽相对时间（窄档标记 1 + 标题 21 + 时间 5，宽档 1 + 30 + 8，标记列永远预留）；状态标记只用现成界面域色 —— 前台 `▍`+`ACCENT`（不加 `BOLD`）、待答 `?`+`WARN`、活跃 `●`+`PLAIN`、cwd 不存在整行 `MUTED`、流损坏尾 `损坏`+`BAD`；排序 = 前台所在检出组钉组顶 + 组间/组内 mtime 降序 + id 兜底；分组 = 按 cwd、组头 1 行写 `--show-toplevel` 目录名（家族探测失败时按 cwd 字符串各成一组，重名写尾两段路径），不带计数与分支名，空组不画；整页文案只有 `还没有历史会话` 与 `读不出会话列表`（无 `git` 不产生文案）；单击任一行 = 交键盘 + 落焦点 + 能切就切，点前台行只交键盘与落焦点，两种切不了的行各回一句提示行；不滚动、末行 `＋N 个会话`、前台必可见；列表行禁走 `summarize` 全量（155.6–172.2 ms），走票 01 §4 的前缀轻路径（约 10 ms）。**列表范围被本票放宽为 store 里的全部会话**（冻结项 3 已改写），可达性缺口升级为 [grilling：会话列表的搜索与过滤](issues/10-session-list-search.md)。
- [grilling：多会话宿主与后台并发](issues/04-session-host-and-concurrency.md)：**一个会话 = 一个 `Harness` + 一个拥有它的 tokio task**；`Harness` 就是宿主、不造新词，只加**宿主表**（每行 = `id` + cwd + 状态 `watch` + 命令 `mpsc` + `CancelSignal` + `JoinHandle`）；**前台指针 = 全进程一个 `watch<SessionId>`**，切换只是改它、忙闲不阻挡；`RenderEvent` 打 session id（前台全量、后台只送状态转换，切回补齐交票 08），状态三态 `Idle` / `RunningTurn` / `AwaitingHuman` 而 asker 非前台**挂住**；**不设并发上限**但界面给并发读数；`PathLocks` 升成进程级 `cwd → 表`；会话级手势（`/model`、`/effort`、`Shift+Tab`、`/clear`）落前台、进程级（`Ctrl-O`、宽度档、终端标题）全局；**`/clear`、阈值 rollover、页签新建 = 同一动作**（旧的留在宿主表里可切回）；**`Ctrl-C` 只取消前台当前动作、永不退出**，`Ctrl-D` 退出且**有任何回合在跑就拒绝**（提示行给可点入口指在跑集合的第一个会话，手动取消完才放行），退出不写 `SessionEnded`、多个被杀无需新恢复机制。
- [grilling：切入历史会话的会话级事实](issues/05-session-facts-on-attach.md)：切入 = 一条共用的装载函数 `assemble_session`（`-c` 与界面点选同一条路径）；`SessionStarted` 补 model / provider / effort / 权限模式 / 沙箱 mode 五个 `Option` 字段（**不记** context_window 与沙箱可用性），运行中变更落新的 log-only `SessionFactsChanged` **全量快照**（**新范围**，推翻 ADR 0017 的「不进事件流」），**当前事实 = 流上最后一条**；旧流缺字段回退**进程配置默认**（冻结项 9 措辞收窄）并如实说明（提示行一条 + 常驻标记，合并成一句）；每次装载后补一条完整快照；环境类事实（沙箱 / 技能 / AGENTS.md）**重探**，技能菜单与模型上下文不同源只记账不进界面；**只读预览**是第二处新范围，只作降级落点（cwd 不存在 / 坏流半读），凡写流的一律禁（含新建，回执一句），宿主表加只读标志而不动票 04 的三态；损坏流分三档（尾行坏 → 可写、半读 → 只读、读不出 → 拒装）。
- [prototype：左栏「会话」页的形态](issues/06-session-page-shape.md)：`会话` 放页签条**最前**（窄档五签占 26/28 列）；行 = 标记 1 + 标题 27/21 + 间隔 1 + 时间 11/5（**回改票 02**：宽档时间列 8→11、`损坏` 标签移到时间之前）；关闭入口 = **页顶读数行右端 `关闭`**，与并发读数 `已装载 N 个 · M 个在跑` 同一行、作用于焦点行；三级降级阶梯（页区 ≥8 行：组头 + 读数；6–7 行：丢组头保读数；<6 行：读数也让位）；页内键盘 `↑`/`↓` + `Enter` + `r` + `Esc`，关闭只走鼠标；重画按**会话 id** 保焦点、不抢位；退出被拒的提示行入口 = `还有 N 个在跑 · 切过去取消「<标题>」`（后半段 `ACCENT`）。17 张实测帧在 [`prototype/frames/`](prototype/frames/)，读法在 [`prototype/README.md`](prototype/README.md)。
- [grilling：新建会话与卸载边界](issues/07-new-session-and-unload.md)：新建 = `SessionStore::create` + `record_skeleton`（`SessionStarted`），**立即成为前台**，继承前台的模型 / effort / 权限模式、沙箱与技能按同 cwd 重探；**新建 = `/clear` = 阈值 rollover**，`/loop` 闸门（`clear_while_looping`）取消、旧会话连它的 loop 一起留后台；**关闭 = 从宿主表移除**（发 `Close` → await `JoinHandle`），**不写盘不删盘**、前台不许关自己、忙时拒绝、不二次确认、只读预览会话能关；**可以关掉最后一个会话** → **空宿主**态（页顶 `0 个会话` + 只剩 `新建`、页区 `没有会话`、输入区禁用、新建 cwd = 进程启动 cwd）；**调研推翻票面假设**：`SessionEnded` 在 `src/` 里零写入点（0/281 条流以它结尾），「不写收尾」因此零后果 —— 判定读的是 goals 的收尾事件与 pending 工具调用，票面的 `harness_resume` 标识符根本不存在（是 `resume_goal` / `current_goal`）；**两处新范围**：空宿主态，与 `sessions ls` / `sessions stats` 末列改按**末条事件**派生（`停稳` / `中断` / `未收尾`）；**不变量**：已在宿主表里的会话绝不重走 `assemble`（否则进行中的工具调用会被读成崩溃遗留）。
- [grilling：前台视图的会话隔离与切换重建](issues/08-front-view-isolation.md)：**单份投影 + 切回按事件流快照全量重建**（否掉「每会话一份活投影」—— 后台不送事件是票 04 §4 定的，那条路与它冲突且内存 ×N；也否掉 LRU 缓存）；投影收成一个**子结构**、切换 = **整块换**（`TuiState` 98 字段里有 10 个投影字段根本没有清空入口，reset 清单必漏）；`SessionFacts` 与配色整份换；草稿 / 转录滚动与跟随 / 浮层**每会话一份**（另存一张 `SessionId → 轻状态` 表），页签与页内焦点仍是进程级；重建走 `ConsoleRequest` 新变体 `SwitchSession { facts, events }`、**启动那条 `Replay` 被收编**；补齐 = 以快照最大 `seq` 为界 + 重建期间的 `Logged` 进 `live_buffer` + 收尾只 apply `seq > S`（现状 `live_event` 丢 `Logged`，要改）；复用重放视觉但**不插历史分隔行**；在飞的旧重建先收尾。实测量级：281 个会话 / 74,803 事件，单个会话的中位数 **9**、最大 **14,544**。
- [grilling：会话列表的搜索与过滤](issues/10-session-list-search.md)：入口 = **页内键盘 `/`**（页签仍只走鼠标；过滤态下页顶那一行**整行变成过滤行**，读数与 `新建` / `关闭` 让位，不吃额外页区行）；判据 = **标题 + 组头目录名**、空格分词、大小写不敏感、各词 AND，**只在前缀轻路径已派生的列表上筛**（不额外读盘、不搜正文）；结果 = 只剩命中的会话行、组头保留且不因过滤改写、空组不画；命中总数写在过滤行右端（复用 `wording::hit_count`，`0 个命中` 同一句）、标题不高亮（`palette::HIT` 只属时间轴）；末行仍 `＋N 个会话`；空结果两句；**过滤态下钉顶让位**（前台不在结果里就不显示）；键位 `/` 进、可打印字符进查询、`↑`/`↓` 移焦点、`Enter` 接受并离开、`Esc` 先取消或清过滤再还键盘；草稿仍走常驻 `editor` 的收起/还回；归属**进程级**（不进票 08 的每会话轻状态表）。
- [grilling：后台会话有事时的后台通知](issues/11-offscreen-attention.md)：**认**，且放进终端标题的**状态那一段**（段数不变、丢序表不动 → 待答信息最后才被丢）：优先级 `N 个待答`（后台待答）→ `重放中`（前台重放）→ `等你`（前台待答）→ `N 个跑完`（后台未读的刚跑完）→ `运行中`（前台在跑）→ 空；**N 只算后台**（单会话行为一字不变）；「跑完」这条**边沿在消费侧从电平快照推**（宿主表每个的 `watch` 保最新不丢，主循环每帧比对；**不新增通道**，一帧内翻两次会漏）；未读的寿命 = **用户下一次键盘 / 鼠标输入**或**那个会话又开跑**（未读集合住主循环，已读由渲染器经既有 `FrontEndEvent` 队列回报）；**桌面通知另开一张图**（seed 已留指针）；提示音 / 退出码 / `sessions ls` 列待答数 **明确不做**（后者是另一个进程，落盘会成第二处真相）；调研纠了两处事实偏差（标题现状是**三段**、`SessionTitled` 与宿主表在 `src/` 里零命中）。

## 尚未明确

<!-- 通往目的地、但还说不清楚的雾；frontier 推进后可能升级成票，也可能什么都不剩。 -->

（没有。最后一条雾 —— 「两个会话同时推进同一个目标」—— 在 2026-10-12 移进了「明确不做」：它不在通往 destination 的路上。）

## 明确不做

<!-- 超出 destination 的工作；永不 graduate。 -->

- **创建 / 删除 git worktree**：`.scratch/git-worktree/seed.md` 仍是一句意向（会话级还是执行者级、从哪来、生命周期都没定）。本图只**读**已存在的检出，不建也不删（列表范围见冻结项 3）。
- **独立进程 / daemon 的会话主机 + IPC**：用户在 charting 里选了同进程多 Harness；真要分离进程时另开一张图。
- **plain / headless 渲染器的会话页**：本图的页签只在 TUI 左栏。
- **界面里的会话删除（prune）**：删盘不可逆，且它与 CLI `prune` 的关系（保留几个、按什么排）是另一张要单独定的事；本图不做。
- **桌面通知**（`.scratch/desktop-notifications/seed.md` 的整件事：通道、失焦检测、配置默认值、平台退化）：另开一张图；本图只把「进程已经持有的信息」送到终端标题的状态段（[票 11](issues/11-offscreen-attention.md) §5）。
- **跨会话的目标与额度语义**（「两个会话同时推进同一个目标」时进度与额度怎么算、要不要禁止）：原是本图的一条雾，2026-10-12 判定它不在「会话页签决策集」这条路上 —— 另开一张图（目标 / 额度那一族）；本图只定了「新建不继承前台的 `/loop` 目标」（[票 07](issues/07-new-session-and-unload.md) §1）。

## 进度

Charting 完成（2026-10-12）。**11 张子票**（10 张决策票 + 1 张实现期的文档回改票）：票 01–08、10、11（grilling / prototype）与票 03（research）已 resolved —— **决策 frontier 为空、图收口**；只剩 [文档回改票](issues/09-doc-revision.md)（`Type: implement`，由实现批次执行）。票 03 的实测与结论在 [`research/03-aggregation-facts-and-cost.md`](research/03-aggregation-facts-and-cost.md)，一次性 bench 在 `research/bench/`；票 01 的契约在 [grilling：会话身份与标题](issues/01-session-identity-and-title.md)，票 02 的在 [grilling：会话列表的条目、排序与分组](issues/02-list-rows-and-grouping.md)，票 04 的在 [grilling：多会话宿主与后台并发](issues/04-session-host-and-concurrency.md)。

**票 04 是这张图最重的一张**（2026-10-12，六轮 grilling / 17 问）：它把「一个进程里同时活着多个会话、切走的照跑」定到了契约级 —— `Harness` 就是一个会话的进程内实例（**不造「会话宿主」新词**），主循环持一张新增的**宿主表**，前台指针是一个全进程 `watch`，每个一个 tokio task 拥有自己的 `Harness`。它顺带改掉两条既有行为（`Ctrl-C` 不再退出、`render` / `render_task` 从 `OpenedSession` 抽出成进程级一份），并清掉两条雾 —— 「并发上限」直接定为**不设**（只给界面读数）、「后台状态要不要自刷新」的前半答完（状态转换经渲染通道）；同时把「后台通知」升级为 [grilling：后台会话有事时的后台通知](issues/11-offscreen-attention.md)。

**票 02 重开了冻结项 3**：列表范围从「本项目 + 它的 git worktree」改为 store 里的全部会话，连带后果见该票 §0。「跨机器搬运后的会话」由票 02 §2/§4 定死（cwd 不存在的会话按目录名各成一组、整组 `MUTED`、切不过去）—— 票 05 把这最后半句改成「只读打开」。

**票 05 引入两处新范围**（2026-10-12，三轮 grilling / 16 问）：新事件 `SessionFactsChanged`（会话事实的全量快照，log-only）与**只读预览**（cwd 不存在 / 坏流半读时的降级落点，凡写流的一律禁，含新建）。它同时收窄冻结项 9 的措辞（回退来源从「当前前台设置」改成**进程配置默认**），并推翻 ADR 0017 的「模型与档位不进事件流」那半句 —— 交棒清单在票 05 §10，其中 `docs/permissions.md`、`src/permissions.rs:352-354` 与 ADR 那两处要一并回改。

**票 06 是唯一的 prototype 票**（2026-10-12）：它先把左栏第五签的形态渲成 **17 张实测帧**（`.scratch/session-tab/prototype/`，ratatui → `TestBackend` dump，不是手绘），再一轮 8 问拍板。三个副产品：**回改票 02 四处**（宽档时间列 8→11、`损坏` 标签移到时间之前、页区不足 8 行不画组头、页顶多一条并发读数 —— 前两条是探针量出来的，票 02 自己定的写法在原表格里装不下）；**清掉一条雾**（重画按会话 id 保焦点、不抢位）；**落地票 04 交过来的两项**（并发读数落在会话页页顶、退出被拒的可点入口写成 `切过去取消「<标题>」`）。票 06 无新范围：页内键盘只是接上 `文件` / `改动` 两页那一族（`docs/render.md:543-550` 的名单加一页）。

**票 07 用两份只读调研换来一个反直觉的结论**（2026-10-12，两轮 grilling 9 + 8 问加第三轮 3 问）：`SessionEnded` 在 `src/` 里**根本没有写入点**，真实 store 0/281 条流以它结尾 —— 所以「退出 / 关闭不写收尾」零后果，票面担心的「收尾判定出错」不存在（判定读的是 goals 的收尾事件与 pending 工具调用，票面写的 `harness_resume` 标识符也不存在，对应物是 `resume_goal` / `current_goal`）。两端也定死了：新建 = `SessionStore::create` + **立即上前台** + 继承前台的模型 / effort / 权限模式（沙箱与技能按同 cwd 重探）、`/loop` 闸门取消；关闭 = 从宿主表移除（**不写盘不删盘**）、前台不许关自己、忙时拒绝、不二次确认、只读预览会话能关。**两处新范围**：可以关掉最后一个会话 → **空宿主**态（新词，已进 `CONTEXT.md`）；`sessions ls` / `sessions stats` 的末列改按**末条事件**派生三档（`停稳` / `中断` / `未收尾`）。它还清掉一条雾（只读会话的出路 = 关到空宿主态再 `新建`），回改票 06（页顶行右端加 `新建`），并留下一条不变量：已在宿主表里的会话绝不重走 `assemble`。

**票 08 是渲染侧的另一半**（2026-10-12，两轮 grilling 5 + 4 问）：它把「切换前台时屏幕怎么换」定到契约级 —— **单份投影 + 切回按事件流快照全量重建**（后台不送事件是票 04 §4 定的，所以「每会话一份活投影」那条路自己就死了），投影**整块换**而不是写 reset 清单（调研查出 `TuiState` 98 个字段里有 **10 个投影字段根本没有清空入口**，`timeline` 与 `block_at` 最危险），草稿 / 滚动 / 浮层**每会话一份**、页签与页内焦点仍进程级。补齐用 `seq` 划界：快照最大 `seq` 记作 `S`，重建期间的 `Logged` 进 `live_buffer`，收尾只 apply `seq > S`。两处要改的既有行为：`live_event` 不再在重放期间丢 `Logged`；收到新重放请求先收尾旧的（现为直接覆盖，旧批次的缓冲会泄到下一次）。真实量级：281 个会话 / 74,803 事件，单个会话**中位数 9、最大 14,544** —— 常见切换瞬时，长尾那几个实现期要实测延迟。

**票 10 收掉了票 02 留下的可达性缺口**（2026-10-12，两轮 grilling 6 + 3 问）：入口是**页内 `/`**（页签仍只走鼠标），过滤态**整行占用页顶那一行**（不额外吃页区行、票 06 的三级阶梯不动），判据是**标题 + 组头目录名**、空格分词 AND、只在前缀轻路径已派生的那份列表上筛（不读盘、不搜正文），结果是只剩命中行 + 组头保留 + **钉顶让位**，命中总数写在过滤行右端（复用 `wording::hit_count`）、末行仍是 `＋N 个会话`。它回改票 02（文案第四句、§7 的前台可见让位）与票 06（页顶行让位、键位表加 `/`），并给票 09 的文档清单加了一条。**剩下只有票 11（后台会话有事时的后台通知）**。

**票 11 收口了这张图**（2026-10-12，两轮 grilling 5 + 4 问）：后台信号只做一个 —— 让它进**终端标题的状态段**（`N 个待答` / `N 个跑完`），段数与丢序表都不动，所以待答信息最后才被截掉；N 只算后台，单会话行为一字不变。「跑完」这条边沿**在消费侧从电平快照推**（不额外加通道），未读保持到用户下一次输入或那个会话又开跑。桌面通知与跨会话目标语义**各自另开一张图**（前者已在 `.scratch/desktop-notifications/seed.md` 留指针，后者从本图的雾里移出）。调研顺手纠了两处事实偏差：标题现状是**三段**、票 01 的四段与 `SessionTitled` 尚未实现；`SessionState` / 宿主表在 `src/` 里零命中（都是票 04 的未来态）。

**实现票已切出**（2026-10-12，`/to-tickets`）：14 张新票（`12`–`25`）加既有的 `09`，共 15 张 `Type: implement`。切法是 tracer-bullet vertical slices —— 先三张 prefactor（`12` 投影抽子结构、`13` 渲染器与锁表升进程级、`14` 事件带 session id 与重放语义），再 `15` 宿主表（一个会话与今天等价）与 `16` 标题，然后是第一条端到端路径 `17`（列表）→ `18`（装载）→ `19`（重建）→ `20`（点一行切过去），此后 `21` 新建与关闭、`22` 三态与待答、`23` 退出闸门、`24` 过滤、`25` 后台通知。**无阻塞的起手票：12 / 13 / 14 / 16。**

**图走完（2026-10-12）**：11 张子票里 **10 张决策票全部 resolved**、frontier 空 —— 剩下的 [文档回改票](issues/09-doc-revision.md) 是 `Type: implement`，由实现批次执行（`docs/agents/issue-tracker.md` 的判据就是「设计票全部关掉的图就算走完了，哪怕实现票还开着」）。**下一步**：`/to-spec` 把这些决策集折成 `.scratch/session-tab/spec.md`，再由 `/to-tickets` 切实现票。

**落地后的两处收尾（2026-10-12）**：维护者在这张图落地后报了三个症状 —— 点会话页一行切不过去、出口提示里 `ctrl-c` 与 `ctrl-d` 没有拆开、按 `ctrl-c` 得到一句「这个会话现在没有在跑的东西」。前一个是**行为**（主循环里 `OpenSession` 落在一个空分支上，票 20 那条路只接了两头），后两个是**同一族**（提示行还停在票 23 之前的旧语义，那句回话只是那份误导文案的直接后果）。两张票：[`26`](issues/26-click-a-row-reaches-the-host-table.md)（会话级手势就地转请求，拆掉「转发给前台再转回来」那一圈）与 [`27`](issues/27-the-exit-hint-follows-the-session-in-front.md)（出口两档拆开，忙闲改读**眼前这个会话**的三态）。两张各留了欠账：主循环逐行路径仍无端到端覆盖、`forward_gesture` 的 `try_send` 会静默丢手势、问卷里那句退出回执还没定。

**又三张**（2026-10-12 第二轮报告）：`Ctrl-D` 按下去**什么也没出现**（`pending` 立着时被静默吞掉）、要**「再按一次」**的确认、以及会话页看不出**哪些会话已装载**。[`28`](issues/28-ctrl-d-never-swallowed-and-asks-twice.md) 把 `Ctrl-D` 接回双击（`Gesture::Exit` 与 500ms 窗口一直完好）、并让它**关掉立着的那一层**而不是静默；闸门拒绝后那把举手作废，于是「第二下」不会连着退。`[`29`](issues/29-session-titles-say-where-a-session-is.md)` 给会话页的标题分三档色：前台 `LIVE`、已装载 `HOSTED`、只在盘上 `MUTED` —— 前两条修的是**看得见**，第三条修的是**看得懂**。两张票的欠账记在各自文件末尾。

**一张（2026-10-12 第三轮报告）**：维护者复测报「`ctrl-d` 不能退出」与「不能正常切换会话，点击没反应」。[`30`](issues/30-exit-deadlock-and-prompt-liveness.md) 查出这两条是**同一个死锁**：`RenderChannel::shutdown` 要等每一个渲染写端消失，而主循环自己那份 `render` 在退出流程里还活着 —— 进程卡在 `await` 上，从此不再回到 `select`，之后一切手势（按键、点击会话页）静默失效。修复是退出流程第一句 `drop(render)`。同一轮还修掉提示请求每轮重发导致的输入丢失：`--plain` 的 `/quit` 因此**稳定**无效、TUI 偶发（保活那份 future，拿到一行才重新武装）。真机脚本 `scripts/tui-startup-check.py` 也一并校准（锚点 / 出口清单 / 退出码分档 / 就绪判据），它重新成为可红的守卫：修复后 **0/6 red**。

**再一张（同日追问）**：维护者接着问「为什么我切换别的会话时，当前会话没有变？」。[`31`](issues/31-foreground-mark-follows-the-switch.md) 确认会话页那行画的是**启动时**那份 listing 的 `foreground`（listing 只推一次），而切换更新的是渲染侧的 `front` —— 于是 `▍` 与 `LIVE` 色一直停在启动那个会话。修法是在 `SwitchSession` 里就地改行快照的前台归属（不重扫 store）；真机复验 `▍` 依次落在第 11 / 12 / 13 行。**欠账**：前台所在检出组「钉在最顶」的排序仍滞后。

**再记一张（同日追问）**：维护者接着问「关闭的会话该如何操作？」—— 现状**可用但绕路**：点 `关闭` 拒绝的是前台自己，而鼠标点一行又会连前台一起切过去，于是只能「点别的行切走 → `↑`/`↓` 把焦点移回目标行 → 点 `关闭`」，而这两步在屏幕上分不出来。登记为 [`32`](issues/32-close-interaction.md)（`Type: grilling`、`open`），把「关哪个会话在点之前就看得见、且一步完成」留到下一次设计对话定方向，四条既有规则不动。顺手把 26–31 补进 `## 任务清单`（此前只列到 25，`scripts/wayfinder-check.py` 因此一直红着，票 30 也漏了 `Blocked by:`）。

**票 32 收尾（2026-10-12，一轮访谈 19 问）**：维护者问「关闭的会话该如何操作？」。查下来它一半不是优化，而是三处「票里定了、实现没落地」：[票 06](issues/06-session-page-shape.md) 的 `Enter` 切前台是空键、[票 07](issues/07-new-session-and-unload.md) 的「关闭可以关掉最后一个会话」被 `close_check` 的前台拒绝挡住、[票 20](issues/20-click-to-switch.md) 的「单击能切就立刻切」与「关闭 / 改名作用于焦点行」不自洽。这一轮一次定完：点行语义（单击只落焦点，`Enter` 与**鼠标双击**切前台）、只剩一个会话时可直接关到空宿主、被拒提示进提示行并可点、`关闭「<标题>」` 点名，以及量词从「场」换成「会话」（新票 [`.scratch/chinese-ui/issues/09`](../chinese-ui/issues/09-session-noun.md)，先做）。票 32 由 `grilling` 转 `implement`（`ready-for-agent`），票 06 / 20 / 07 各追加一条评论记下推翻与例外。
