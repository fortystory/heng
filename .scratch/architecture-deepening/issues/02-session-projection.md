# 同一会话状态，收到一处

Type: task
Status: resolved
Resolved-by: wayfinder session 2026-10-19（见 § 作答）
Part of: ../map.md
Blocked by: —

## 背景

[report-2026-10-18.md](../report-2026-10-18.md) **Strong** 候选。session-panes 票 23 / 25 落地后，`TuiState` 上冒出 5 份按 `SessionId` 平行的 `HashMap`（`views` / `states` / `light` / `catalogs` / `read_only_by`）。每加一会话属性要在 `View`（57 字段）、`SessionLight`（18 字段）、`stash_light` / `load_light`（`tui.rs:4393 / 4421`）、还有一到两份 `HashMap` 上同步改。代码量随会话数线性涨，但语义是同一个：「按会话切片」。

## 想要的形态

seam 放在「一个会话的投影」。`SessionProjection` module 把 `View` 与 `SessionLight` 收在一起；`TuiState` 仅持 `HashMap<SessionId, SessionProjection>`；`stash` / `load` 收成一份迁移函数。

## 验收

1. `TuiState` 上的 `HashMap<SessionId, T>` 数量从 5 收成 1。
2. 新加一会话字段只动 `SessionProjection` 一个 module；`View` / `SessionLight` 不再分别维护。
3. `stash` / `load` 收成一个函数，单元测试钉死「同一会话切走再切回后状态一致」。

## 关联

- 来源：[report-2026-10-18.md §同一会话状态，收到一处](../report-2026-10-18.md)。
- HTML：[report-2026-10-18.html 第 2 张卡片](../report-2026-10-18.html)。
- 起源：10-12 的 seed.md 「`View` 是投影的 seam」（Worth exploring）—— 本轮从形态问题升为 Strong。

## 作答（2026-10-19）

`src/render/tui.rs` 里新增私有 `mod projection`：`View`（57 字段）与 `SessionLight`（18 字段）
的定义搬了进去，字段改成 `pub(super)`；再新增 `SessionProjection` —— 一个会话在渲染器里的
**切片状态**：

```rust
pub(super) struct SessionProjection {
    view: Option<View>,          // 切走时收进来；焦点会话的那份在 TuiState::view
    light: Option<SessionLight>, // 同上
    state: Option<SessionState>,
    catalog: Vec<CatalogEntry>,
    read_only: bool,
}
```

`TuiState` 的 5 张 `HashMap<SessionId, T>` 收成 1 张 `projections: HashMap<SessionId,
SessionProjection>`；`view` / `light` 是 `Option`，所以同一张表既装完整投影（切走的会话），
也装只报过三态 / 名册 / 只读的后台会话。焦点会话的投影与轻状态仍以散字段住在 `TuiState` 上
（热路径：`self.view.xxx` 与 `self.editor` 之类有数百处访问，整份搬进表会把每个访问点都变成
借用协商）；「焦点那份不在表里」的不变量由唯一的迁移函数维持。

`stash_light` / `load_light` 收成 **一个** `switch_session(facts, cwd) -> bool`：收走旧会话
（投影 + 轻状态写回表、三态 / 名册 / 只读原样留着）、取出目标那份装进焦点字段；返回
`true` = 第一次装载（调用方据此决定铺不铺历史）。它和 `SessionProjection` 定义一起住在
`mod projection` 里，于是「会话字段 + 迁移」只有这一个 module。

其余引用点（会话页行、平铺非焦点格、背景读数、`SessionState` / `ReadOnly` / `Catalog` /
`SessionClosed` 事件、`busy()`）都改成读同一张表。

§ 验收对照：

1. `HashMap<SessionId, T>` 从 5 张收成 1 张：`grep -n "HashMap<crate::events::SessionId"`
   只剩 `projections` 一处（`View` 内部的 `block_at` 是 `HashMap<BlockId, BlockStamp>`，不是
   会话切片表）。✅
2. 新增会话字段只动 `mod projection`：投影字段动 `View`、轻状态字段动 `SessionLight`、
   三态 / 名册 / 只读动 `SessionProjection`，三者与迁移函数同在一个 module。✅
3. `stash` / `load` 收成 `switch_session` 一个函数；新增单测
   `a_session_projection_survives_a_round_trip` 钉住「同一会话切走再切回后三态 / 草稿 /
   投影原样回来、投影不重建」，`tests/session_switch.rs` 的
   `the_way_out_follows_the_session_that_is_in_front` 追加了「再切回忙会话三态仍在」。✅

验证：`cargo test` 全部 target 通过（0 failed）；`cargo clippy --all-targets` 0 warning；
`cargo fmt --check` 干净。

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「同一会话状态收到一处（Worth exploring → Strong，已升级）」分支