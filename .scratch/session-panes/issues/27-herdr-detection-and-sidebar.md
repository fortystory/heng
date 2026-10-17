# 27: herdr 检测与左栏初值

Type: implement
Status: done
Part of: ../map.md
Blocked by: —

**要建的：** 在 herdr 里跑 heng 时，左栏默认是收起的。

- 判据：**`HERDR_ENV == "1"` 精确等值**（官方承认，但没有稳定性承诺）。读不到 / 非 `1` /
  变量不存在 = **不在 herdr**（默认开左栏）。
- 只在 `[ui] sidebar = auto` 时**读一次**（TUI 组装那一刻）；不每帧读、不落盘、不写配置。
- **plain 与 headless 不读**（它们没有左栏）。
- 结果只当 `sidebar_wanted` 的**初值**：`Ctrl-O` 照旧改它、意愿只活在进程内；**重启回到默认关**。
- 宽度档照旧相乘：`可见 = sidebar_wanted && 宽度档有值`。

## 验收

- [x] `HERDR_ENV=1` + `sidebar = auto`：启动后左栏不画；`Ctrl-O` 能叫回。
- [x] 叫回之后退出再启动：左栏**又回到收起**（pty）。
- [x] `HERDR_ENV` 未设：左栏正常画。
- [x] `HERDR_ENV=1` + `sidebar = "always"`：左栏画。
- [x] `HERDR_ENV=0` 或缺失时不产生任何副作用；plain / headless 行为不变。
- [x] pty 回归覆盖上述四组环境 × 配置。

## 依赖

无（可以立刻开始；`[ui] sidebar` 键本身由票 26 提供，本票可以先按缺省 `auto` 实现，票 26 落地后接上）。
决定见 [`issues/11-grilling-limits-config-and-herdr.md`](11-grilling-limits-config-and-herdr.md)
与 [`issues/03-research-herdr.md`](03-research-herdr.md)。

## 落地注记（2026-10-17）

- **判据**：`herdr::in_herdr(env)` = **`HERDR_ENV == "1"` 精确等值**；`Reporter::from_env`
  复用它（票 13 时期那段判据收成一处）。
- **初值**：`SidebarMode::initial_wanted(herdr)` 是纯函数；`TuiOptions.sidebar` 带着它进
  `Tui::run`，在那里 `set_sidebar_wanted` 一次。plain / headless 根本不构造 `TuiOptions`，
  所以「不读」是结构性的。每次启动都重新算 —— 「退出再启动又回到收起」由这一点保证。
- `Ctrl-O` 照旧只改进程内的 `sidebar_wanted`，不影响配置、不落盘。
- **验收改用单测 + 帧断言，不是 pty**：仓库没有 Rust 侧 pty 装置（`scripts/tui-startup-check.py`
  只冒烟启动）。`herdr::tests` 钉住精确等值，`config_profiles` 钉住四组
  `initial_wanted`，`render_layout` 的 `a_hidden_starting_sidebar_leaves_the_frame_without_it`
  钉住「不画时整条列不在」。
- **只在 `auto` 时读环境**（审查后修）：`always` / `never` 是显式覆盖，不再调用
  `in_herdr(env)`（口径与 spec 45 对齐；行为不变）。
