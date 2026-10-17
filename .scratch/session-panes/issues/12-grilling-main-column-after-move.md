# 轨迹搬走之后：主列、页签条与文档

Type: grilling
Status: resolved
Part of: ../map.md
Blocked by: 08

## 问题

这一次搬家（冻结项 7）把 `trace-tab` / `trace-in-main` / `trace-shortcuts` 三张 spec 的落点都推翻了。
[右栏的键盘与页面容器](08-grilling-right-rail-keyboard.md) 定完手势之后，这一票收尾：

1. **主列页签条删掉**。`trace-shortcuts` spec §1 的页签条与 `Ctrl-V` 一起被推翻；
   `docs/render.md` 里「轨迹页是主列的一页，所以 `Ctrl-O` 收起左栏或终端窄于 80 列时它照旧在屏幕
   上」那段要重写。主列只剩对话视图之后 chrome 少一行，`layout.rs` 的分区跟着改。
2. **两套提示行**。`KEY_HINTS` / `TRACE_HINTS` 重写：pane 那一套还叫 `TRACE_HINTS` 吗；标签态与
   平铺态各给哪一套（与
   [平铺的手势与焦点](07-grilling-pane-gestures-and-focus.md) 的键位对齐填）。
3. **`CONTEXT.md` 词条**。要**新立**哪些（右栏、顶部页签条、平铺、焦点会话……）、要**改**哪些
   （**轨迹视图**那条「常驻主列」、**对话视图**、**左栏**、**前台**相关的说法）。**一处已知的
   撞车**：`src/render/pane.rs` 的 `Pane` 是**转录滚动缓冲**（`pane.rs:30`，一个会话两个实例，
   `tui.rs:793` / `:798`），本图说的「pane」在代码里**没有对应类型** —— 给本图这个「屏上的一块」
   定一个不与它重复的名字（或明确限定），并把 `_Avoid_` 写清
   （[宿主表与多会话基础设施](01-research-session-infrastructure.md) 第 4 条报的）。用
   `/domain-modeling` 的口径：不造与既有词重复的新词、每条给 `_Avoid_`。
4. **ADR 判据**。这次有没有「难反悔 + 不讲就意外 + 真的取舍」三条都成立的决定要走 ADR？候选：
   平铺的进程模型（同进程 vs 多进程）、轨迹的落点（主列 → 右栏，第三版）、herdr 的检测与默认值。
   逐条给判据结论；**不立也要写清为什么**。
5. **文档面**。`docs/render.md`（轨迹一节、左栏一节、键盘一节）、
   [`docs/tui-manual-checklist.md`](../../../docs/tui-manual-checklist.md)（真机走查要新增哪几项）、
   [`docs/lifecycle.md`](../../../docs/lifecycle.md)（平铺如果改到生命周期图的节点，就要更新那几张
   mermaid 与 `scripts/lifecycle-check.py` 的锚点）。改动要过
   [`scripts/check-doc-size.py`](../../../scripts/check-doc-size.py)。
6. **顺带收不收 `architecture-deepening` 的两条**。那份种子里的「左栏四页各自成 module」与
   「`View` 是投影的 seam」与本图正面相邻（本图要动左栏与 `View`）。判「顺带做」还是「不动」，
   给理由；顺带做的话它就是一张新的实现票，不是本图的决策。

## 作答

**（2026-10-17，一轮七问，维护者全部按推荐。这是本图最后一票。）**

**一句话：主列只剩对话视图、页签条整条拆掉；提示行按「键盘在谁手里」分两套；本图那个「屏上的
一块」定名**格（`Tile`）**；立两条 ADR；文档面五处收口；`architecture-deepening` 两条不做。**

### 1. 主列页签条删干净

- 删：`MainTab`（`tui.rs:3161` 前的定义）、`TuiState.main_tab`（`:2563`）、
  `HitAction::SwitchMainTab`（`:3161`）、`set_main_tab` / `switch_main_tab`（`:5957-5974`）、
  `draw_main_tab_bar`（`:9351` 一带）、`layout::Regions.main_tabs`（`layout.rs:188`）与
  `MAIN_TABS_ROWS`（`layout.rs:53`）。
- 回落到对话视图：按 `main_tab` 分派的滚轮 / 翻页 / 贴底（`tui.rs:4585-4603`）与
  `if main_tab == Trace`（`:7349`、`:8159`）。
- `CHROME` 6 → 6 由票 09 的顶部页签条（2 行）补回；`## 外壳` 的 6 行预算重写。
- 命中表：`SwitchMainTab` 那一支删掉，屏幕上的页签点击只剩左侧栏那条。

### 2. 两套提示行

- `KEY_HINTS`（`wording.rs:1658`）：`ctrl-v 轨迹` → **`ctrl-v 右栏`**；末尾 `ctrl-o 左栏` 前插
  **`ctrl-r 平铺`**。共 9 条。
