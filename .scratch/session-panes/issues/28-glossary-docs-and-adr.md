# 28: 词条、文档与 ADR

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27

**要建的：** 外壳改造之后，仓库里的说法与记录跟上。

- **`CONTEXT.md` 新立五个**：右栏（`RightRail`）、顶部页签条（`TabStrip`）、平铺（tiles）、
  格（`Tile`）、焦点格（`FocusedTile`）；**改四个**：轨迹视图（住右栏、不再常驻主列）、
  对话视图（主列唯一一页）、左栏（删「轨迹在主列页签条上」两处）、前台（平铺态下 = 焦点格）。
  「格」的 `_Avoid_` 写清：pane / 窗格（那是转录滚动缓冲）、分屏、面板。
- **`docs/render.md`**：改 `## 外壳`（6 行预算与分区）、`## 两个视图`（对话在主列、轨迹在右栏，
  删「永远可达」）、`## 时间轴`、`## 键盘`（`Ctrl-V` 与两套提示行）、`## 折叠与过滤` /
  `## 拖选与复制` / `## 可点链接`（「轨迹页」改「右栏」），并**新增「右栏」一节**。
- **`docs/tui-manual-checklist.md`**：加一条真机项（顶部页签条 / `Ctrl-R` / `Ctrl-1…4` /
  右栏层序 / herdr 默认关）。
- **ADR**：写两条正文 —— 0023「平铺的进程模型」（同进程多会话，不做多进程、不交给 herdr / tmux
  的 pane）、0024「轨迹的落点（主列 → 右栏，第三版）」（为什么又搬、为什么不两边都放）。
  **herdr 的检测与默认值不立 ADR。**
- **`docs/lifecycle.md` 不动**（平铺不新增进程或事件）。

## 验收

- [ ] `CONTEXT.md` 九处改动落地（五新四改），每条带 `_Avoid_`。
- [ ] `docs/render.md` 不再有「轨迹在主列」的旧说法，且有「右栏」一节。
- [ ] `docs/tui-manual-checklist.md` 的新项可以照着一步步走。
- [ ] ADR 0023 / 0024 各有状态、背景、决定与替代方案，且与 `docs/adr/` 既有格式一致。
- [ ] `python3 scripts/check-doc-size.py` 不因这次改动新增违规。

## 依赖

[13](13-right-rail-and-trace.md)、[14](14-right-rail-keyboard.md)、[15](15-top-tab-strip.md)、
[16](16-remove-main-tabs-and-hints.md)、[17](17-tiles-two-modes-and-geometry.md)、
[18](18-tiles-set-and-focus.md)、[19](19-tiles-lifecycle-and-limit.md)、
[20](20-question-and-ask-per-session.md)、[21](21-draft-and-status-readout.md)、
[22](22-detail-overlay-and-drag.md)、[23](23-view-per-session.md)、
[24](24-width-replay-fidelity.md)、[25](25-session-facts-per-session.md)、
[26](26-ui-config-keys.md)、[27](27-herdr-detection-and-sidebar.md) 全部落地。

决定见 [`issues/12-grilling-main-column-after-move.md`](12-grilling-main-column-after-move.md) 第 4–6 条。
