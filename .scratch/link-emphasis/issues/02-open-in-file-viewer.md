# 02：点工作区文件链接走文件查看器，URL 仍走 xdg-open

Type: implement
Status: done
Blocked by: —
Part of: ../spec.md

> 规格：[`.scratch/link-emphasis/spec.md`](../spec.md) §3、§4。
> 本票**只改打开那一路**：候选怎么认、颜色怎么画是 [票 01](01-link-colour.md) 与既有的
> [`clickable-links`](../clickable-links/spec.md) 的事。

## 目标

- 对话视图里点一条**工作区文件路径** → 打开**文件查看器**：内置预览或内嵌 nvim，由
  `[ui] file_viewer` 挑（与左栏文件页同一扇门）。
- 点一条 **URL** → 照旧交给 `xdg-open`（宿主侧 argv 直传）。
- 点一个**目录** → 照旧交给 `xdg-open`。

## 现状（开工前先复核）

- 点击落点：[`src/render/tui.rs`](../../../src/render/tui.rs) 的 `follow_link()` —— 它判「点在对话
  转录区 / 落在候选的列上 / 候选解析得出目标」，然后把目标字符串塞进 `self.open_request`。
- 运行期取走它：`src/render/tui.rs` 主循环里的 `state.take_open_request()` → `opener::open(&target)`
  → spawn `xdg-open` → 提示行打一句回执（`wording::opened` / `wording::open_failed`）。
- 候选解析：[`src/render/links.rs`](../../../src/render/links.rs) 的 `Target::resolve()` —— 路径要
  `canonicalize` 成功、**真存在**、且**落在工作区里**；URL 原样。
- 文件查看器那一扇门：`src/render/tui.rs` 的 `open_file(path)`（按 `self.view.facts.file_viewer.kind`
  分到 `open_file_detail` 或 `open_file_viewer`）；左栏文件页点一行文件走的就是它。

## 具体行为

1. `follow_link()` 里**先分路**：候选是 `Target::Url` → 照旧塞 `open_request`；候选是 `Target::Path`
   → 照旧解析（`resolve()`，判据一个字不动），解析不出 → **什么都不发生、不留回执**（今天一样）。
2. 解析出来的是**文件**（不是目录）→ 走 `open_file()`：内置预览那一档开一个详情覆盖层
   （`DetailKind::File`），`nvim` 那一档开一屏外来屏幕。**不给提示行打回执** —— 浮层本身就是回执。
3. 解析出来的是**目录** → 仍旧塞 `open_request`（交给 `xdg-open`），回执照旧。
4. **标题用工作区相对路径**：内置预览那一档的详情标题今天是 `path.to_owned()`。从链接点进来时路径是
   **绝对路径**（`resolve()` 给的），标题会变成一长串 —— 标题要用**工作区相对路径**（绝对路径且在
   工作区内时 `strip_prefix(cwd)`；不是绝对路径或不在工作区内就照旧原样）。左栏文件页那一路给的
   是相对路径，`strip_prefix` 失败 → 行为不变。
5. 平铺态不用另写：焦点格那一格与主列共用 `follow_link()`。

## 验收

- 单元测试（`src/render/tui.rs` 的 `#[cfg(test)]`）：
  - 落点是一条**工作区文件路径** → 打开的是文件内容那一档（内置预览：详情覆盖层起来、
    `file_viewer` 开着；`nvim` 那一档：`file_viewer` 有值），**没有** `open_request`。
  - 落点是一条 **URL** → `open_request` 仍是那个 URL。
  - 落点是一个**目录** → `open_request` 仍是那个目录路径。
  - 落点**不存在 / 在区外** → 既没有 `open_request`，也没有回执（今天一样）。
- `cargo test` 全绿；`cargo clippy --all-targets` 无警告；`cargo fmt --check` 干净。

## 评论

2026-10-21 落地。分路住在 `follow_link()` 里，判据一个字没动：

- **新增 `link_route()`**（一个纯函数：候选 → `Host` / `File` / `Nothing`）与 `LinkRoute`。URL 原样
  交给宿主；路径照 `Target::resolve()` 的判据解析（真存在 + 落在工作区内），解析不出来就什么都不
  发生、也不留回执（今天一样）。
- **文件那一档判的是「普通文件」**（`is_file()`），不是「不是目录」：目录读不了，而管道与设备
  读它会**卡住**整个界面 —— 那一档是「看文件」的。目录（与别的）仍旧交给宿主。
