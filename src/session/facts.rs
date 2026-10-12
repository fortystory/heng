//! 会话级事实：五样，以及「当前是哪一份」
//! （`.scratch/session-tab/spec.md` §2、决策票 05）。
//!
//! 两条纪律：
//!
//! * **当前事实 = 流上最后一条快照**（`SessionFactsChanged`），没有快照才退回第一条
//!   `SessionStarted` 那五格。与目标、`todo`、标题同一条派生纪律 —— 只追加，不修改。
//! * **不记 `context_window` 与沙箱可用性**：前者由模型推得出来（`context::usable_input`），
//!   后者是**这一刻**的机器事实（它另有一条 `SandboxStatus`）。
//!
//! 旧流没有那五格（都是 `Option` + `serde(default)`），于是装载时回退**进程配置默认**，并由
//! [`SessionFacts::missing`] 如实报出补了哪几样。

use crate::events::{Event, EventPayload};

/// 一个会话的五样会话级事实。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionFacts {
    /// provider 调用所用的模型标识。
    pub model: Option<String>,
    /// 跑在哪个 provider profile 上。
    pub provider: Option<String>,
    /// 推理档位。`None` 是「默认」那一档。
    pub effort: Option<String>,
    /// 权限模式（`readonly` / `ask` / `auto` / `workspace`）。
    pub permission_mode: Option<String>,
    /// 沙箱档（`bwrap` / `off`）。
    pub sandbox_mode: Option<String>,
}

impl SessionFacts {
    /// 流上**最后一条**快照；一条都没有就退回第一条 `SessionStarted`。
    ///
    /// 一条流都没有这两样（比如半读的坏流）时答全 `None`。
    pub fn current(events: &[Event]) -> Self {
        if let Some(facts) = events.iter().rev().find_map(|event| match &event.payload {
            EventPayload::SessionFactsChanged {
                model,
                provider,
                effort,
                permission_mode,
                sandbox_mode,
            } => Some(Self {
                model: model.clone(),
                provider: provider.clone(),
                effort: effort.clone(),
                permission_mode: permission_mode.clone(),
                sandbox_mode: sandbox_mode.clone(),
            }),
            _ => None,
        }) {
            return facts;
        }
        events
            .iter()
            .find_map(|event| match &event.payload {
                EventPayload::SessionStarted {
                    model,
                    provider,
                    effort,
                    permission_mode,
                    sandbox_mode,
                    ..
                } => Some(Self {
                    model: model.clone(),
                    provider: provider.clone(),
                    effort: effort.clone(),
                    permission_mode: permission_mode.clone(),
                    sandbox_mode: sandbox_mode.clone(),
                }),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// 哪几格是空的 —— 「回退进程配置默认并如实说明」那句话读它。
    pub fn missing(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.model.is_none() {
            missing.push("模型");
        }
        if self.provider.is_none() {
            missing.push("provider");
        }
        if self.effort.is_none() {
            missing.push("思考档");
        }
        if self.permission_mode.is_none() {
            missing.push("权限模式");
        }
        if self.sandbox_mode.is_none() {
            missing.push("沙箱档");
        }
        missing
    }

    /// 用 `fallback`（进程配置默认）补上缺的格子，并报出补了哪几样。
    ///
    /// 补是**逐格**的：旧流可能只缺其中一两样，那时别的格子照用流上那份。
    pub fn with_fallback(mut self, fallback: &SessionFacts) -> (Self, Vec<&'static str>) {
        let mut filled = Vec::new();
        let mut take = |slot: &mut Option<String>, value: &Option<String>, name: &'static str| {
            if slot.is_none() {
                *slot = value.clone();
                // 只有**真的填上了值**才算「补上了」：默认也是空的时候，这一格本来就空着 ——
                // 那不是「回退到了进程配置默认」，而是那一档本来就没设。
                if value.is_some() {
                    filled.push(name);
                }
            }
        };
        take(&mut self.model, &fallback.model, "模型");
        take(&mut self.provider, &fallback.provider, "provider");
        take(&mut self.effort, &fallback.effort, "思考档");
        take(
            &mut self.permission_mode,
            &fallback.permission_mode,
            "权限模式",
        );
        take(&mut self.sandbox_mode, &fallback.sandbox_mode, "沙箱档");
        (self, filled)
    }
}
