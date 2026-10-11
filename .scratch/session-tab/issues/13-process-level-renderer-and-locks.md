# 13: 渲染器与路径锁升进程级、`Harness: Send`

Type: implement
Status: ready-for-agent
Blocked by: —
Part of: ../map.md

**要打通的行为**：渲染器（终端、渲染任务、渲染通道）与路径锁是**整个进程一份**，不是每场会话一份；`Harness` 可以被 `tokio::spawn` 拥有。用户可见行为不变 —— 这一片只是把「一场一份」与「进程一份」拆开，为多会话腾地方。

**来源**：[spec.md](../spec.md) §1、§10。

## 验收

- [ ] `render` / `render_task` 从 `OpenedSession` 抽出来，组装时注入；一场会话不再各持一份渲染器句柄
- [ ] `PathLocks` 升成进程级 `cwd → 表`：同 cwd 共享一张（两场写同一文件串行），不同 cwd 各一张（不因为工作区级那把 `exclusive` 互相阻塞）；三个组装点都走这张表
- [ ] `Harness: Send` 的编译期断言落地（例如一个 `fn assert_send<T: Send>()` 的调用），`tokio::spawn` 能接受它
- [ ] 既有调用点（交互、讨论、probe）都按新形状组装
- [ ] 既有测试全绿；新增一个「同 cwd 串行 / 异 cwd 不阻塞」的并发用例
