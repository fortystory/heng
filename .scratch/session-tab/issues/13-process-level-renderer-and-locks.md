# 13: 渲染器与路径锁升进程级、`Harness: Send`

Type: implement
Status: done
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

## 实现记录

- **`RenderChannel`（`src/render/mod.rs`）**：进程级的一条通道 + 拥有它的那个任务，内部一份
  `Arc`、克隆很便宜。一场会话从它取一个带自己身份的 [`RenderHandle`]
  （`RenderChannel::handle(session)`）。`Renderer::spawn` 因此多接一个 `SessionId`（plain 会
  在 `Lagged` 时自己造诊断）。
- **`AssemblyParts.renderer: Renderer` 换成 `render: RenderChannel`**；`OpenedSession` 不再持
  渲染器与渲染任务（它曾经是「一场会话一份」，那正是挡着多会话的那件事）；`Harness` /
  `DiscussionHarness` 各持一个写端与那个渲染端。45 个测试文件的组装点跟着换形状。
- **收尾的顺序是这一票真正的坑**：渲染任务靠「没有写端了」结束，而 `RenderChannel` 自己也
  持着一份发送端 —— 握着它去 `await` 任务就是死锁（先在 `tests/discussion.rs` 上挂住 17 个
  用例）。所以通道那一份发送端放进 `Mutex<Option<Sender>>`，`shutdown()` 先把它放掉再 await；
  `Harness::shutdown` 同样先 `drop` 自己那个写端。
- **`PathLockTable`（`src/tools/paths.rs`）**：进程级的 `cwd → PathLocks`，键取规范化 cwd。
  同一个 cwd 拿到同一张（写互斥真的成立），不同 cwd 各一张（不因为工作区级那把 `exclusive`
  互相阻塞）。`cli.rs` 三个组装点（交互、讨论、probe）各在一个入口里建一张并把这一张分给该
  入口开出的每一场会话；`SessionScaffold.locks` 的形状不动。
- **`Harness: Send` / `DiscussionHarness: Send` 的编译期断言**落地（宿主表要把每一场会话钉在
  它自己的 tokio task 上）。
- 新增 `tests/path_locks.rs` 两条：同 cwd 共享一把（含带 `.` 的另一种写法）、异 cwd 不互相
  阻塞而同 cwd 的 `exclusive` 排队。
