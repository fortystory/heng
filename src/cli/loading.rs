//! 装载（票 34）：盘上一条旧会话与一个刚建的空会话走同一条路。
//!
//! `CONTEXT.md` 里**装载**那条词条就是这个 module 的 interface。它落在 cli 这一侧，因为库层
//! （`session::host`）不认识 `Config` 与 `CliActor`。
//!
//! （文件里曾经有两份同名实现：内嵌 `mod loading` 那份是拆分后的旧拷贝，从未被调用，2026-10-17
//! 清编译警告时删掉；下面这一份是模块级的那份。）

use super::goals::*;
use super::interactive::*;
use super::probe::*;
use super::*;

/// 组装一个会话所需的那些**可重复取用**的材料（票 20 的装载路径要它）。
///
/// 点一行切到一个还没装载的会话时，要装的是「同一批配置、同一个渲染端、同一张锁表」下的另
/// 一个。它与第一个会话那段组装必须保持同一批材料 —— 两处漂开的话，装载出来的会话会与启动
/// 那个不一样。
#[derive(Clone)]
pub(crate) struct AssemblyKit {
    pub(crate) config: std::sync::Arc<Config>,
    /// 启动时那个模型：新装载的会话用它。
    pub(crate) model: String,
    /// 发言者名（provider profile 的名字）。
    pub(crate) speaker: String,
    pub(crate) store: SessionStore,
    pub(crate) locks: crate::tools::PathLocks,
    pub(crate) channel: crate::render::RenderChannel,
    /// 真正往下送的应答者（票 22 在这上面包一层按会话的）。
    pub(crate) console_asker: Arc<dyn Asker>,
    /// 全进程一个前台指针：按会话包的应答者等自己成为前台要用它。
    pub(crate) foreground: tokio::sync::watch::Receiver<SessionId>,
    pub(crate) questions: Option<Arc<dyn UserQuestions>>,
    pub(crate) mcp: Arc<McpService>,
    pub(crate) home: Option<PathBuf>,
    pub(crate) mode: Mode,
    pub(crate) goals: GoalSetup,
}

impl AssemblyKit {
    /// 按会话包一个应答者（票 22）。
    ///
    /// 装完之后调用方要用 [`SessionAsker::attach`] 把那个会话的三态发送端填进去。
    fn session_asker(&self, id: &SessionId) -> Arc<crate::session::SessionAsker> {
        Arc::new(crate::session::SessionAsker::new(
            Arc::clone(&self.console_asker),
            id.clone(),
            self.foreground.clone(),
        ))
    }

    /// 按这份材料装一个会话 —— 走的是 `-c <id>` 那条共用路径（`crate::load`）。
    async fn load(
        &self,
        stored: StoredSession,
        cwd: PathBuf,
        inherit: Option<&crate::session::FrontFacts>,
        asker: Arc<crate::session::SessionAsker>,
    ) -> Result<crate::Loaded, String> {
        // 新建的那个会话**继承前台**的模型 / 档位 / 权限模式（票 21）；装载历史会话时不继承。
        let model = inherit
            .map(|facts| facts.model.clone())
            .unwrap_or_else(|| self.model.clone());
        let mode = inherit.map(|facts| facts.mode).unwrap_or(self.mode);
        let provider = OpenAiProvider::build(&self.config, &model, stderr_warnings())
            .map_err(|error| error.to_string())?;
        let mut session_config = self
            .config
            .session_config(&model)
            .map_err(|error| error.to_string())?;
        if let Some(effort) = inherit
            .and_then(|facts| facts.effort.as_deref())
            .and_then(effort_from_token)
        {
            session_config.params.reasoning_effort = Some(effort);
        }
        let tools = tools::with_mcp(
            tools::with_web(
                tools::with_dynamic(&self.config.tools, self.questions.is_some()),
                web_service(&self.config),
            ),
            Arc::clone(&self.mcp),
        );
        crate::load(AssemblyParts {
            provider: Box::new(provider),
            speaker: SpeakerId::Debater(self.speaker.clone().into()),
            config: session_config,
            render: self.channel.clone(),
            scaffold: SessionScaffold {
                cwd,
                log_path: stored.log_path.clone(),
                session_id: stored.id.clone(),
                tools,
                locks: self.locks.clone(),
                policy: Policy::for_mode(mode).with_outside_read(self.config.outside_read),
                asker: Some(asker),
                questions: self.questions.clone(),
                hook: None,
                home: self.home.clone(),
            },
        })
        .await
        .map_err(|error| error.to_string())
    }

