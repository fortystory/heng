# 27: herdr 检测与左栏初值

Type: implement
Status: ready-for-agent
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

- [ ] `HERDR_ENV=1` + `sidebar = auto`：启动后左栏不画；`Ctrl-O` 能叫回。
- [ ] 叫回之后退出再启动：左栏**又回到收起**（pty）。
- [ ] `HERDR_ENV` 未设：左栏正常画。
- [ ] `HERDR_ENV=1` + `sidebar = "always"`：左栏画。
- [ ] `HERDR_ENV=0` 或缺失时不产生任何副作用；plain / headless 行为不变。
- [ ] pty 回归覆盖上述四组环境 × 配置。

## 依赖

无（可以立刻开始；`[ui] sidebar` 键本身由票 26 提供，本票可以先按缺省 `auto` 实现，票 26 落地后接上）。
决定见 [`issues/11-grilling-limits-config-and-herdr.md`](11-grilling-limits-config-and-herdr.md)
与 [`issues/03-research-herdr.md`](03-research-herdr.md)。
