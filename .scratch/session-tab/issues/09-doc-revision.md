# 文档回改：lifecycle.md 与逐面文档（实现期执行）

Type: implement
Status: in-progress
Blocked by: 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25
Part of: ../map.md

> **这不是决策票。** wayfinder 会话在 frontier 里遇到它会跳过（`Type: implement`，见 `docs/agents/issue-tracker.md`）；它由本 effort 的**实现批次**执行 —— 真正的执行前置是 `/to-tickets` 切出的那批实现票（编号待生成），这里只挂 `04`，因为回改清单取决于**会话宿主**的形状。
>
> **现在不要动 `docs/lifecycle.md` 的图**：§1.3 写明「没有代码落点的 roadmap 不进图」。本图目前只有决策，把多会话宿主画进去会让生命周期图变成路线图；等实现落地再回改。

## 目标

本 effort 落地后，把「一进程一会话」变成「同进程多会话宿主」这件事**如实写进那些会过期的文档**，并让护栏重新全绿。

## 要回改的地方（实现落地后逐条核对）

### `docs/lifecycle.md`（受 `scripts/lifecycle-check.py` 护栏）

- **§2 鸟瞰那句「一次进程只走一条主干」**：多会话宿主后，进程生命周期 ≠ 会话生命周期，「会话宿主」要成为骨架里的一层。
- **图 2「进程启动与收尾」**：`sess` / `loop` / `tail` 三个节点今天绑着「一场会话 = 一次进程」；宿主的装载 / 切换 / 关闭要进去，`tail` 的收尾要回答「后台会话写不写收尾」。
- **图 5 的 `rlog --> tui`** 与它那句「一个进程恰好一个渲染器」：宿主下要么收细（渲染器一个、但按会话隔离或重建，取决于票 08 的结论），要么改写；同一张图里补会话宿主与会话表的节点 / 边。
- **§7 不变量**：第 1 条（事件流是唯一真相源）与第 3 条（工具表组装后不变）要按「每场会话一份」重述；若票 04 定了新词（暂定**会话宿主**），术语与 `CONTEXT.md` 对齐。
- **附录证据表**：新增 / 改动的节点与边都补 `文件:行号` 与稳定符号；**每张图的节点集合必须等于它那一节证据表的键集合**（脚本的集合对账）。
- **验证**：`python3 scripts/lifecycle-check.py` 退出 0 且 `0 line drift`（只是 PASS 但带漂移提示不够）。

### `docs/render.md`

- 「两条页签条」一节列着左栏四页（`调用量` / `todo` / `文件` / `改动`）—— 加第五签后重写这一段；页签命中与宽度两档（40 / 28）的预算、降级若被票 06 改动，一并回改。
- **会话页那一页**（[票 06](06-session-page-shape.md)）：`会话` 在最前、行格式（标记 1 + 标题 27/21 + 间隔 1 + 时间 11/5）、五种状态标记、末行 `＋N 场`、三级降级阶梯（页区 ≥8 / 6–7 / <6 行）、页顶读数行（`N 场 · M 场在跑` + `新建` + `关闭`）、页内键盘与「收键盘的两页」名单（`docs/render.md:543-550` 加一页）。
- **空宿主态**（[票 07](07-new-session-and-unload.md) §5）：宿主表为空时页顶写 `0 场`、右端只剩 `新建`，页区写 `没有会话`，输入区禁用。
- **会话页的过滤**（[票 10](10-session-list-search.md)）：入口是**页内 `/`**（页签仍只走鼠标）、页顶那一行在过滤态**整行变成过滤行**（左 `会话` + 查询串、右 `n 个命中`）、实时筛、`Esc` 两层（取消/清过滤 → 还键盘）、空结果两句、过滤态下钉顶让位。`docs/render.md:581-596` 的「轨迹页搜索」可作写法对照，但**判据来源不同**：列表只筛标题 + 组头目录名，不读正文。

### `CONTEXT.md`

- **左栏**词条里的页签清单（四页 → 五页）；
- 若票 04 引入新词（**会话宿主** / **会话表**），按 `docs/agents/domain-modeling` 的格式补词条；若它同时满足「难以回头 / 不看上下文会奇怪 / 真的有权衡」，另起一条 ADR（对照 ADR 0017 的先例）。

### `docs/observability.md` 与 CLI 表

- `sessions ls` 的末列（`docs/observability.md:9` 一带与 `README.md:230` 的 CLI 表）：派生从「读 `SessionEnded`，没有就写未结束」改成**按末条事件**的三档 `停稳` / `中断` / `未收尾`（[票 07](07-new-session-and-unload.md) §6），`sessions stats` 的 stops 用同一条派生。
- `/clear` 的回执口径：从「已结束会话 X」改成「新开一场，旧会话留在列表里」（[票 07](07-new-session-and-unload.md) §8）；`clear_while_looping` 那道闸门取消后，`docs/` 里若有它的描述一并回改。

### 真机清单与索引

- `docs/tui-manual-checklist.md`：切换、后台跑、后台待答标记、标题改名各一条走查。
- `.scratch/README.md` 的 feature 行与 `README.md`「文档」表对 `docs/lifecycle.md` 的引用照旧核对一遍。
- `python3 scripts/check-language.py` 与 `cargo test` 不能因为这一轮变红。

## 不做什么

- 不重新设计生命周期图的深浅 —— 本票只回改，不加新图。
- 不把未落地的东西画进任何图。

## 评论

**2026-10-12 · 票 06 / 07 resolved 后补的清单**（都已就地写进上面的正文）：`docs/render.md` 加
「会话页那一页」与空宿主态两条；新增 `docs/observability.md` 与 CLI 表一节（`sessions ls` 末列
三档派生、`/clear` 文案口径、`clear_while_looping` 闸门取消）。`CONTEXT.md` 的**空宿主**词条
票 07 已直接写入，回改时只需核对格式与链接。


## 进度（2026-10-12）

**机械回改已落地：`python3 scripts/lifecycle-check.py` 现在 PASS 且 `0 line drift`。**

- 证据表里 **39 处行号漂移**逐条更新（多会话改造让 `src/cli.rs`、`src/lib.rs`、`src/render/*`
  的行号整体后移）；范围形式的条目（如 `241-255`）按原长度平移，写成 `新起点-新终点`。
- **两个改名符号**跟着改：`fn interactive_loop`（票 15 把整段循环搬进 `CliActor`、票 20 又把
  输入搬回主循环）→ `fn interactive`；`fn drain_renderer`（票 13 把渲染端升成进程级的
  `RenderChannel`）→ `fn shutdown`，落点也从 `src/lib.rs:1257` 挪到 `src/render/mod.rs:408`。
- 节点数与边数没动（74 / 78），图与证据表的集合对账照旧。

**还没做（这一票的内容回改）**：

- `docs/lifecycle.md` 的 §2 那句「一次进程只走一条主干」、图 2 的 `sess` / `loop` / `tail`、
  图 5 的 `rlog --> tui` 与「一个进程恰好一个渲染器」、§7 第 1 与第 3 条 —— 都要按**同进程多会话
  宿主**重述（术语用决策票 04 定的**会话宿主**）。
- `docs/render.md`（第五签 / 会话页那页 / 空宿主 / 过滤）、`CONTEXT.md`（页签四→五、新词条）、
  `docs/observability.md` 与 CLI 表（`sessions ls` 末列三档、`/clear` 口径）、
  `docs/tui-manual-checklist.md` 的四条走查。
