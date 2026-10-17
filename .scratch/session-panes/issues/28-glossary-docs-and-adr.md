# 28: 词条、文档与 ADR

Type: implement
Status: done
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

- [x] `CONTEXT.md` 九处改动落地（五新四改），每条带 `_Avoid_`。
- [x] `docs/render.md` 不再有「轨迹在主列」的旧说法，且有「右栏」一节。
- [x] `docs/tui-manual-checklist.md` 的新项可以照着一步步走。
- [x] ADR 0023 / 0024 各有状态、背景、决定与替代方案，且与 `docs/adr/` 既有格式一致。
- [x] `python3 scripts/check-doc-size.py` 不因这次改动新增违规。

## 依赖

[13](13-right-rail-and-trace.md)、[14](14-right-rail-keyboard.md)、[15](15-top-tab-strip.md)、
[16](16-remove-main-tabs-and-hints.md)、[17](17-tiles-two-modes-and-geometry.md)、
[18](18-tiles-set-and-focus.md)、[19](19-tiles-lifecycle-and-limit.md)、
[20](20-question-and-ask-per-session.md)、[21](21-draft-and-status-readout.md)、
[22](22-detail-overlay-and-drag.md)、[23](23-view-per-session.md)、
[24](24-width-replay-fidelity.md)、[25](25-session-facts-per-session.md)、
[26](26-ui-config-keys.md)、[27](27-herdr-detection-and-sidebar.md) 全部落地。

决定见 [`issues/12-grilling-main-column-after-move.md`](12-grilling-main-column-after-move.md) 第 4–6 条。

## 落地注记（2026-10-17）

- **`CONTEXT.md`**：新立**右栏**、**顶部页签条**、**平铺**、**格**、**焦点格**五条（各带
  `_Avoid_`；「格」明写避开 pane / 窗格 / 分屏 / 面板）；改写**轨迹视图**（住右栏、不再常驻
  主列）、**对话视图**（主列唯一一页）、**左栏**（删「轨迹在主列页签条上」两句、补平铺态收起
  与 `Ctrl-O` 回执）、**前台**（平铺态 = 焦点格），并顺带修了**渲染器**词条里那句「主列页签条
  上的两页」。
- **`docs/render.md`**：外壳节重写（顶部页签条 + 左栏 / 主列 / 右栏的分区、4 行主列外壳预算、
  平铺态）；两个视图节改「对话在主列、轨迹在右栏」并删「永远可达」；新增**右栏**与**平铺**两节；
  时间轴、时间戳、键盘（`Ctrl-V` 语义、两套提示行按键盘归属、右栏键盘段）、点击分派与覆盖层
  归属一并跟上。
- **`docs/tui-manual-checklist.md`**：新增 `## ㊷ 平铺与右栏` 七步（顶部条 / `Ctrl-V` 层序 /
  `Ctrl-R` 与加号、移出、`Ctrl-2` / 80×24 降级 / herdr 默认关左栏 / `tile_limit` / 状态行读数）。
- **ADR**：0023「平铺活在一个进程里，不交给外层 pane」、0024「轨迹的落点：主列 → 右栏，第三版」；
  `scripts/check-doc-size.py` 的 `DOC_FILES` 补上这两份。
- **`README.md`** 顺带跟上（TUI 长什么样那一段与图、ADR 索引、前端那一句）：它不是本票验收项，
  但「仓库里的说法」也归它。
- `docs/lifecycle.md` 按要求不动。
- 检查：`python3 scripts/check-doc-size.py` **OK**（53 份文档、单元 ≤500、入口三份在预算内）。
  `scripts/check-language.py` 仍卡在它既有的那条 `src/cli.rs: 清单里的文件不存在`（文件早在
  cli 拆分时删掉，与本次无关）。
- 审查后修：文档里「平铺降级」那句「并说一句」现在有对应实现（窗口缩小的帧回退也会给回执），
  render.md 的 40/28 档列宽（31 / 28）、横带两行、组头独占一行、120 列提示行 50 列与右栏可见
  的主列地板都按实现校正；CONTEXT 与 README 同步。
- 覆盖强度补记：`map_key`（Ctrl-R/V/O/1–4/←→）与真进程组装态（`boot_state`）各补了单测，
  启动夺键盘与 `Ctrl-←→` 两个缺口就是从这两处漏过的。
