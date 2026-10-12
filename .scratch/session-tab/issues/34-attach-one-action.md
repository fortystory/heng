# 34: 装载收成一个动作

Type: implement
Status: done
Blocked by: 33
Part of: ../map.md

**要打通的行为**：把「装载」收成 `cli.rs` 里的一个 module —— 一个入口、一份可测的计划、一处说话。今天同一段装配在四处各拼一遍（启动 `src/cli.rs:576`、新建与翻页 `:2028`、只读预览 `:2048`、点一行 `:2165`）：`CliActor` 构造三份、`adopt_with` 加 `asker.attach` 的两步仪式四处、`host.switch` 与 `console.switch_session` 各写各的。代价已经写在同一段代码里：`Loaded.filled`（旧流缺格的如实说明）只有 `-c` 那条路说（`:526`），界面点选那条路丢掉它；`host_count` 只有新建与关闭发（`:2068`、`:2107`），点一行不发。

**来源**：票 [18](18-attach-path.md) 的收口记录说 `load(AssemblyParts) -> Loaded` 就是那条共用路径，但共用只到组装为止；2026-10-12 一轮架构访谈（Q1–Q26）把剩下的半段定成这一票。`spec.md` §2 的意图一个字没变。

## 冻结的 interface（落地后）

```rust
enum AttachTarget {
    Existing(StoredSession),
    New { cwd: PathBuf, inherit: FrontFacts, rollover: bool },
}

enum AttachPlan {
    SwitchOnly { id: SessionId },                 // 已在宿主表：切前台，不装载
    Attach { id: SessionId, cwd: PathBuf, notices: Vec<Notice> },
    Refuse { reason: Refusal },                   // 工作区不在 / 流读不出 / 流里没记下工作区
}

enum Refusal { NoCwd, CwdGone(PathBuf), Unreadable(String) }
enum Notice { Loading, FactsFilled(Vec<&'static str>), NewOpened }   // 结构，不带中文

async fn attach(host, kit, console, requests, target) -> Result<SessionId, ()>
```

与票面冻结版的四处出入，都是落地时的事实：

- **`Attach` 不带 `read_only`**：它由 `Loaded.read_only` 在**装载之后**给出（Q8 决定 `crate::load()` 不拆开），计划阶段还不知道。
- **`New.rollover` 是 `bool` 不是载荷**：载荷交接留在薄壳 `open_new`（Q15），module 只用它决定**不**说那句新开回执。
- **`Target` 落名 `AttachTarget`**：`AttachPlan` 已经在用 `Attach` 这个词，两个类型同名会读混。
- **`AttachCheck::AlreadyHosted` 归 `SwitchOnly`**：与 `hosted` 是同一件事的两条路（竞态的另半）；理由文本因此只剩 `Refusal` 三档。

## 冻结的验收

- [x] `mod loading` 落在 `cli.rs` 那一区（现在 2003–2330）；`close_session` 留在 module 外 —— 关是把一行从宿主表里卸下来，不是装载
- [x] **计划是纯的**：`attach_plan(kind, hosted, id, cwd, check)` 不吃 `AssemblyKit`，在 `cli.rs` 的 `mod tests` 里断言（计划表六格一条测试）
- [x] `Existing`：已在宿主表 → `SwitchOnly`（切前台加 `switch_session`，不装载、不说「正在装载」）；不在 → `attach::check` → `crate::load` → 入表 → 接应答者 → 切前台 → 说
- [x] `New`：`SessionStore::create` 在 module 内；**rollover 交接**由 `open_new` 薄壳在装载返回后发
- [x] 装完要说的话只在一处（`say_notice`）：`Loading` 只在 `Existing` 说；`FactsFilled` 两条路都说；`host_count` 由 module 发 —— 后两条是**意图内的行为变化**（界面装载路径以前不说缺格、也不发读数）
- [x] cwd 用票 [33](33-attach-cwd-single-source.md) 的 `attach_cwd`；新建那条路的 cwd 由调用方给（空会话还没有流）
- [x] 拒装各档一句话（`Refusal::message` → `wording.rs`）：`NoCwd` / `CwdGone` / `Unreadable`；「找不到这条」留在调用点
- [x] `crate::load()` 的签名不动（`health` 在 `check` 与 `load` 各读一次 —— 已知小账）
- [x] 启动路径（`src/cli.rs:576`）不走这个 module
- [x] 测试：计划表六格加两条 `attach_cwd`（`cli.rs` 的 `mod tests`）。**端到端没有搭**：`attach` 的依赖是真 `AssemblyKit`（真 provider、真 MCP），票面写的「走 `tests/session_host.rs` 那一路」不成立 —— 那条路用的是 `SessionHost::adopt`，不经过 `cli::loading`。要端到端得先有一个可注入材料的装置，那是另一票（记在进度里）

## 明确不做

- **`FrontFacts` 陈旧**：它只在启动取一次（`src/cli.rs:569`），`/model` 之后不刷新，而 `ui_facts` 从流上读（`:1943`）。另一张票 —— 它的解在「从流派生」那一侧。
- **`SessionDir` / `WorkCwd` 新类型**：见票 [33](33-attach-cwd-single-source.md) 的说明。
- **会话页那三行**（`list_all` 加 `find` 加「找不到这条」）：留在点击的调用点。
- **关闭那条路的 `host_count`**（`src/cli.rs:2107`）：不归装载管。

## 进度

**2026-10-12 落地。**

- `mod loading`（`src/cli.rs:2003–2330`）：`AttachTarget` / `AttachPlan` / `Refusal` / `Notice` / `attach_plan`（纯）/ `say_notice` / `attach` / `open_new` / `open_session`；`attach_cwd` 从票 33 那份搬进来。
- 四处装配收到一处：`open_new` 变成薄壳（建目录进 module，只留 rollover 交接），`open_session` 只留 `list_all` 加 `find` 加「找不到这条」三行，`attach` 一条路走完 check → load → adopt → 接应答者 → 切前台 → 说。
- 三档新措辞进 `wording.rs`：`session_cwd_gone` / `session_stream_unreadable` / `session_loading` / `new_session_failed` —— 原来散在 `cli.rs` 的 inline `format!`。
- **两处意图内的行为变化**（验收第 5 条）：界面点选装载现在也说 `Loaded.filled` 的缺格说明，也发 `host_count`。
- 启动路径没动（Q19）：它前后挂着开幕式，只有一处，不是重复的来源。

**欠账**：`attach` 的端到端仍只能靠真终端（`docs/tui-manual-checklist.md` ㊶ 那一族）；「可注入材料的紧凑测试装置」是另一票 —— 它要先把 `AssemblyKit` 里那几样（provider 工厂、MCP、题目表）拆成可替换的输入。
