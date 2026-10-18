# 架构深化：一轮回顾后的候选池

来源：2026-10-12 一轮 `/improve-codebase-architecture`（扫描范围按 YAGNI 划在近 120 次提交的热点上：
`src/render` 197 次改动、`src/cli.rs` 36、`src/session` 47）。报告里的九条候选有两条已经落地 ——
**装载**（[`session-tab` 票 33 / 34](../session-tab/issues/34-attach-one-action.md)）与 **cli 拆分**
（[`cli-modules` spec](../cli-modules/spec.md)）。剩下的七条与两笔欠账记在这里。

**这是意向池，还没有折成 spec**：想推进哪一条，就走一轮访谈把它折成 spec 再拆票。

判据一句话：**locality 与 leverage** —— 改动能不能集中到一处，一处 interface 能不能供多个 caller
与测试使用。每条末尾括号里是报告给的推荐强度。

## 渲染层

- **把截断收回 `width` module**（Strong）：折行只有 `pane::wrap_line` 一处，截断却有五处，其中四处按
  **字符**（`transcript.rs:664`、`wording.rs:934`、`changes.rs:589`、`files.rs:112`）—— 中文界面里字符
  数不等于列数；`markdown::wrap_code:678` 还自己折一份。seam 收回 `width.rs`：量宽、按列切、按列折同
  处，缩进续行成为折行的显式选项。与 [ADR 0008](../../docs/adr/0008-markdown-parsing-by-pulldown-cmark.md)
  的「换行归我们」同向，只是没走完。
- **左栏四页各自成 module**（Strong）：`SessionsPage` / `FilesPage` / `ChangesPage` 只有字段
  （`tui.rs:9673`）—— 一页的绘制、键、指针、滚动、焦点散在 `impl TuiState`（4280 行）的 5 处，页与页
  还互相清 `rect`（`:8773`、`:8789`），同形状重复四遍。seam 放在「一页」，`TuiState` 只留「当前哪页」。
- **谁持键盘，一处判据**（Strong）：`key()`（`tui.rs:7269`）是 16 层阶梯，`click_at` / `wheel_at` /
  `keyboard_in_the_input:14534` 各持一份字段子集；`tui.rs:2588` 的注释自称这是「第四套机制」。seam 放在
  「层」—— 每层自己声明是否持有。先例是 [ADR 0010](../../docs/adr/0010-questionnaire-keys-dispatch-by-zone.md)。
- **`headless` 做 `Block` 的第三个 adapter**（Strong）：`headless.rs:103` 自认养着自己一套叙述（28 个
  匹配臂），而 `transcript.rs` 32 个、`tui.rs` 的画家 53 处；同一句话两处拼（`headless.rs:141` 与
  `plain.rs:115`）。代价：`BlockId` 的身份记账
  （[ADR 0021](../../docs/adr/0021-line-identity-comes-from-the-event-envelope.md)）会进机器模式。
- **`View` 是投影的 seam**（Worth exploring）：44 字段的 `View` 被 `self.view.` 穿透 166 处；宽度重放
  （`:4313`）住在 `TuiState` 里，却要同时懂 `View` 的六个字段。seam：「事件 + 当前宽度」进，「画出来的
  行」出。
- **措辞层回到「值 → 短语」**（Worth exploring）：`wording.rs` 的 362 个 `pub fn` 里有十来条是排版决策
  （`status_row:2908`、`hint_row:1798`），另一半阶梯在 `layout.rs:604`。seam：「哪几段、按什么顺序放弃」
  一处。删掉 `wording.rs` 复杂度是放大而不是消失 —— 这条说的是分家，不是「它 shallow」。
- **`pane` 的平行表**：靠「丢了几条」这个整数与调用方手工对齐（`pane.rs:119`、`:503`、`:164`）——
  2026-09-23 那次「历史从屏幕上消失」就出在这。
- **命中测试两套机制**：`Regions`（`tui.rs:3186`，18 变体）只覆盖一部分动作，另一半各自重算几何
  （`:5024`、`:7848`、`:4957`、`:6696`、`:6735`）。

## cli 与会话

- **宿主表的投影只算一处**（Strong）：「宿主表现在长什么样」劈成两半 —— 三份上一帧快照与边沿判定住在
  `cli/interactive.rs` 的 80ms 轮询里，渲染器为此长出 7 个专用推命令（`input.rs:316` 起）。三条规则
  （未读的寿命、`N` 只算后台、单会话标题不变）因此只能靠真机走查
  （[`tui-manual-checklist.md` ㊶](../../docs/tui-manual-checklist.md)）。
- **命令名的知识两处**：对拍测试自己写着「两边各有一份真相」（拆分前在 `cli.rs:6681`）。
- **渲染器要认识的领域事实有六种碎片**：`input.rs:133` 的 20 变体里十条是宿主表与会话清单的事实。
- **会话级轻状态是一张手写搬运清单**：`SessionLight` 13 字段加 `stash_light` / `load_light`；要动它得先
  重开 [`session-tab` spec](../session-tab/spec.md) §6。

## 欠账

- **可注入材料的测试装置**（票 34 留下）：`cli::loading::attach` 的端到端依赖真 `AssemblyKit`（真
  provider、真 MCP）。把材料里那几样拆成可替换的输入之后，装载路径才能在没终端、没网络的测试里断言
  「装完说哪几句」。