    /// 渲染侧那一份「这个会话是什么」的事实 —— 票 19 的 `SwitchSession` 带它过去。
    ///
    /// 模型与权限模式从**流上**读（票 18 的会话事实），读不出才落回启动时那套；别的几样是
    /// 进程配置里定下来的值。
    fn ui_facts(&self, stored: &StoredSession, events: &[Event]) -> SessionFacts {
        let stream = crate::session::SessionFacts::current(events);
        let model = stream.model.clone().unwrap_or_else(|| self.model.clone());
        let mode = stream
            .permission_mode
            .as_deref()
            .and_then(Mode::parse)
            .unwrap_or(self.mode);
        let context_window = caps_for(&model)
            .map(|caps| crate::context::usable_input(&caps))
            .unwrap_or(0);
        let tools = tools::with_mcp(
            tools::with_web(
                tools::with_dynamic(&self.config.tools, self.questions.is_some()),
                web_service(&self.config),
            ),
            Arc::clone(&self.mcp),
        );
        SessionFacts {
            switchable: true,
            session_id: stored.id.as_str().to_owned(),
            session_dir: stored.dir.display().to_string(),
            model,
            context_window,
            mode,
            budget_limit: self.config.budget.limit,
            number_style: self.config.number_style,
            file_viewer: self.config.file_viewer,
            diff_viewer: self.config.diff_viewer.clone(),
            tool_schemas: tool_schemas(&tools),
            speaker_order: vec![self.speaker.clone()],
        }
    }

    /// 提醒一行 —— 走渲染通道（它才是「往屏幕上说一句话」的那一端）。
    pub(crate) fn notice(&self, id: &SessionId, message: &str) {
        self.channel
            .handle(id.clone())
            .notice(&render::wording::heng(message));
    }
}

/// 把流上那个档位词读回一档（新建时的继承，票 21）。
pub(crate) fn effort_from_token(token: &str) -> Option<crate::config::ReasoningEffort> {
    use crate::config::ReasoningEffort;
    [
        ReasoningEffort::Low,
        ReasoningEffort::Medium,
        ReasoningEffort::High,
        ReasoningEffort::Xhigh,
        ReasoningEffort::Max,
    ]
    .into_iter()
    .find(|effort| effort.as_str() == token)
}

/// 要装载的那一条（票 34）。
pub(super) enum AttachTarget {
    /// 盘上一条旧会话 —— 调用方已经按 id 找好了。
    Existing(StoredSession),
    /// 一个刚建的空会话：`cwd` 是它的工作区、`inherit` 是它继承的那份前台事实（票 21），
    /// `rollover` 说明这是一次翻页 —— 翻页不说那句新开回执（票 21 的口径）。
    ///
    /// 建目录与 rollover 的载荷交接留在薄壳 [`open_new`] 里；这一层只管把它装起来。
    New {
        cwd: PathBuf,
        inherit: FrontFacts,
        rollover: bool,
    },
}

/// 把一条装完要说的话说出来（票 34：话只在一处说）。
pub(super) fn say_notice(kit: &AssemblyKit, id: &SessionId, notice: &Notice) {
    match notice {
        Notice::Loading => kit.notice(id, &render::wording::session_loading(id.as_str())),
        Notice::FactsFilled(names) => kit.notice(id, &render::wording::facts_filled(names)),
        Notice::NewOpened => kit.notice(id, &render::wording::new_session_opened()),
    }
}

