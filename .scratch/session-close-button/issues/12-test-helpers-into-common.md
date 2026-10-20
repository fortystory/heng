# 12: 测试辅助收拢进 `tests/common/`

Type: implement
Status: ready-for-agent
Part of: ../map.md
Blocked by: 11

**要建的：** `tests/render_layout.rs` 里还留着一份自己的 `facts` / `screen` / `row_text` /
`buffer` / `find_cell` / `click` —— 与 `tests/common/mod.rs` 里的那份重复。把它也换成共用模块，
一个造状态、画一帧、读屏、合成指针的写法只留一处。

## 验收

- [ ] `tests/render_layout.rs` 不再自带那份辅助，改用 `tests/common/`；文件里其它测试一个字
  不用改（除非某个辅助两边签名不同 —— 那时以 `common` 那份为准，并记下改动）。
- [ ] `cargo test` 全绿，用例数与收拢前相同（不因收拢丢用例）。
- [ ] `tests/common/mod.rs` 的模块文档与实际内容一致（它现在不做键盘事件 —— 那条文档已经
  改过；若最后还是不做，就别再写「键盘」二字）。

## 依赖

[11 词条、文档与被推翻条款的改写](11-terms-docs-and-overturned-clauses.md)（收尾票：文档与代码都
定下来之后再整理）。这票是票 `05` 落地时**浮现出来**的一张 —— 抽取共用辅助时只收了
`tests/session_page.rs` 与 `tests/pointer_hover.rs`，`tests/render_layout.rs` 那份太大，没动。
