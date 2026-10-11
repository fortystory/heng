# grilling：会话身份与标题

Type: grilling
Status: resolved
Blocked by: —
Part of: ../map.md

## 问题

把「一场会话在界面里叫什么」定到可实现级：标题从哪来、怎么存、怎么改、怎么截断。

**范围级决定已在 charting 冻死**（见 `../map.md` 的冻结项 5）：标题**自动派生**（首条用户消息首行，**不落流**），**手动改名落一条新的 log-only 事件**（暂名 `SessionTitled`），`SCHEMA_VERSION` 2 → 3。本票只定「怎么落」。

## 需要定

1. **事件形状**：`SessionTitled { title }` 够不够？要不要带 `source`（人改的 / 系统生成的）、`at`（信封已有时刻）、长度上限；它是 **log-only**（不进 `messages`、不碰投影前缀，照 ADR 0019 的先例）；`SCHEMA_VERSION` 涨到几，与票 05 给 `SessionStarted` 补字段是不是**同一次**涨版（若是，两票要对接同一处改动，见票 05）。
2. **标题的权威链**：流上**最后一条** `SessionTitled` → 否则派生（该流**首条** `MessageCompleted { role: User }` 的 `text` 首行）→ 否则 `<id>` 里的时间戳。每一级的降级文案是什么。
3. **清空 / 回退**：人把标题改空（或 `--title ""`）算「删掉手动标题、回落派生」，还是算「标题就是空」；要不要一条显式的"撤销改名"。给一个不引入第二种事件类型的写法。
4. **改名入口**：左栏页内怎么改名（页内键盘焦点 + 一个键？双击行？弹一个输入框？）；要不要同时给 CLI（`heng sessions title <id> <text>`）——若给，它是不是另一个 effort（对照 `continue-by-id` 的先例：CLI 入口可以很小）。**左栏"页签从不给键位"的不变量不许破**：改名要么是页内焦点区里的动作（照 `文件`/`改动` 页的先例），要么是别处的入口。
5. **派生标题的成本**：派生要对每条流找首条用户消息 —— 与票 03 的"每条流全量读"是同一笔开销，还是能只读前缀（`SessionStarted` 在第一行、首条用户消息通常在很前面）。给出一处共用的读取路径（与票 03 对接），别让列表与标题各读一遍盘。
6. **宽度与截断**：标题在 28 / 40 列档各占几列（要按 `text_columns` 算宽字符）、超长怎么截（尾 `…`？）、去掉换行与控制字符（首条用户消息可能是多行 markdown）。列表行里标题与时间/工作区同处一行的排版由票 02 定，本票只给"标题字符串"的规范。
7. **旧会话**：没有 `SessionTitled` 的流只读派生；允许改名（写一条新事件即成立）。派生标题**不落流**的代价写清（每次进程重启都要重算一次，可缓存多久）。
8. **模型可见性**：标题**绝不**进 `messages`、不进 system prompt；确认它对前缀缓存零影响（log-only 事件的既有纪律）。

## 先读

- `../map.md`（冻结项 5、10，与票 03 / 05 的边界）
- `03-aggregation-facts-and-cost.md` 的作答（**派生源与成本的实测**：52.3% 的流没有 `role:User` 消息，`SessionStarted` 恒为首行、只读前缀便宜 42× —— 本票的权威链要按它来定）
- `src/events.rs` 的 `SCHEMA_VERSION`、`EventPayload`、信封字段（`seq` / `at` / `speaker_id`）
- `src/agent.rs` 的 `record_user_message`（首条人话落成什么）
- `docs/adr/0019-command-records-are-log-only.md`（log-only 的先例与理由）
- `CONTEXT.md` 的**会话** / **会话目录** / **会话桶**三条
- `docs/agents/issue-tracker.md`（答案写进票、`Status` 怎么改）

## 答案落点

契约级：事件形状与版本 + 标题解析规则（含回退链与清空语义）+ 改名入口 + 派生成本与缓存 + 截断规范。不要写实现代码。

## 作答

三轮 grilling（2026-10-12，12 问 + 2 条确认）定下的契约。**只定形状与规则，不含实现代码**；证据一律给 `文件:行`。

### 1. 事件形状与版本

