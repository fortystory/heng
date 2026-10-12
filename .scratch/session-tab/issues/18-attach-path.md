# 18: 装载路径：切入历史会话

Type: implement
Status: done
Blocked by: 15
Part of: ../map.md

**要打通的行为**：能**装起来**一场磁盘上的历史会话，并且装起来之后它知道自己是谁 —— 模型、档位、权限模式、沙箱档从流上来；旧格式的流缺字段就回退到进程配置默认并如实说明；装不成可写的会话时降级成**只读预览**。

**来源**：[spec.md](../spec.md) §2；决策票 [`05-session-facts-on-attach.md`](05-session-facts-on-attach.md)。

## 验收

- [ ] 一条共用装载函数：`-c <id>` 与界面点选**同一条路径**；先做能立即判定的检查（已是前台 / cwd 不存在 / 流损坏），通过后交给装载 task
- [ ] `SessionStarted` 补 model / provider / effort / permission_mode / sandbox_mode 五个 `Option` 字段，全部 `serde(default)`；**不记** context_window 与沙箱可用性
- [ ] 新事件 `SessionFactsChanged`：会话事实的**全量快照**、log-only；**当前事实 = 流上最后一条**（与目标的派生纪律一致）
- [ ] 旧流缺字段 → 回退**进程配置默认**，并如实说明（提示行一条 + 常驻标记，合并成一句）
- [ ] 每次装载后补一条完整快照；环境类事实（沙箱 / 技能 / `AGENTS.md`）**重探**；技能菜单与模型上下文不同源时只记账、不进界面
- [ ] **只读预览**：损坏流三档 —— 尾行坏 → 可写、半读 → 只读、读不出 → 拒装；只读时凡写流的一律禁（提交、所有 `/` 命令、`Shift+Tab`、页内改名、新建），各回一句
- [ ] 宿主表加只读标志，**不动三态**
- [ ] **已在宿主表里的会话绝不重走装载**（否则 `resuming` 闸门会把进行中的工具调用读成崩溃遗留、合成一条「结果未知」）
- [ ] 测试：宿主接缝覆盖「新流 / 旧流回退 / 三档损坏流 / 只读禁令」；`-c` 与界面点选走同一条路径

## 进度

**2026-10-12 · 事实与判定层已落地（这一票尚未 done）**：`src/session/facts.rs`、
`src/session/attach.rs`、`tests/session_attach.rs`（9 条）、`tests/session_host.rs`（+3 条）。
已覆盖：

- **`SessionStarted` 补五格**（model / provider / effort / permission_mode / sandbox_mode），
  全是 `Option` + `serde(default)`；`SCHEMA_VERSION` 已在票 16 涨到 3，这一次不动它。
  `record_session_started` 填的是**那一刻**的取值（`Session` 的 config 与 mode），
  `SessionConfig` 因此多了 `provider`（`Config::session_config` 从 profile 名填上）。
- **新事件 `SessionFactsChanged`**：log-only、只追加、当前事实 = 流上最后一条；五处穷尽 match
  各自表态（投影零产出、headless 静默、transcript 不产块、`entry_of` 不列、`stats` 不计）。
- **`SessionFacts::current`**：最后一条快照 → 第一条 `SessionStarted` → 全 `None`；
  `missing()` 报出哪几格是空的，`with_fallback()` 逐格补进程配置默认并报出补了哪几样
  （「如实说明」那句话的原料）。
- **`Harness::record_facts`**：装载之后补一条完整快照的入口（`agent::record_session_facts`）。
- **流的三档健康**（`attach::health`）：尾行坏 → **可写**（`EventLog::open` 会截掉那条尾巴）、
  坏行后面还有好行 → **只读预览**、一行都读不出或打不开 → **拒装**。
- **`attach::check`**：已在宿主表 → `AlreadyHosted`（绝不重走装载，否则 `resuming` 闸门会把
  进行中的工具调用读成崩溃遗留）、cwd 目录不在 → `MissingCwd`、流读不出 → `Unreadable`，
  其余 `Ready`。
- **宿主表加只读标志**：`HostEntry::read_only` + `SessionHost::adopt_read_only`，**三态不动**
  （只读不是第四个状态）。

**还没做**：

- **只读预览的写流禁令**：提交、所有 `/` 命令、`Shift+Tab`、页内改名、新建各回一句 —— 执行点
  在 cli 与 TUI，等票 15 的接线。
- **装载函数本身**（`check` 之后交给装载 task、`assemble` 一场已存在的会话、装完切前台）与其
  cli 接线：`-c <id>` 与界面点选走同一条路径。
- 提示行那句「旧流缺字段，已回退进程配置默认」的具体措辞与常驻标记。
- 环境类事实（沙箱 / 技能 / `AGENTS.md`）重探：组装时本来就是重探，但还没有一条测试钉住它。


## 收口（2026-10-12）

装载函数与只读预览落地，这一票收口：

- **`load(AssemblyParts) -> Loaded`**（`src/lib.rs`）：一条共用路径 —— 组装 → `record_facts()`
  补一条完整快照 → 按 `attach::health` 判只读 → 用 `SessionFacts::with_fallback` 报出旧流缺了
  哪几样。`Loaded { harness, read_only, filled }`。`-c <id>` 已经走它（票 20 的界面点选也走）。
- **只读预览真的装得起来了**：`read_events` 对**中间**坏行是报错的（严格纪律），于是半读的流
  以前根本进不了 `assemble`。补了 `EventLog::open_read_only`（跳过错行、**绝不 repair**），
  `OpenedSession::open` 按 `health` 选严格 / 宽松那一档。**尾行坏仍走严格那一档** —— `open`
  会把那条坏尾巴收拾掉，所以它可写。
- **只读禁令**：`CliActor` 在提交之前判 `read_only`（退出与空行放行，其余各回一句
  `read_only_session`）；空闲段那个 `Shift+Tab` 也挡住；渲染侧经新 `ConsoleRequest::ReadOnly`
  拿到标志，页内 `r` 改名在只读下只回一句提示、不进改名模式。
- `with_fallback` 的语义收紧：只有**默认真的填上了值**才算「补上了」—— 默认也是空的时候那一
  格本来就空着，不是「回退到了进程配置默认」。
- 新增 `tests/session_host.rs` 2 条（装载后补快照且 `filled` 为空、半读的流装起来是只读预览）。
- **环境类事实（沙箱 / 技能 / `AGENTS.md`）重探**：组装期本来就是每次重探（`OpenedSession::open`
  现读），这一票不改它，也没有为它单写一条测试。
