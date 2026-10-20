# 11: 词条、文档与被推翻条款的改写

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 05, 06, 07, 08, 09, 10

**要建的：** 把这一轮的形状写进词条与文档，并修掉被推翻的既有条款 —— 让下一个读的人看到的是一套说法。

## 验收

- [ ] `CONTEXT.md` 新立两条词条：**关闭按钮**（三处落点、悬浮才现、红 = 关闭色）、**悬浮目标**
  （指针当前落在哪一条会话上）。
- [ ] 改既有词条：**宿主表**（关闭 = 从表里移除；若是前台则前台落到相邻；不再禁止关前台）、**前台**、
  **平铺** / **格** / **焦点格**（焦点标记退场）、**会话页**（页顶按钮删掉、每行带关闭标记）。
- [ ] 语义色板新增「关闭」红，注释写清它与「错误」红的区别（撞值不混名）。
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