- **`SessionTitled { title: String }`** —— 唯一的写者是「人手动改名」（自动派生**不落流**，冻结项 5）。不加 `source`、不加时刻字段：信封已有 `seq` / `at` / `speaker_id`（`src/events.rs:826`），照 `CommandRun` 只有 `text` 的先例（`src/events.rs:367`）。
- **log-only**：在 `src/provider/projection.rs:64` 那处穷尽 match 里 `=> {}`，零产出。
- **`redact()` 必须打码 `title`**：标题是人敲的散文（可能就是带密钥的第一行），照 `CommandRun` 的处理（`src/events.rs:561`）。
- **`SCHEMA_VERSION` 2 → 3，与票 05 同一次**（票 05 给 `SessionStarted` 补 model / effort / 权限模式 / 沙箱档，用 `Option` + `#[serde(default)]`）。两票给同一个值、共用一份改动清单。涨版本身不改任何读写行为 —— `SCHEMA_VERSION` 只写不读（`src/events.rs:20-23`），兼容性由 serde 形状给。
- **知情代价**：旧二进制读到流**中间**位置的 `SessionTitled` 会 `unknown variant` → `InvalidData`，整条 `--continue` 失败（`src/events.rs:1096-1104`）；落在末行则被当坏尾巴丢弃，并可能让 `seq` 与行号错位。这与「跨版本不承诺兼容」的既有纪律一致，**认**。
- **实现触点（加一个变体）**：7 处穷尽 match 会编译失败 —— `events.rs:511` `kind()`、`events.rs:550` `redact()`、`provider/projection.rs:64` `project()`、`session/observe.rs:511` `entry_of()`、`session/observe.rs:998` `stats()`、`render/transcript.rs:354` `push_logged()`、`render/headless.rs:104` `consume()`。文档随行号漂移（`docs/lifecycle.md` + `scripts/lifecycle-check.py`）、`docs/render.md`（若上屏）、`docs/observability.md`（若进复盘）在实现期同步。

### 2. 标题的权威链

1. 流上**最后一条** `SessionTitled`，且 `title` 归一化后非空；
2. 否则**该流前 8 行内**首条 `MessageCompleted { role: User }` 的 `text` 首行（`ContextInjected` 不算）；
3. 否则兜底：从 `SessionId` 的 UTC 段（`<YYYYMMDDTHHMMSSZ>-<8 hex>`，`src/session/store.rs:229`）渲染**本地** `MM-DD HH:mm`；解析不出即取 id 前 8 字符。

第 1 级的「空」是撤回，见 §3。第 2 级若命中的首条用户消息归一化后为空（纯空白、只有换行、只有 ` ``` `），**在同一 8 行窗口内继续找下一条** `role:User`；窗口内全空才落第 3 级。口径写明：取的是「前 8 行内首条」，不是「全流首条」—— 实测 134 条有用户消息的流里 130 条落在第 4–7 行（p50 = 6、p90 = 7），另 4 条在第 685 / 2549 / 2737 / 13195 行，它们走兜底。

### 3. 清空 / 撤销

- 人把标题改空（TUI 里提交空串、CLI `--title ""`）**算撤回**：这一条之后不再有手动标题，回落第 2 级派生。
- 语义由「空 `title` 就是没有手动标题」承载：**不加 `Option`、不加第二种事件类型、不加 `--untitle`**。
- **清空时仍必须写下这条事件** —— 什么都不写的话，最后一条手动标题仍然生效。
- `speaker_id` 用 `SpeakerId::User`（改名是人敲的一行）。

### 4. 派生读取路径与缓存

- 列表与标题**共用一条前缀读路径**（放 `src/session/` 下，约 30 行）：`BufReader::lines()` + `serde_json::Value`，固定读**前 8 行**，只取 `payload.SessionStarted.cwd` 与首条 `payload.MessageCompleted` 且 `role == "User"` 的 `text`。不要 `Event` 全量反序列化。
- **绝不走 `EventLog::open`**：它会 `repair_before_append` 补换行 / `set_len` 截断坏尾巴，**改文件**（`src/events.rs:1018-1021`、`:1114-1134`）；列表与派生都是只读动作。
- **成本**：281 条 3.7–4.8 ms，比全量解析（155.6–168.8 ms）便宜约 42×。未命中第 2 级**不扫全文件**（扫到文件尾是 54.5–58.5 ms），直接落第 3 级。
- **缓存**：只在**进程内**（`HashMap<SessionId, String>`），懒算；列表重取时沿用，不做失效判定（流文件没变则派生结果没变）。**不落盘** —— 落盘就是给标题造第二处真相。

### 5. 标题字符串规范

- **归一化**（写盘前）：取 `text` 首行 → trim → 去控制字符 → 内部连续空白压成一个空格。**不解析 markdown**，也不剥行首的 `#` / `-` / `>`（会显示成 `# 帮我改 x`，这是已知的取舍）。
- **上限 200 字符**（按字符数，不按列）。存储层不按列预截断 —— 列预算是渲染层的事，宽度档变了不该改数据。
- **渲染层按列截断**：用 `text_columns`（`src/render/width.rs:13`）量宽，超宽用现成 `ellipsize_line`（`src/render/width.rs:68`，`…` 占 1 列），照左栏既有写法（`src/render/tui.rs:8158`、`:8277`）。28 / 40 两档各给标题多少列**由票 02 定**；本票只钉「截断在渲染层、按列、尾 `…`」。

