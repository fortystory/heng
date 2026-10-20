# 03: 滚动条自己算位置，只在滚动时出现

Type: implement
Status: done
Blocked by: —
Part of: ../spec.md

**What to build:** 一条共享的滚动条读法：位置与长度**自己算**（整数运算），并让主列先接上。
它同时修掉今天那个真机缺陷 —— 内容贴底时滑块停在轨道中段（ratatui `Scrollbar` 的分母是
`content_length − 1 + viewport_length`，把 `position` 取到 `total − height` 也到不了轨底）。
滚动条**只在滚动时出现**：该栏最后一次滚轮 1 秒内才画，过了消失；时间由既有的 60ms 脉冲推进，
不另起时钟。

算术（`v` 视口行数、`n` 总行数、`track` 轨道格数、`d = n − v`）：`d == 0` 不画；
滑块长 `l = max(1, round(track × v / n))`；起点 `s = round(top × (track − l) / d)`。

- [x] 主列内容贴底时：滑块最后一格落在轨道最后一格（不是中段）。
- [x] 顶部时滑块第一格落在轨道第一格。
- [x] 短内容（总行数只比视口多几行）同样贴底、同样贴顶 —— 那是旧公式偏得最明显的一档。
- [x] 滚动主列时滚动条出现；停手 1 秒后消失；未滚动时不画。
- [x] 只画滑块（`█`），不画轨道与箭头；`MUTED`，视口不在底部时 `BOLD`。
- [x] 出现与消失不改变内容宽度：同一帧的文字列数相同。
- [x] 不依赖 ratatui `Scrollbar` 的位置公式；时间由既有脉冲推进，用例不睡真实时间。
- [x] `cargo test` 绿；`cargo clippy --all-targets` 无新增告警。