/// 装载一个会话：装盘上的一条，或装一个刚建的空会话（票 34）。
///
/// 一条路：**计划**（纯，见 [`attach_plan`]）→ 装载 → 入表 → 接应答者 → 切前台 → 说。
/// 盘上那一条的 cwd 从流上读、先过 `attach::check`；刚建的空会话没有流可查，cwd 由调用方给、
/// 检查那一档直接算放行。
pub(super) async fn attach(
    host: &mut SessionHost<CliCommand>,
    kit: &AssemblyKit,
    console: &ConsoleHandle,
    requests: &tokio::sync::mpsc::UnboundedSender<CliRequest>,
    target: AttachTarget,
) -> Result<SessionId, ()> {
    // 归一化：一条 `StoredSession`、它的流、它的 cwd、要过的那一档检查，以及「回执说不说」。
    let (stored, kind, events, cwd, check, inherit, receipt) = match target {
        AttachTarget::Existing(stored) => {
            let events = read_events(&stored.log_path).unwrap_or_default();
            let cwd = attach_cwd(&events);
            let check =
                crate::session::attach::check(&*host, &stored.id, cwd.as_deref(), &stored.log_path);
            (
                stored,
                AttachKind::Existing,
                events,
                cwd,
                check,
                None,
                false,
            )
        }
        AttachTarget::New {
            cwd,
            inherit,
            rollover,
        } => {
            let Ok(stored) = kit.store.create(&cwd) else {
                kit.notice(&inherit.id, &render::wording::new_session_failed());
                return Err(());
            };
            // 新建的空会话：流还没有，`check` 与 `attach_cwd` 都无从谈起。
            let receipt = !rollover;
            (
                stored,
                AttachKind::New,
                Vec::new(),
                Some(cwd),
                crate::session::AttachCheck::Ready,
                Some(inherit),
                receipt,
            )
        }
    };
    let id = stored.id.clone();
    // 装不成时跟谁说：新建那条路认前台（调用方），盘上那条路认被点的那一行。
    let recipient = inherit
        .as_ref()
        .map_or_else(|| id.clone(), |facts| facts.id.clone());

    match attach_plan(kind, host.contains(&id), &id, cwd, check) {
        // 已在宿主表：零延迟取回 —— 不装载、也不说「正在装载」。
        AttachPlan::SwitchOnly { .. } => {
            host.switch(&id);
            let session_cwd = host
                .get(&id)
                .map(|entry| entry.cwd.clone())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            console.switch_session(kit.ui_facts(&stored, &events), session_cwd, events);
            Ok(id)
        }
        AttachPlan::Refuse { reason } => {
            kit.notice(&recipient, &reason.message());
            Err(())
        }
        AttachPlan::Attach { cwd, notices, .. } => {
            for notice in &notices {
                say_notice(kit, &id, notice);
            }
            let asker = kit.session_asker(&id);
            let loaded = match kit
                .load(
                    stored.clone(),
                    cwd.clone(),
                    inherit.as_ref(),
                    Arc::clone(&asker),
                )
                .await
            {
                Ok(loaded) => loaded,
                Err(message) => {
                    kit.notice(&recipient, &message);
                    return Err(());
                }
            };
            let actor = CliActor {
                console: console.clone(),
                config: Arc::clone(&kit.config),
                goals: kit.goals.clone(),
                mcp: Arc::clone(&kit.mcp),
                prompts: Arc::new(Vec::new()),
                read_only: loaded.read_only,
                loop_running: false,
                quit: ExitRequest::default(),
                requests: requests.clone(),
            };
            // 名册与只读也随这个会话走（票 25）：装载路径与启动路径一样推一次，
            // 切到它时 `/` 菜单与页内改名的判据才有东西。
            console.catalog(
                id.clone(),
                super::interactive::slash_catalog(
                    &crate::render::wording::BUILT_IN_COMMANDS,
                    &loaded.harness.skill_catalog(),
                    &[],
                ),
            );
            let session_cwd = cwd.clone();
            if let Err(error) =
                host.adopt_with(loaded.harness, cwd, loaded.read_only, Box::new(actor))
            {
                kit.notice(&recipient, &error.to_string());
                return Err(());
            }
            // 入表之后把这个会话的三态发送端交给应答者（票 22）。
            if let Some(entry) = host.get(&id) {
                asker.attach(entry.state_tx.clone());
            }
            host.switch(&id);
            console.switch_session(kit.ui_facts(&stored, &events), session_cwd, events);
            // 装完要说的话（票 18 的缺格、票 21 的新开回执）只在这里说。
            if !loaded.filled.is_empty() {
                say_notice(kit, &id, &Notice::FactsFilled(loaded.filled.clone()));
            }
            if receipt {
                say_notice(kit, &id, &Notice::NewOpened);
            }
            Ok(id)
        }
    }
}

/// 新建一个会话：同 cwd 再加载一个**空**会话，并切过去（票 21）。
///
/// 继承前台的模型 / 档位 / 权限模式；沙箱与技能按同 cwd 重探（那两样是组装期的事）；
/// `carried_tokens` 清 0 —— 新会话本来就不带。装配那一段归 [`attach`]，这里只剩一件与
/// 「新建」绑定的事：**翻页交接** —— 它要新会话入表之后的那个 `cmd`，属于「装完之后对它
/// 做的事」（票 34）。
pub(super) async fn open_new(
    host: &mut SessionHost<CliCommand>,
    kit: &AssemblyKit,
    console: &ConsoleHandle,
    requests: &tokio::sync::mpsc::UnboundedSender<CliRequest>,
    cwd: &Path,
    inherit: &crate::session::FrontFacts,
    rollover: Option<(u64, String, Option<String>)>,
) {
    let target = AttachTarget::New {
        cwd: cwd.to_path_buf(),
        inherit: inherit.clone(),
        rollover: rollover.is_some(),
    };
    let Ok(id) = attach(host, kit, console, requests, target).await else {
        return;
    };
    // **翻页交接**（票 21）：新会话接过这一趟的额度、目标与压缩摘要，然后接着跑。
    // `/clear` 不带这个载荷 —— 它是清场。
    if let Some((carry_tokens, goal, summary)) = rollover
        && let Some(entry) = host.get(&id)
    {
        let _ = entry
            .cmd
            .send(CliCommand::Rollover {
                carry_tokens,
                goal,
                summary,
            })
            .await;
    }
}

