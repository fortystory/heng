# 34: 装载收成一个动作

Type: implement
Status: ready-for-agent
Blocked by: 33
Part of: ../map.md

**要打通的行为**：把「装载」收成 `cli.rs` 里的一个 module —— 一个入口、一份可测的计划、一处说话。今天同一段装配在四处各拼一遍（启动 `src/cli.rs:576`、新建与翻页 `:2028`、只读预览 `:2048`、点一行 `:2165`）：`CliActor` 构造三份、`adopt_with` 加 `asker.attach` 的两步仪式四处、`host.switch` 与 `console.switch_session` 各写各的。代价已经写在同一段代码里：`Loaded.filled`（旧流缺格的如实说明）只有 `-c` 那条路说（`:526`），界面点选那条路丢掉它；`host_count` 只有新建与关闭发（`:2068`、`:2107`），点一行不发。

**来源**：票 [18](18-attach-path.md) 的收口记录说 `load(AssemblyParts) -> Loaded` 就是那条共用路径，但共用只到组装为止；2026-10-12 一轮架构访谈（Q1–Q26）把剩下的半段定成这一票。`spec.md` §2 的意图一个字没变。

## 冻结的 interface

```rust
enum Target {
    Existing(StoredSession),
    New { cwd: PathBuf, inherit: FrontFacts, rollover: Option<Rollover> },
}

enum AttachPlan {
    SwitchOnly { id: SessionId },                 // 已在宿主表：切前台，不装载
    Attach { id: SessionId, cwd: PathBuf, read_only: bool, notices: Vec<Notice> },
    Refuse { reason: Refusal },                   // 不在 store / cwd 读不出 / 拒装
}

enum Notice { Loading, FactsFilled(Vec<&'static str>), NewOpened }   // 结构，不带中文

async fn attach(
    host: &mut SessionHost<CliCommand>,
    kit: &AssemblyKit,
    console: &ConsoleHandle,
    target: Target,
) -> Result<SessionId, ()>
```

## 冻结的验收

- [ ] `mod loading` 落在 `cli.rs` 现在 `open_new` / `open_session` 那一区（1986–2230）
- [ ] **计划是纯的**：不吃 `AssemblyKit`，只吃宿主表的只读查询（`contains` / `foreground`）加 `StoredSession` 与 cwd；能在 `cli.rs` 的 `mod tests` 里断言
- [ ] `Existing`：已在宿主表 → `SwitchOnly`（切前台加 `switch_session`，不装载、不说「正在装载」）；不在 → `attach::check` → `crate::load` → 入表 → 接应答者 → 切前台 → 说
- [ ] `New`：`SessionStore::create` 之后走同一条路；**rollover 交接**（`CliCommand::Rollover`）由 `open_new` 薄壳在装载返回后发
- [ ] 装完要说的话只在一处：`Loading` 只在 `Existing` 说；`FactsFilled` 两条路都说；`host_count` 由 module 发
- [ ] cwd 用票 [33](33-attach-cwd-single-source.md) 那个来源函数
- [ ] 拒装各档一句话：不在 store / cwd 读不出 / `MissingCwd` / `Unreadable`；文本走 `wording.rs`
- [ ] `crate::load()` 的签名不动（`health` 在 `check` 与 `load` 各读一次，记成已知小账）
- [ ] 启动路径（`src/cli.rs:576`）不走这个 module
- [ ] 测试：计划值那层在 `cli.rs` 的 `mod tests`；端到端走 `tests/session_host.rs` 那一路（真 store 加临时目录加假 actor）

## 明确不做

- **`FrontFacts` 陈旧**：它只在启动取一次（`src/cli.rs:569`），`/model` 之后不刷新，而 `ui_facts` 从流上读（`:1943`）。另一张票 —— 它的解在「从流派生」那一侧。
- **`SessionDir` / `WorkCwd` 新类型**：见票 [33](33-attach-cwd-single-source.md) 的说明。
- **会话页那三行**（`list_all` 加 `find` 加「找不到这条」）：留在点击的调用点。
- **关闭那条路的 `host_count`**（`src/cli.rs:2107`）：不归装载管。

## 进度

（还没开工。）
