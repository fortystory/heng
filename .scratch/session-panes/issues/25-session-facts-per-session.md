# 25: 会话属性每会话

Type: implement
Status: ready-for-agent
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
