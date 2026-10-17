# 26: `[ui]` 两个新键

Type: implement
Status: done
Part of: ../map.md
Blocked by: 19

**要建的：** `[ui]` 段多两个可配项，写错了拒绝启动。

- **`tile_limit`**：整数 `2..=4`，默认 `4`；集合上限（票 19 的满员回执从此读它）。
- **`sidebar`**：`auto` / `always` / `never`，默认 `auto`；给左栏启动初值（`auto` 的检测在票 27）。
- 非法值（越界、非整数、未知字面量）**一律拒绝启动**，错误消息照 `number_style` 那套写法。

## 验收

- [x] `tile_limit = 2` 时集合上限 2（第 3 个被拒并提示「最多 2 个」）—— 回执与上限在票 19 /
  25 的接线里读 `tile_limit`；`the_tile_limit_caps_the_set_and_puts_the_limit_in_the_receipt` 钉住。
- [x] `tile_limit = 1` / `5` / 非整数 → 启动失败并指名该项。
- [x] `sidebar = "never"` 启动时左栏不画；`"always"` 在 herdr 里也画。
- [x] `sidebar = "sometimes"` → 启动失败并列出合法字面量。
- [x] config 单测覆盖上述各档，错误消息逐字对齐 `number_style` 的测试写法。

## 依赖

[19 集合生灭与满员拒绝](19-tiles-lifecycle-and-limit.md)（回执要读 `tile_limit`）。决定见
[`issues/11-grilling-limits-config-and-herdr.md`](11-grilling-limits-config-and-herdr.md)。

## 落地注记（2026-10-17）

- `SidebarMode { Auto, Always, Never }` 与 `tile_limit`（`DEFAULT_TILE_LIMIT = 4`、
  `MIN_TILE_LIMIT = 2`、`MAX_TILE_LIMIT = 4`）住在 `config.rs`；`UiSettings` 与 `Config`
  都多了这两个字段，`RawUi` 用 `deny_unknown_fields` 照旧挡不认识的键。
- 错误：`TileLimitOutOfRange { value, min, max }` 与 `UnknownSidebarMode { value }`，
  文案逐字对齐 `number_style` / `file_viewer` 那两条。
- **集合上限那条留给票 19**：本票只负责「配置值读出来、写错拒绝启动」。
- 测试：`tests/config_profiles.rs` 四条（缺省、合法档、越界、非法字面量）。
- **2026-10-17 维护者修订**：`tile_limit` 的语义收窄成「平铺显示宿主表的前 N 个」（不再有
  「满员被拒」的回执）；`sidebar` 不变，平铺态里 `Ctrl-O` 现在真的能叫回左栏。
