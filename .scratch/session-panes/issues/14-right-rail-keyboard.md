# 14: 右栏开关与键盘层序

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 13

**要建的：** 右栏可以开、关，并接住键盘。

- **`Ctrl-V` 改成右栏开关**：一次按下 = 开栏 + 把键盘交给右栏；再按 = 交还键盘 + 关栏。
- **点输入区 = 交还键盘但右栏留着**。
- 右栏里照轨迹既有层序：点右栏任何位置拿键盘，`Esc` 依次退「搜索 → 过滤 → 选中 → 交还」。
- 屏宽 < 80（右栏整栏隐藏）时 `Ctrl-V` 给一句回执；提示行里右栏那一段不画。

## 验收

- [ ] pty：`Ctrl-V` 两次往返（开栏并拿键盘 → 交还并关栏）。
- [ ] pty：点输入区后键盘回输入区，右栏仍然画着。
- [ ] pty：右栏里 `Esc` 依次退三层，最后把键盘交还输入区。
- [ ] 右栏隐藏时按 `Ctrl-V` 出一句回执，不改变布局。
- [ ] `Ctrl-V` 不再触发主列两页的切换（那两页此时还在，由票 16 删）。
- [ ] 既有 TUI 测试全绿。

## 依赖

[13 右栏容器与轨迹搬家](13-right-rail-and-trace.md)。决定见
[`issues/08-grilling-right-rail-keyboard.md`](08-grilling-right-rail-keyboard.md)。