### 6. 改名入口

- **TUI（机制由本票钉，键位留票 06 微调）**：`会话` 页拿页内键盘焦点（照 `文件` / `改动`：`src/render/tui.rs:4462` 那个 `matches!` 加一个变体），单键进入改名模式，**复用常驻 `editor: Input`**（照轨迹页搜索模式 `src/render/tui.rs:4844`「一个绘制件都不新造」：暂存草稿、`Enter` 提交、`Esc` 取消）。默认动作 `Enter` = 切过去，改名给 `r`。
- **不做双击**：命中层明确没有双击识别（`src/render/tui.rs:1600`），也不新增手势。
- **不破页签不变量**：「页签只走鼠标」管的是页签条（`src/render/tui.rs:8703`），页内焦点是既有先例。
- **CLI**：`heng sessions title <id> <text>`（空串 = 撤回）。解析在 `src/cli.rs`（动词表 `:3337`、`parse_sessions` `:3281`），帮助文本在 `src/render/wording.rs:3640`、`:3739` 两处同步。照 `-c <id>` 的先例，入口很小。

### 7. 可见面

- **左栏 `会话` 页**：必显示（形态与行排版归票 02 / 票 06）。
- **终端标题认它**（`.scratch/terminal-title/` 的三段规则改为四段）：`<路径> · <标题> · <状态> · <目标名>`，标题紧跟路径。丢段顺序（超 `TITLE_COLUMNS` = 40 列时从右往左丢，`src/render/wording.rs:2122`）：**目标名 → 标题 → 状态词 → 退成基名路径 → 硬截断**。状态词排在标题之后丢，保住「这个终端在等你」这条即时信号（`title_state` 的既有优先级：重放 > 等你 > 运行中，`src/render/wording.rs:2063`）。标题段**不预截断**，沿用「整段试着放，放不下丢整段」的既有规则。`join_title` 从三段扩到四段（`:2154`）。
- **兜底标题（`MM-DD HH:mm`）不进终端标题**：那里每一段都该是人选中的信息；左栏照显示（行与行要能区分）。
- **`sessions ls` / `show` 认它**：`Listing` 加 `title`（`src/session/observe.rs:45`），`Entry` 加 `SessionTitled` 一行（照 `Entry::Command` 的先例，`src/session/observe.rs:254`）—— 那两处穷尽 match 本来就必须表态。
- **headless / plain / probe**：`SessionTitled` 在 `render/headless.rs:104` 显式静默忽略；会话页只在 TUI（冻结项 12）。
- **桌面通知**：`src/` 里没有实现（`notify` / `dbus` 零命中，`.scratch/desktop-notifications/` 只有 `seed.md`），本票不涉及。

### 8. 旧流与模型可见性

- 没有 `SessionTitled` 的旧流**只读派生**；允许改名（写一条新事件即成立，不需要迁移）。
- **派生标题不落流**的代价见 §4：每次进程启动重算一次，进程内缓存覆盖同一次运行。
- `SessionTitled` **绝不进 `messages`、不进 system prompt**：投影里零产出（§1），对前缀缓存零影响。
- 四处手写 `schema_version: 1` 的 fixture 现在照读；**不要**顺手加「版本 ≥ 3 才可读」的校验。

### 9. 已知限制（写进契约，不引锁）

`EventLog::append` 只有进程内 Mutex（`src/events.rs:1052`），`next_seq` 是打开时按 `events.len() + 1` 算的（`:1022`）。所以**别在一场会话正跑着时用 CLI 改它的名**：两个进程各持自己的 `next_seq`，会写出重复 `seq`，破了「`seq` = 行号 = 唯一身份」。TUI 内改名走宿主 `emit`（进程内串行，即冻结项 6 的同进程多 Harness），不在此列。本图**不为改名引入文件锁**。

### 10. 与票 05 的对接点

- 两票**共用** `SCHEMA_VERSION = 3` 与同一份改动清单：票 01 加变体 `SessionTitled`（旧二进制**不能**安全读中间位置的新变体），票 05 加 `SessionStarted` 字段（旧二进制读新字段安全：serde 默认忽略未知字段，`tests/event_log.rs:358` 是先例）。
- 票 05 定的 `SessionStarted` 新字段与标题无关，不得改动标题的权威链（§2）。