- **`open_file()` 直接复用**左栏文件页那一个岔口（`FileViewer::Builtin` → `open_file_detail`，
  `FileViewer::Nvim` → `open_file_viewer`），没有另写一条开文件的路。**不给提示行打回执** ——
  浮层本身就是回执。
- **标题改成工作区相对路径**（`file_title()`）：绝对路径且落在工作区里就 `strip_prefix(cwd)`；
  不是绝对路径或不在工作区内就照旧原样。左栏文件页那一路给的是相对路径，剥不掉前缀，行为不变。
  链接那一路给的是**解开过符号链接**的绝对路径，而工作区本身也可能带着一条符号链接 —— 剥不掉时
  按解开后的那个根再试一次。
- 这一路**不过 `bash` 工具、不过权限门、不进沙箱** —— 与 `xdg-open` 同一条纪律（宿主侧动作，模型
  触发不了它）。这句写进了 `follow_link()` 的注释。

⚠️ **本票范围外的一处必要改动，请 Lead 过目**：`tests/render_layout.rs` 里
`clickable-links` 那一节的 `clicking_a_workspace_file_asks_to_open_its_absolute_path` 断言的是**旧**
规则（「文件传绝对路径，交给系统去挑程序」），与本票的新规则直接冲突。已把它改成
`clicking_a_workspace_file_opens_it_in_the_file_popup`（不交给宿主 + 弹窗里是文件的内容）。

测试：`src/render/tui.rs` 的 `#[cfg(test)]` 新增 **6** 条 —— 文件链接开详情覆盖层（没有
`open_request`、没有回执）、标题是相对路径、URL 仍旧交给宿主、目录仍旧交给宿主、不存在或在区外
什么都不发生、`[ui] file_viewer = "nvim"` 那一档走的是同一扇门的另一档（外来屏幕；它起不来时回退
内置预览 —— 见下面收尾那一条）。`tests/link_open.rs`（新建）**4** 条：文件 → 界面上出现文件内容
且标题是相对路径、目录 → 交给宿主、URL → 交给宿主、不存在 → 什么都不发生。

实测：`cargo test --no-fail-fast` **1933 passed / 0 failed**；`cargo clippy --all-targets` 退出码 0、
零警告；`cargo fmt --check` 退出码 0；`python3 scripts/check-language.py` OK。

一条与本票无关的坑，记在这里省得下一个人再踩一遍：会话页那几条用例断言 90 分钟前是
「1 小时前」，而 `wording::session_age_at` 按**本地日期**算 —— 00:00 到 01:30 之间跑，90 分钟前
落在昨天，于是显示「昨天 23:49」，那几条会红（`tests/session_close_button.rs` 两条、
`tests/session_page.rs` 一条）。在 HEAD 的干净副本上同样红，与本票无关；过了 01:30 就是绿的。

真机走查没做的部分：没在真终端里点过一条工作区文件链接；`nvim` 那一档在真机上没看过（环境允许时
测试仍会起一个真 nvim，但断言不依赖它）；目录与 URL 仍走 `xdg-open`，宿主那边弹出来的东西没在真机
上验过。

2026-10-21 收尾（Lead 复核后）。**nvim 那条用例改成与环境无关**：原来那条在 PATH 上有 nvim 时
会起一个**真 nvim**，而 `pty` 开不出来时 `NvimViewer::spawn` 会失败 → 回退内置预览 —— 那是**环境**
决定的，拿它当断言就是拿环境当实现。改成
`clicking_a_workspace_file_with_the_nvim_viewer_takes_the_file_door`：断言**不交给宿主**
（`open_request` 是 `None`），且走的是**文件那一扇门**（外来屏幕，或它起不来时回退的内置预览，
二者之一）。那一档的判定住在 `open_file` 里（`FileViewer::kind` → 那一扇门），仓库里既有的文件页
用例已经钉住它；这里要钉的是「文件链接不交给宿主、而是走文件那一扇门」。`close_file_viewer()`
留着 —— 别把进程留在后台。

复跑（改完之后）：

- `cargo test --lib`（本票那 6 条）：**6 passed / 0 failed**
- `cargo test --test link_open`（本票那 4 条）：**4 passed / 0 failed**
- `cargo test --test render_layout`（链接那一组 7 条）：**7 passed / 0 failed**
- `cargo test --no-fail-fast`（全量）：**1933 passed / 0 failed**
- `cargo clippy --all-targets`：退出码 0，零警告
- `cargo fmt --check`：退出码 0
- `python3 scripts/check-language.py`：OK