- `TRACE_HINTS` → **改名 `RAIL_HINTS`**（`wording.rs:1683`）：条目照旧，`ctrl-v 对话` →
  **`ctrl-v 交还`**。
- 判据从「页」变成**「键盘在谁手里」**：右栏拿住键盘时用 `RAIL_HINTS`，否则用 `KEY_HINTS`；
  **平铺态不另开第三套**（每格只有对话视图，票 08 第 12 条）。
- `ctrl-1…4` 不进表（加速档，与 `g` / `G` 同类）。
- 提示行那组按宽度分档的断言要按新条目重取实测值。

### 3.「格（Tile）」定名

- 本图说的「屏上的一块」= **格（`Tile`）**；平铺 = 一组格；当前那个 = **焦点格（`FocusedTile`）**。
- `_Avoid_`：pane / 窗格（那是 `render::pane::Pane`：转录滚动缓冲）、分屏、面板。

### 4. `CONTEXT.md`

- **新立五个**：右栏（`RightRail`）、顶部页签条（`TabStrip`）、平铺（tiles）、格（`Tile`）、
  焦点格（`FocusedTile`）。
- **改四个**：
  - **轨迹视图**（`CONTEXT.md:318`）：住**右栏**、不再「常驻主列」；主列不因它变窄。
  - **对话视图**（`:371`）：主列**唯一**一页，去掉「页签条上」与「另一页」的说法。
  - **左栏**（`:394`）：删「`轨迹` 不在这一列里，在主列页签条上」与「主列顶上另有一条同款画法的
    页签条（对话 / 轨迹）」两处。
  - **前台**（`:68`）：补一句平铺态下「前台 = 焦点格」（票 07）。
- 其余词条不动。

### 5. ADR 判据

立**两条**（三条判据都成立）：

- **0023 平铺的进程模型**：同进程多会话（复用宿主表与 `Harness`），不做多进程、不把分屏交给
  herdr / tmux 的 pane。取舍：共享宿主表与团队通信，换来隔离的放弃。
- **0024 轨迹的落点（主列 → 右栏，第三版）**：为什么又搬、为什么不两边都放（一个家、一个宽度
  来源、一套键盘层序）。

**herdr 的检测与默认值不立 ADR**：小、可逆、没有真正的取舍，写进 `docs/render.md` 与
[`docs/herdr.md`](../../../docs/herdr.md) 就够。

### 6. 文档面

- **`docs/render.md`**：
  - `## 外壳`（`:94` 起）：6 行预算重算（顶部页签条 2 行 + 主列两条线 + 状态行 + 提示行）、
    左栏顶部下移 2 行、右栏从条下起算。
  - `## 两个视图`（`:216` 起）：写成「对话视图在主列 + 轨迹在右栏」，删「主列页签条上的两页」
    与「永远可达」那段（`:256`）；`## 时间轴`（`:275` 起）只画 Compact 一档（票 08）。
  - `## 键盘`（`:487` 起，提示行在 `:549-562`）：`Ctrl-V` 改指右栏、两套提示行的判据改成
    「键盘在谁手里」。
  - `## 折叠与过滤`、`## 拖选与复制`、`## 可点链接`：把「轨迹页」改成「右栏」。
  - **新增「右栏」一节**：容器、页面标签（只有一页时不画）、40 / 28 两档里轨迹的形态、
    `Ctrl-V` 语义、焦点跟会话。
- **`docs/tui-manual-checklist.md`**：加一条真机项（顶部页签条、`Ctrl-R` 平铺、`Ctrl-1…4`、
  右栏开关与层序）。
- **`docs/lifecycle.md` 不动**：平铺不新增进程或事件，图讲的是进程生命周期，
  `scripts/lifecycle-check.py` 的锚点不用改。
- 全部过 `scripts/check-doc-size.py`。

### 7. `architecture-deepening` 两条：都不做

- **左栏四页各自成 module**：与本图的左栏改动（宽度、默认关、分工）无关；混做会让 spec 说不清
  哪一步是为什么。
- **`View` 是投影的 seam**：票 10 已经把 `View` 改成每会话一份；seam 重构叠上去会让「投影常驻」
  与「投影的接口」搅在一起。
- 两条各自另开 effort。

### 图的收尾

本票 resolved 之后，[图](../map.md) **没有 fog**：

- 「平铺态的新会话 / 装载 / 关闭 / 翻页语义」已由票 07 拍到「够 spec 动手」的那一档；
- 「pty 回归的形状」是**实现阶段**的活，不是决策；
- 「平铺时的鼠标手势」与「团队视图的接缝」其实是 scope 边界 → 已移进图的**范围之外**。

可以进 `/to-spec`。

---

**产物**：答案写进本票的 `## 作答`，并且这一票是**最后一票**：它 resolved 之后，
[图](../map.md) 应该没有 fog，可以进 `/to-spec`。