/// 一条会话的工作区：**流上**记着的那个（`SessionStarted.cwd`），读不出就是 `None`。
///
/// 装载与入表都只问这一处 —— `StoredSession.dir` 是**会话目录**（`src/session/store.rs:64`），
/// 从不是工作目录；拿它顶上会让「新建 / 翻页继承 cwd」那一格记错（票 33）。
pub(super) fn attach_cwd(events: &[Event]) -> Option<PathBuf> {
    crate::session::store::started_cwd(events)
        .filter(|cwd| !cwd.trim().is_empty())
        .map(PathBuf::from)
}

/// 这一条装载是「装盘上一条」还是「装刚建的空会话」—— 只影响装完说哪几句（票 34）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttachKind {
    /// 盘上一条旧会话：说「正在装载」。
    Existing,
    /// 刚建的空会话：不说。
    New,
}

/// 一条会话装不成的理由（票 34）。文本归措辞层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Refusal {
    /// 流里没记下它的工作区（票 33）。
    NoCwd,
    /// 那个工作区已经不在了。
    CwdGone(PathBuf),
    /// 事件流读不出。
    Unreadable(String),
}

impl Refusal {
    fn message(&self) -> String {
        match self {
            Refusal::NoCwd => render::wording::session_cwd_missing(),
            Refusal::CwdGone(dir) => render::wording::session_cwd_gone(&dir.display().to_string()),
            Refusal::Unreadable(reason) => render::wording::session_stream_unreadable(reason),
        }
    }
}

/// 装完之后要说的话 —— 结构，不带中文（票 34）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Notice {
    /// 「正在装载会话 X…」（只在盘上那一条说）。
    Loading,
    /// 旧流缺了几样会话事实，回退到了进程配置默认（票 18）。
    FactsFilled(Vec<&'static str>),
    /// 新开一个会话之后的回执（票 21）。
    NewOpened,
}

/// 一份装载计划：**纯的**，能单独断言（票 34）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AttachPlan {
    /// 已在宿主表：切前台，不装载、也不说「正在装载」。
    SwitchOnly { id: SessionId },
    /// 装它。
    Attach {
        id: SessionId,
        cwd: PathBuf,
        notices: Vec<Notice>,
    },
    /// 装不成。
    Refuse { reason: Refusal },
}

/// 装载前的判定 —— 只吃宿主表的只读结论、`attach::check` 的结果与 cwd（票 34）。
///
/// 它不吃 [`AssemblyKit`]：计划要能在单元测试里断言，「真配置、真 provider」那半边留在执行里。
pub(super) fn attach_plan(
    kind: AttachKind,
    hosted: bool,
    id: &SessionId,
    cwd: Option<PathBuf>,
    check: crate::session::AttachCheck,
) -> AttachPlan {
    // 已在宿主表（或竞态的另一半）：零延迟取回，绝不重走装载。
    if hosted || matches!(check, crate::session::AttachCheck::AlreadyHosted(_)) {
        return AttachPlan::SwitchOnly { id: id.clone() };
    }
    match check {
        crate::session::AttachCheck::MissingCwd(gone) => {
            return AttachPlan::Refuse {
                reason: Refusal::CwdGone(gone),
            };
        }
        crate::session::AttachCheck::Unreadable(reason) => {
            return AttachPlan::Refuse {
                reason: Refusal::Unreadable(reason),
            };
        }
        _ => {}
    }
    let Some(cwd) = cwd else {
        return AttachPlan::Refuse {
            reason: Refusal::NoCwd,
        };
    };
    let notices = match kind {
        AttachKind::Existing => vec![Notice::Loading],
        AttachKind::New => Vec::new(),
    };
    AttachPlan::Attach {
        id: id.clone(),
        cwd,
        notices,
    }
}

/// 点会话页上的一行：切过去（`.scratch/session-tab/spec.md` §3、票 20）。
///
/// 三条结局：**已在宿主表** —— 零延迟取回（忙闲不阻挡）；**不在** —— 先做能立即判定的检查
/// （票 18 的 `check`），通过就装载、装完切前台；**cwd 不在、流读不出、流里没记下工作区**
/// —— 各回一句，不切。找行与「找不到这条」留在这一层（票 34）；装配那一段归 [`attach`]。
pub(super) async fn open_session(
    host: &mut SessionHost<CliCommand>,
    kit: &AssemblyKit,
    console: &ConsoleHandle,
    requests: &tokio::sync::mpsc::UnboundedSender<CliRequest>,
    id: &SessionId,
) {
    let Ok(sessions) = kit.store.list_all() else {
        kit.notice(id, "读不出会话列表");
        return;
    };
    let Some(stored) = sessions.into_iter().find(|stored| &stored.id == id) else {
        kit.notice(id, &format!("找不到会话 {}", id.as_str()));
        return;
    };
    let _ = attach(host, kit, console, requests, AttachTarget::Existing(stored)).await;
}
