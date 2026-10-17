# 25: 会话属性每会话

Type: implement
Status: done
Part of: ../map.md
Blocked by: 17

**要建的：** 跟会话走的那几样事实不再钉在进程上。

- `cwd` / `home`、`files: FileIndex`（根 = 会话 cwd）、`catalog`（命令 / 技能名册）、`read_only`
  从单例改成**每会话一份**，切焦点跟着换。
- `catalog` 在**切焦点时也要重发**（今天只在会话组装后发一次）。
- 顺手修掉既有缺陷：**`SwitchSession` 不更新 `cwd` / `home`**，于是终端标题与 `@` 候选一直钉在
  启动那个会话的 cwd 上。

## 验收

- [ ] 切到别 cwd 的会话：终端标题与 `@` 候选跟着换（今天不换）。
- [ ] 文件页与 `@` 索引显示的是焦点会话的 cwd（pty）。
- [ ] `/` 菜单在切焦点后是那个会话的名册。
- [ ] 只读预览会话的写动作仍被禁（`read_only` 跟着会话）。
- [ ] 既有 `session-tab` 与文件索引测试全绿。

## 依赖

[17 平铺两态与几何](17-tiles-two-modes-and-geometry.md)。决定见
[`issues/10-grilling-per-session-view-state.md`](10-grilling-per-session-view-state.md)
与 [`issues/01-research-session-infrastructure.md`](01-research-session-infrastructure.md)。

## 落地注记（2026-10-17）

- **`cwd` 与文件索引进 `SessionLight`**：`stash_light` 收走、`load_light` 装回；第一次切到
  某个会话时用 `SwitchSession` 新带的 `cwd` 起一份干净的，并请一次重扫。`SwitchSession`
  因此多了 `cwd` 字段（宿主表里的会话从 `HostEntry.cwd` 取，新装载的从 `AttachPlan.cwd` 取）。
- **`/` 名册与只读标记按 id 存表**（`catalogs` / `read_only_by`），`ConsoleRequest::Catalog`
  与 `ReadOnly` 因此都带上了会话 id；切焦点取对应那一份。这比“切焦点时让循环重发一次”更省
  —— 名册在会话装载时本来就由它自己的组装推过一次，按 id 存就跟着会话走了。
- **`home` 保持进程级**：它是用户家目录，与“往哪个会话切”无关；终端标题里它只作为路径缩写
  的参照。`SessionFacts` 没有变 —— `cwd` 是渲染期输入，与面板要的那份事实集合不同层。
- 关闭会话时表里那几行一起清掉。
- 测试：`render_layout` 3 条（cwd → 标题、文件索引往返、名册按会话）、`session_page` 2 条
  （只读会话拒改名 / 普通会话照旧）。
