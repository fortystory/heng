# 11: 词条、文档与被推翻条款的改写

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 05, 06, 07, 08, 09, 10

**要建的：** 把这一轮的形状写进词条与文档，并修掉被推翻的既有条款 —— 让下一个读的人看到的是一套说法。

## 验收

- [ ] `CONTEXT.md` 新立两条词条：**关闭按钮**（三处落点、悬浮才现、红 = 关闭色）、**悬浮目标**
  （指针当前落在哪一条会话上）。**英文槽位对齐代码**：代码里那一个叫 `hover`（字段）/
  `hovered_session()`（只读口），词条若给英文名，用同一个词。
- [ ] 改既有词条：**宿主表**（关闭 = 从表里移除；若是前台则前台落到相邻；不再禁止关前台）、**前台**、
  **平铺** / **格** / **焦点格**（焦点标记退场）、**会话页**（页顶按钮删掉、每行带关闭标记）。
- [ ] 语义色板新增「关闭」红，注释写清它与「错误」红的区别（撞值不混名）。
  —— **色板那部分已由票 `05` 落地**（`palette::CLOSE`）；这张票只剩在 `CONTEXT.md` 的语义
  色板词条里把它记进去。
- [ ] 既有条目里**已经过时的那几处**逐处改掉（不是新立词条，是改旧的）：
  `CONTEXT.md` 的**前台**词条（`▍` 那个字形退场，前台靠标题色 `LIVE` 认）、
  **会话页**词条（页顶 `关闭` 入口删掉、每行带关闭标记、关闭标记顶替标记位）；
  `docs/render.md` 左栏一节里那几行（页顶两个入口 → 只剩 `新建`；标记列四态 → 前台那档
  退场）。`▍` 与页顶 `关闭` 在这些文件里都还写着 —— 留着就是下一个读的人按旧形状做。
- [ ] `docs/render.md` 的左栏、顶部页签条、平铺三节同步。
- [ ] `session-tab` spec 被推翻的十二行逐条改写：页顶 `关闭` 入口、前台不许关自己、关闭点名、
  只走鼠标、两步关闭、页内键位表、「不新增 `Ctrl-*`」、三处焦点标记、关闭动作硬编码焦点行、
  视图重建触发点、宿主表词条、docs 描述。
- [ ] `docs/tui-manual-checklist.md` 新增八条真机走查（含 herdr 常态一条）。
- [ ] 文档过 `scripts/check-doc-size.py` 的压表达规则。
- [ ] 不立 ADR 的说明留在 spec 与决策票里，文档里不另起一条 ADR。

## 依赖

[05](05-close-button-in-sessions-page.md)、
[06](06-close-foreground-and-land-on-neighbor.md)、
[07](07-close-button-in-tab-strip.md)、
[08](08-close-button-in-tile-header.md)、
[09](09-keyboard-close-shortcuts.md)、
[10](10-stale-cross-clearing.md)。决定见
[`../spec.md`](../spec.md) 的「G 术语与文档」与「H 被推翻的既有条款」，
逐条出处见[关闭这条动作的契约改写](03-grilling-close-contract.md)。
