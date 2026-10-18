# 同一会话状态，收到一处

Type: task
Status: ready-for-agent
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

## 评论

- 2026-10-18：从 [.scratch/architecture-deepening/seed.md](../seed.md) 「同一会话状态收到一处（Worth exploring → Strong，已升级）」分支