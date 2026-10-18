# host → cli → tui 的事实投影只算一处

Type: task
Status: resolved
Resolved-by: wayfinder session 2026-10-19（前置 spec 见下；见 § 作答）
Part of: ../map.md
Blocked by: —

## 背景

[report-2026-10-18.md](../report-2026-10-18.md) **Worth exploring**（10-18 重写）。「宿主表现在长什么样」算成三处 ——

- `cli/interactive.rs:572` 的 80ms 轮询算上一帧快照与边沿判定；
- `ConsoleHandle`（`src/render/input.rs:311`）12+ 个 `pub fn` 推命令；
- `ConsoleRequest` 22 变体；`TuiState` 散落 `HashMap` 或字段承接。

10-12 的 seed.md 那条「宿主表的投影只算一处」升级而成。渲染层要认的领域事实从 6 种涨到 10+ 种，每种一条 push 命令。三条规则（未读的寿命、`N` 只算后台、单会话标题不变）只能靠真机走查（[tui-manual-checklist.md](../../docs/tui-manual-checklist.md)）。

## 想要的形态

seam 放在「host 视图」。一个 `HostFacts` module 算「宿主表现在长什么样」，`cli` 与 `tui` 都订阅它的 diff。12+ 个 `ConsoleRequest` 收成两条「现在」、「事件」。80ms 轮询降到一处。

## 验收

1. `HostFacts` module 单元测试钉死三条规则（未读寿命、`N` 只算后台、单会话标题不变）。
2. `ConsoleRequest` 变体从 22 个收成 ≤ 5 个。
3. `cli/interactive.rs` 的 80ms 轮询合并到一处（不再散在两个文件）。

## 关联

- 来源：[report-2026-10-18.md §host → cli → tui 的事实投影只算一处](../report-2026-10-18.md)。
- HTML：[report-2026-10-18.html 第 4 张卡片](../report-2026-10-18.html)。
- 真机清单：[docs/tui-manual-checklist.md](../../docs/tui-manual-checklist.md) ㊶。

## 注意

与「单会话」默认行为耦合：把订阅改写前，先在 spec 钉死「宿主表空、单会话、多会话」三态的读数。

## 前置 spec（2026-10-19，动手前钉死）

`## 注意` 要求先钉死「宿主表空 / 单会话 / 多会话」三态的读数。判据与三条规则（未读寿命、
`N` 只算后台、单会话标题不变）都住在 `src/session/host_facts.rs`：

- **宿主表空**（`sessions.is_empty()`）：`已装载 0 个`、页区 `没有会话`、输入区禁用（`muted`），
  没有前台条目。前端**不**保留上一帧的会话切片 —— 不在装载列表里的会话，它的投影 / 三态 /
  名册 / 只读一起收掉（原 `SessionClosed` 的账，现在由快照的 diff 推出）。
- **单会话**（`sessions.len() == 1`）：那个会话就是前台。`waiting` 与 `finished` 恒为 0
  （两个计数都只数后台）；标题状态段因此**一字不差**（与今天单会话行为相同）。
  `已装载 1 个`；关闭规则是「唯一会话可直接关到空宿主」（票 32）。
- **多会话**（`sessions.len() >= 2`）：`hosted` 的顺序 = 宿主表的装载顺序（页签条与平铺槽位
  都按它排）；`waiting` 数**非前台**的 `AwaitingHuman`，`finished` 数**非前台**的未读；
  前台自己的三态走状态行 / 出口提示，不进这两个数。

三个读数的来源只有一个：`HostSample::from_host`（前台 + 每会话 `(id, 三态, 只读)`），
80ms 采一次；`HostTracker` 负责未读边沿；前端只消费 `HostFacts` 快照。

## 作答（2026-10-19）

### 验收 1：`HostFacts` 单测钉三条规则

新文件 `src/session/host_facts.rs`：

- `HostSample`：一次采样的原料（前台 + 每会话 `HostedSession { id, state, read_only }`）。
  `from_host` 泛型化到命令类型，测试直接造样本。
- `HostFacts`：前端读的唯一宿主事实（`foreground` / `sessions` / `waiting` / `finished`）。
- `HostTracker`：上一帧三态 + 未读集合的记账；`update(&HostSample) -> HostFacts` 一处推边沿，
  `attention_seen()` 是「用户下一次输入」。

四条单测：`unread_lives_until_the_next_input`（规则 1）、`only_background_sessions_count` /
`a_single_session_never_moves_the_title`（规则 2、3）、`a_closed_session_takes_its_unread_with_it`。

### 验收 2：`ConsoleRequest` 变体 20 → 4

```rust
pub enum ConsoleRequest {
    Prompt { reply },
    Ask(ConsoleAsk),        // Permission / Questionnaire / Picker
    Facts(HostFacts),       // 「现在」
    Notify(ConsoleNotify),  // 「事件」：SessionUpdate / SwitchSession / Sessions /
                            // Catalog / Replay / Attention / CloseBlocked /
                            // ExitBlocked / RunState / Muted
}
```

原来按宿主事实各守一条 push 命令的 15 个变体（`SessionState` / `ReadOnly` / `HostCount` /
`Hosted` / `TitleAttention` / `SessionClosed` / `Muted` / `RunState` / `SessionUpdate` /
`Sessions` / `Catalog` / `Replay` / `Attention` / `CloseBlocked` / `ExitBlocked`）收成
`Facts` + `Notify`；`Ask` / `Questionnaire` / `Picker` 收进 `ConsoleAsk`。`ConsoleHandle`
上对应的 6 个推送方法（`session_state` / `read_only` / `host_count` / `hosted` /
`title_attention` / `session_closed`）删掉，新增 `facts()` 与 `notify()`。

### 验收 3：80ms 轮询收成一处

`cli/interactive.rs` 的轮询块现在是：

```rust
let sample = HostSample::from_host(&host);
let facts = host_tracker.update(&sample);
console.facts(facts);
```

三态边沿、未读身份、后台两个计数、装载列表全部由 `HostTracker` 一处推；`loading.rs` 与
`close_session` 里那几处直接推送一并删掉（它们由下一次快照带出）。`AttentionSeen` 调
`host_tracker.attention_seen()`。`herdr` 的上报仍只在 80ms 这一处读前台状态。

前端侧 `TuiState::apply_host_facts` 是唯一应用点：读数、每会话三态 / 只读、装载列表与
「谁不在表里了」（原 `SessionClosed` 的账）、空宿主态都从这一份快照更新。

### 验证

`cargo test` 全量通过；`cargo clippy --all-targets` 0 warning；`cargo fmt --check` 干净。
`docs/tui-manual-checklist.md` ㊶ 第 4 条补注：三条规则已由单元测试钉住，真机只看观感。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「事实投影只算一处（10-18 重写）」分支