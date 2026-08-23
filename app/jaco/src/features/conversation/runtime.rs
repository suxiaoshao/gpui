mod approval;

use std::{collections::HashMap, sync::Arc};

use gpui::{App, AppContext, AsyncApp, Context, Entity, EventEmitter, Task, WeakEntity};
use gpui_operation::{Cancel, Complete, Load, Retry, Transition, refresh};
use jaco_agent::{
    AgentCancellationToken, AgentPersistence, AgentRunHandle, AgentRunRequest, AgentRuntime,
    AgentRuntimeObserver, OpenAiResponsesSessionPool, ToolApprovalDecision,
};
use jaco_core::{AgentRunId, ConversationId, ProjectId, ToolInvocationId};
use jaco_db::ProviderRecord;
use smol::channel::{Receiver, Sender};
use tracing::{Level, event};

use self::approval::ConversationApprovalBroker;
use crate::{database, errors::JacoResult, state::providers::secrets::ProviderSecretStore};

enum RuntimePublication {
    Event(jaco_agent::AgentRuntimeEvent),
    ToolApprovalAvailabilityChanged {
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    },
    Drain(Sender<()>),
}

pub(crate) struct ConversationRuntimeStore {
    active_runs: ActiveRuns,
    archive_fences: ArchiveFences,
    last_errors: HashMap<ConversationId, String>,
    next_run_key: u64,
    shutting_down: bool,
    openai_sessions: OpenAiResponsesSessionPool,
    recovery: refresh::Operation<(), ConversationRuntimeProblem, Task<()>>,
}

#[derive(Debug)]
pub(crate) struct ConversationRuntimeProblem(String);

impl std::fmt::Display for ConversationRuntimeProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConversationRuntimeProblem {}

#[derive(Default)]
struct ActiveRuns(HashMap<ConversationId, ConversationAttempt>);

#[cfg(test)]
impl ActiveRuns {
    fn insert(&mut self, conversation_id: ConversationId, active: ActiveRun) {
        self.0
            .insert(conversation_id, ConversationAttempt::Running(active));
    }

    fn contains_key(&self, conversation_id: &ConversationId) -> bool {
        self.0.contains_key(conversation_id)
    }
}

enum ConversationAttempt {
    Submitting(SubmissionAttempt),
    Running(ActiveRun),
    Stopping(ActiveRun),
}

struct SubmissionAttempt {
    key: ActiveRunKey,
    project_id: Option<ProjectId>,
    task: Task<()>,
}

struct ActiveRun {
    key: ActiveRunKey,
    project_id: Option<ProjectId>,
    agent_run_id: Option<AgentRunId>,
    cancellation_token: AgentCancellationToken,
    approval_broker: Arc<ConversationApprovalBroker>,
    task: Task<()>,
    _event_task: Task<()>,
}

struct SubmitAttempt {
    conversation_id: ConversationId,
    key: ActiveRunKey,
    project_id: Option<ProjectId>,
    task: Task<()>,
}

struct SubmissionCommitted {
    conversation_id: ConversationId,
    key: ActiveRunKey,
}

struct StartRun {
    conversation_id: ConversationId,
    key: ActiveRunKey,
    active_run: ActiveRun,
}

struct SubmissionFailed {
    conversation_id: ConversationId,
    key: ActiveRunKey,
}

struct StopRun<'cx, 'app> {
    conversation_id: ConversationId,
    key: ActiveRunKey,
    persistence: Result<Arc<dyn AgentPersistence>, String>,
    openai_sessions: OpenAiResponsesSessionPool,
    cx: &'cx mut Context<'app, ConversationRuntimeStore>,
}

struct RunFinished {
    conversation_id: ConversationId,
    key: ActiveRunKey,
}

struct StopFinished {
    conversation_id: ConversationId,
    key: ActiveRunKey,
}

#[derive(Default)]
struct ArchiveFences {
    next_key: u64,
    conversations: HashMap<ConversationId, ConversationArchiveFence>,
    projects: HashMap<ProjectId, ArchiveFenceKey>,
}

struct ConversationArchiveFence {
    project_id: ProjectId,
    key: ArchiveFenceKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ArchiveFenceKey(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
enum ArchiveFenceTarget {
    Conversation {
        conversation_id: ConversationId,
        project_id: ProjectId,
    },
    Project(ProjectId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArchiveFenceTicket {
    key: ArchiveFenceKey,
    target: ArchiveFenceTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConversationRunStatus {
    Idle,
    Submitting,
    Running,
    Stopping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConversationSubmissionKind {
    Create,
    Message,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConversationSubmissionTicket {
    conversation_id: ConversationId,
    attempt_key: ActiveRunKey,
}

impl ConversationSubmissionTicket {
    pub(crate) fn conversation_id(&self) -> &ConversationId {
        &self.conversation_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConversationSubmissionError {
    Busy,
    Unavailable(String),
}

impl std::fmt::Display for ConversationSubmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => formatter.write_str("conversation already has an active attempt"),
            Self::Unavailable(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ConversationSubmissionError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ActiveRunKey(u64);

impl ConversationAttempt {
    fn project_id(&self) -> Option<&ProjectId> {
        match self {
            Self::Submitting(attempt) => attempt.project_id.as_ref(),
            Self::Running(attempt) | Self::Stopping(attempt) => attempt.project_id.as_ref(),
        }
    }
}

impl ArchiveFences {
    fn reserve_conversation(
        &mut self,
        conversation_id: ConversationId,
        project_id: ProjectId,
    ) -> jaco_db::Result<ArchiveFenceTicket> {
        if self.conversations.contains_key(&conversation_id) {
            return Err(jaco_db::DbError::Invariant(format!(
                "conversation {conversation_id} already has an archive fence"
            )));
        }
        if self.projects.contains_key(&project_id) {
            return Err(jaco_db::DbError::Invariant(format!(
                "project {project_id} already has an archive fence"
            )));
        }

        let key = self.next_key();
        self.conversations.insert(
            conversation_id.clone(),
            ConversationArchiveFence {
                project_id: project_id.clone(),
                key,
            },
        );
        Ok(ArchiveFenceTicket {
            key,
            target: ArchiveFenceTarget::Conversation {
                conversation_id,
                project_id,
            },
        })
    }

    fn reserve_project(&mut self, project_id: ProjectId) -> jaco_db::Result<ArchiveFenceTicket> {
        if self.projects.contains_key(&project_id)
            || self
                .conversations
                .values()
                .any(|fence| fence.project_id == project_id)
        {
            return Err(jaco_db::DbError::Invariant(format!(
                "project {project_id} already overlaps an archive fence"
            )));
        }

        let key = self.next_key();
        self.projects.insert(project_id.clone(), key);
        Ok(ArchiveFenceTicket {
            key,
            target: ArchiveFenceTarget::Project(project_id),
        })
    }

    fn blocks(&self, conversation_id: &ConversationId, project_id: Option<&ProjectId>) -> bool {
        self.conversations.contains_key(conversation_id)
            || project_id.is_some_and(|project_id| self.projects.contains_key(project_id))
    }

    fn owns(&self, ticket: &ArchiveFenceTicket) -> bool {
        match &ticket.target {
            ArchiveFenceTarget::Conversation {
                conversation_id,
                project_id,
            } => self
                .conversations
                .get(conversation_id)
                .is_some_and(|fence| fence.key == ticket.key && &fence.project_id == project_id),
            ArchiveFenceTarget::Project(project_id) => {
                self.projects.get(project_id) == Some(&ticket.key)
            }
        }
    }

    fn release(&mut self, ticket: &ArchiveFenceTicket) -> bool {
        if !self.owns(ticket) {
            return false;
        }
        match &ticket.target {
            ArchiveFenceTarget::Conversation {
                conversation_id, ..
            } => {
                self.conversations.remove(conversation_id);
            }
            ArchiveFenceTarget::Project(project_id) => {
                self.projects.remove(project_id);
            }
        }
        true
    }

    fn next_key(&mut self) -> ArchiveFenceKey {
        let key = ArchiveFenceKey(self.next_key);
        self.next_key = self.next_key.wrapping_add(1);
        key
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ConversationRuntimeEvent {
    SubmissionCommitted {
        ticket: ConversationSubmissionTicket,
        kind: ConversationSubmissionKind,
    },
    SubmissionFailed {
        ticket: ConversationSubmissionTicket,
        kind: ConversationSubmissionKind,
        error: String,
    },
    RunLaunchFailed {
        ticket: ConversationSubmissionTicket,
        error: String,
    },
    RunStarted {
        ticket: ConversationSubmissionTicket,
    },
    RunFinished {
        ticket: ConversationSubmissionTicket,
    },
    ToolApprovalAvailabilityChanged {
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_id: ToolInvocationId,
    },
}

impl Transition<SubmitAttempt> for &mut ActiveRuns {
    type Output = bool;

    fn transition(self, message: SubmitAttempt) -> Self::Output {
        use std::collections::hash_map::Entry;

        match self.0.entry(message.conversation_id) {
            Entry::Vacant(entry) => {
                entry.insert(ConversationAttempt::Submitting(SubmissionAttempt {
                    key: message.key,
                    project_id: message.project_id,
                    task: message.task,
                }));
                true
            }
            Entry::Occupied(entry) => {
                event!(
                    Level::DEBUG,
                    conversation_id = %entry.key(),
                    "ignored duplicate conversation submission"
                );
                false
            }
        }
    }
}

impl Transition<SubmissionCommitted> for &mut ActiveRuns {
    type Output = bool;

    fn transition(self, message: SubmissionCommitted) -> Self::Output {
        matches!(
            self.0.get(&message.conversation_id),
            Some(ConversationAttempt::Submitting(submitting)) if submitting.key == message.key
        )
    }
}

impl Transition<StartRun> for &mut ActiveRuns {
    type Output = bool;

    fn transition(self, message: StartRun) -> Self::Output {
        let Some(current) = self.0.get_mut(&message.conversation_id) else {
            return false;
        };
        if !matches!(current, ConversationAttempt::Submitting(submitting) if submitting.key == message.key)
        {
            return false;
        }

        let previous = std::mem::replace(current, ConversationAttempt::Running(message.active_run));
        drop(previous);
        true
    }
}

impl Transition<SubmissionFailed> for &mut ActiveRuns {
    type Output = bool;

    fn transition(self, message: SubmissionFailed) -> Self::Output {
        let matches = matches!(
            self.0.get(&message.conversation_id),
            Some(ConversationAttempt::Submitting(submitting)) if submitting.key == message.key
        );
        if !matches {
            return false;
        }
        drop(self.0.remove(&message.conversation_id));
        true
    }
}

impl Transition<StopRun<'_, '_>> for &mut ActiveRuns {
    type Output = bool;

    fn transition(self, message: StopRun<'_, '_>) -> Self::Output {
        let StopRun {
            conversation_id,
            key: expected_key,
            persistence,
            openai_sessions,
            cx,
        } = message;
        let matches = matches!(
            self.0.get(&conversation_id),
            Some(ConversationAttempt::Running(active)) if active.key == expected_key
        );
        if !matches {
            return false;
        }

        let Some(ConversationAttempt::Running(active)) = self.0.remove(&conversation_id) else {
            unreachable!("running attempt was validated before removal");
        };
        active.cancellation_token.cancel();
        if let Some(agent_run_id) = active.agent_run_id.as_ref() {
            active
                .approval_broker
                .cancel_all_for_run(&conversation_id, agent_run_id);
        } else {
            active.approval_broker.cancel_all();
        }

        let ActiveRun {
            key,
            project_id,
            agent_run_id,
            cancellation_token,
            approval_broker,
            task: running_task,
            _event_task,
        } = active;
        let cleanup_conversation_id = conversation_id.clone();
        let cleanup_agent_run_id = agent_run_id.clone();
        let completion_sessions = openai_sessions.clone();
        let cleanup = cx.spawn(async move |store, cx| {
            running_task.await;
            completion_sessions
                .close_conversation(&cleanup_conversation_id)
                .await;
            let result = match persistence {
                Ok(persistence) => AgentRuntime::new(persistence)
                    .with_openai_session_pool(completion_sessions.clone())
                    .cancel_non_terminal_runs_for_conversation(&cleanup_conversation_id, None)
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error),
            };
            finish_stop_from_task(
                store,
                cleanup_conversation_id,
                key,
                cleanup_agent_run_id,
                result,
                cx,
            );
        });
        self.0.insert(
            conversation_id,
            ConversationAttempt::Stopping(ActiveRun {
                key,
                project_id,
                agent_run_id,
                cancellation_token,
                approval_broker,
                task: cleanup,
                _event_task,
            }),
        );
        true
    }
}

impl Transition<RunFinished> for &mut ActiveRuns {
    type Output = bool;

    fn transition(self, message: RunFinished) -> Self::Output {
        let matches = matches!(
            self.0.get(&message.conversation_id),
            Some(ConversationAttempt::Running(active)) if active.key == message.key
        );
        if !matches {
            return false;
        }
        drop(self.0.remove(&message.conversation_id));
        true
    }
}

impl Transition<StopFinished> for &mut ActiveRuns {
    type Output = bool;

    fn transition(self, message: StopFinished) -> Self::Output {
        let matches = matches!(
            self.0.get(&message.conversation_id),
            Some(ConversationAttempt::Stopping(active)) if active.key == message.key
        );
        if !matches {
            return false;
        }
        drop(self.0.remove(&message.conversation_id));
        true
    }
}

impl EventEmitter<ConversationRuntimeEvent> for ConversationRuntimeStore {}

impl ConversationRuntimeStore {
    fn new() -> Self {
        Self {
            active_runs: ActiveRuns::default(),
            archive_fences: ArchiveFences::default(),
            last_errors: HashMap::new(),
            next_run_key: 0,
            shutting_down: false,
            openai_sessions: OpenAiResponsesSessionPool::new(),
            recovery: refresh::Operation::new(),
        }
    }

    #[cfg(test)]
    fn new_ready_for_test() -> Self {
        let mut runtime = Self::new();
        runtime.recovery.transition(Load(Task::ready(())));
        runtime.recovery.transition(Complete(Ok(())));
        runtime
    }

    #[cfg(test)]
    pub(crate) fn install_tool_approval_authorities_for_test(
        &mut self,
        conversation_id: ConversationId,
        agent_run_id: AgentRunId,
        tool_invocation_ids: &[ToolInvocationId],
    ) -> ConversationSubmissionTicket {
        let key = self.next_active_run_key();
        let (publications, _publication_receiver) = smol::channel::unbounded();
        let approval_broker = Arc::new(ConversationApprovalBroker::new(publications));
        for tool_invocation_id in tool_invocation_ids {
            let _receiver = approval_broker.register_pending_for_test(
                conversation_id.clone(),
                agent_run_id.clone(),
                tool_invocation_id.clone(),
            );
        }
        self.active_runs.insert(
            conversation_id.clone(),
            ActiveRun {
                key,
                project_id: None,
                agent_run_id: Some(agent_run_id),
                cancellation_token: AgentCancellationToken::new(),
                approval_broker,
                task: Task::ready(()),
                _event_task: Task::ready(()),
            },
        );
        ConversationSubmissionTicket {
            conversation_id,
            attempt_key: key,
        }
    }

    #[cfg(test)]
    pub(crate) fn finish_run_event_for_test(
        &mut self,
        ticket: ConversationSubmissionTicket,
    ) -> ConversationRuntimeEvent {
        assert!((&mut self.active_runs).transition(RunFinished {
            conversation_id: ticket.conversation_id.clone(),
            key: ticket.attempt_key,
        }));
        ConversationRuntimeEvent::RunFinished { ticket }
    }

    pub(crate) fn run_status(&self, conversation_id: &ConversationId) -> ConversationRunStatus {
        match self.active_runs.0.get(conversation_id) {
            None => ConversationRunStatus::Idle,
            Some(ConversationAttempt::Submitting(_)) => ConversationRunStatus::Submitting,
            Some(ConversationAttempt::Running(_)) => ConversationRunStatus::Running,
            Some(ConversationAttempt::Stopping(_)) => ConversationRunStatus::Stopping,
        }
    }

    pub(crate) fn reserve_conversation_archive(
        &mut self,
        conversation_id: ConversationId,
        project_id: ProjectId,
    ) -> jaco_db::Result<ArchiveFenceTicket> {
        if self.active_runs.0.contains_key(&conversation_id) {
            return Err(jaco_db::DbError::ConversationHasActiveRun { conversation_id });
        }
        self.archive_fences
            .reserve_conversation(conversation_id, project_id)
    }

    pub(crate) fn reserve_project_archive(
        &mut self,
        project_id: ProjectId,
    ) -> jaco_db::Result<ArchiveFenceTicket> {
        let mut active_conversation_ids = self
            .active_runs
            .0
            .iter()
            .filter(|(_, attempt)| attempt.project_id() == Some(&project_id))
            .map(|(conversation_id, _)| conversation_id.clone())
            .collect::<Vec<_>>();
        active_conversation_ids.sort();
        if let Some(conversation_id) = active_conversation_ids.into_iter().next() {
            return Err(jaco_db::DbError::ConversationHasActiveRun { conversation_id });
        }
        self.archive_fences.reserve_project(project_id)
    }

    pub(crate) fn prepare_archive_commit(
        &mut self,
        ticket: &ArchiveFenceTicket,
        conversation_ids: &[ConversationId],
        cx: &mut Context<Self>,
    ) -> Task<()> {
        if !self.archive_fences.owns(ticket) {
            event!(
                Level::ERROR,
                ?ticket,
                "archive commit does not own its runtime fence"
            );
            return Task::ready(());
        }

        let mut tasks = Vec::new();
        for conversation_id in conversation_ids {
            let Some(attempt) = self.active_runs.0.remove(conversation_id) else {
                self.last_errors.remove(conversation_id);
                continue;
            };
            match attempt {
                ConversationAttempt::Submitting(submitting) => {
                    drop(submitting.task);
                }
                ConversationAttempt::Running(active) | ConversationAttempt::Stopping(active) => {
                    active.cancellation_token.cancel();
                    if let Some(agent_run_id) = active.agent_run_id.as_ref() {
                        active
                            .approval_broker
                            .cancel_all_for_run(conversation_id, agent_run_id);
                    } else {
                        active.approval_broker.cancel_all();
                    }
                    super::registry::release_active(conversation_id, cx);
                    cx.emit(ConversationRuntimeEvent::RunFinished {
                        ticket: ConversationSubmissionTicket {
                            conversation_id: conversation_id.clone(),
                            attempt_key: active.key,
                        },
                    });
                    tasks.push((active.task, active._event_task));
                }
            }
            self.last_errors.remove(conversation_id);
        }
        cx.notify();

        let sessions = self.openai_sessions.clone();
        let conversation_ids = conversation_ids.to_vec();
        cx.spawn(async move |_, _| {
            for (task, event_task) in tasks {
                task.await;
                event_task.await;
            }
            for conversation_id in conversation_ids {
                sessions.close_conversation(&conversation_id).await;
            }
        })
    }

    pub(crate) fn release_archive_fence(&mut self, ticket: &ArchiveFenceTicket) -> bool {
        self.archive_fences.release(ticket)
    }

    #[cfg(test)]
    fn is_running(&self, conversation_id: &ConversationId) -> bool {
        self.run_status(conversation_id) != ConversationRunStatus::Idle
    }

    pub(crate) fn active_agent_run_id(
        &self,
        conversation_id: &ConversationId,
    ) -> Option<AgentRunId> {
        self.active_runs
            .0
            .get(conversation_id)
            .and_then(|attempt| match attempt {
                ConversationAttempt::Running(active) | ConversationAttempt::Stopping(active) => {
                    active.agent_run_id.clone()
                }
                ConversationAttempt::Submitting(_) => None,
            })
    }

    pub(crate) fn recovery(&self) -> &refresh::Operation<(), ConversationRuntimeProblem, Task<()>> {
        &self.recovery
    }

    pub(crate) fn take_last_error(&mut self, conversation_id: &ConversationId) -> Option<String> {
        self.last_errors.remove(conversation_id)
    }

    pub(crate) fn submit_message(
        &mut self,
        request: super::SendConversationMessageRequest,
        cx: &mut Context<Self>,
    ) -> Result<ConversationSubmissionTicket, ConversationSubmissionError> {
        let conversation_id = request.conversation_id.clone();
        let project_id = request.project_id.clone();
        self.ensure_submission_available(&conversation_id, Some(&project_id))?;
        self.last_errors.remove(&conversation_id);
        let key = self.next_active_run_key();
        let ticket = ConversationSubmissionTicket {
            conversation_id: conversation_id.clone(),
            attempt_key: key,
        };
        let submission = super::send_conversation_message(request, cx);
        let completion_ticket = ticket.clone();
        let task = cx.spawn(async move |store, cx| {
            let result = submission
                .await
                .map(|sent| sent.run_request)
                .map_err(|error| error.to_string());
            finish_submission_from_task(
                store,
                completion_ticket,
                ConversationSubmissionKind::Message,
                result,
                cx,
            );
        });
        let accepted = (&mut self.active_runs).transition(SubmitAttempt {
            conversation_id,
            key,
            project_id: Some(project_id),
            task,
        });
        debug_assert!(
            accepted,
            "submission availability was checked synchronously"
        );
        cx.notify();
        Ok(ticket)
    }

    pub(crate) fn submit_new_conversation(
        &mut self,
        request: super::CreateConversationRequest,
        cx: &mut Context<Self>,
    ) -> Result<ConversationSubmissionTicket, ConversationSubmissionError> {
        let conversation_id = request.conversation_id.clone();
        let project_id = request.project_id.clone();
        self.ensure_submission_available(&conversation_id, project_id.as_ref())?;
        self.last_errors.remove(&conversation_id);
        let key = self.next_active_run_key();
        let ticket = ConversationSubmissionTicket {
            conversation_id: conversation_id.clone(),
            attempt_key: key,
        };
        let submission = super::create_conversation(request, cx);
        let completion_ticket = ticket.clone();
        let task = cx.spawn(async move |store, cx| {
            let result = submission
                .await
                .map(|created| created.run_request)
                .map_err(|error| error.to_string());
            finish_submission_from_task(
                store,
                completion_ticket,
                ConversationSubmissionKind::Create,
                result,
                cx,
            );
        });
        let accepted = (&mut self.active_runs).transition(SubmitAttempt {
            conversation_id,
            key,
            project_id,
            task,
        });
        debug_assert!(
            accepted,
            "submission availability was checked synchronously"
        );
        cx.notify();
        Ok(ticket)
    }

    fn ensure_submission_available(
        &self,
        conversation_id: &ConversationId,
        project_id: Option<&ProjectId>,
    ) -> Result<(), ConversationSubmissionError> {
        if self.active_runs.0.contains_key(conversation_id)
            || self.archive_fences.blocks(conversation_id, project_id)
        {
            return Err(ConversationSubmissionError::Busy);
        }
        if self.shutting_down || !matches!(self.recovery, refresh::Operation::Ready(_)) {
            return Err(ConversationSubmissionError::Unavailable(
                self.recovery
                    .problem()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "conversation runtime is recovering".to_string()),
            ));
        }
        Ok(())
    }

    pub(crate) fn stop_run(
        &mut self,
        conversation_id: &ConversationId,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(ConversationAttempt::Running(active)) = self.active_runs.0.get(conversation_id)
        else {
            return false;
        };
        let run_key = active.key;
        let persistence = database::ready_agent_persistence(cx).map_err(|error| error.to_string());
        let openai_sessions = self.openai_sessions.clone();
        let stopped = (&mut self.active_runs).transition(StopRun {
            conversation_id: conversation_id.clone(),
            key: run_key,
            persistence,
            openai_sessions,
            cx,
        });
        if !stopped {
            return false;
        }

        self.last_errors.remove(conversation_id);
        cx.notify();
        true
    }

    fn prepare_active_run(
        &mut self,
        request: AgentRunRequest,
        project_id: Option<ProjectId>,
        run_key: ActiveRunKey,
        cx: &mut Context<Self>,
    ) -> Result<ActiveRun, String> {
        let conversation_id = request.conversation_id.clone();
        let persistence = match database::ready_agent_persistence(cx) {
            Ok(persistence) => persistence,
            Err(error) => return Err(error.to_string()),
        };
        let provider = match crate::state::providers::ready_provider(&request.provider_id, cx) {
            Ok(provider) => provider,
            Err(error) => return Err(error.to_string()),
        };
        let (tx, rx) = smol::channel::unbounded();
        let event_task = self.spawn_event_listener(rx, run_key, cx);
        let approval_broker = Arc::new(ConversationApprovalBroker::new(tx.clone()));
        let run_conversation_id = conversation_id.clone();
        let cancellation_token = request.cancellation_token.clone();
        let runtime_approval_broker = approval_broker.clone();
        let openai_sessions = self.openai_sessions.clone();
        let run_task = cx.spawn(async move |store, cx| {
            let result = run_agent_with_saved_provider(
                persistence,
                provider,
                request,
                tx.clone(),
                runtime_approval_broker,
                openai_sessions,
                cx,
            )
            .await;
            if let Err(error) = drain_runtime_publications(&tx).await {
                event!(
                    Level::ERROR,
                    %error,
                    conversation_id = %run_conversation_id,
                    "drain conversation runtime publications failed"
                );
            }
            finish_run_from_task(store, run_conversation_id, run_key, result, cx);
        });

        Ok(ActiveRun {
            key: run_key,
            project_id,
            agent_run_id: None,
            cancellation_token,
            approval_broker,
            task: run_task,
            _event_task: event_task,
        })
    }

    pub(crate) fn shutdown_all(&mut self, cx: &mut Context<Self>) -> Task<()> {
        if self.shutting_down {
            return Task::ready(());
        }
        self.shutting_down = true;
        if self.recovery.is_running() {
            self.recovery.transition(Cancel);
        }

        let ActiveRuns(active_runs) = std::mem::take(&mut self.active_runs);
        let mut tasks = Vec::with_capacity(active_runs.len());
        for (conversation_id, attempt) in active_runs {
            match attempt {
                ConversationAttempt::Submitting(submitting) => {
                    drop(submitting.task);
                }
                ConversationAttempt::Running(active) | ConversationAttempt::Stopping(active) => {
                    active.cancellation_token.cancel();
                    if let Some(agent_run_id) = active.agent_run_id.as_ref() {
                        active
                            .approval_broker
                            .cancel_all_for_run(&conversation_id, agent_run_id);
                    } else {
                        active.approval_broker.cancel_all();
                    }
                    self.last_errors.remove(&conversation_id);
                    cx.emit(ConversationRuntimeEvent::RunFinished {
                        ticket: ConversationSubmissionTicket {
                            conversation_id,
                            attempt_key: active.key,
                        },
                    });
                    tasks.push((active.task, active._event_task));
                }
            }
        }
        cx.notify();

        let openai_sessions = self.openai_sessions.clone();
        cx.spawn(async move |_, _| {
            for (task, event_task) in tasks {
                task.await;
                event_task.await;
            }
            openai_sessions.close_all().await;
        })
    }

    pub(crate) fn approve_tool_invocation(
        &mut self,
        conversation_id: ConversationId,
        tool_invocation_id: ToolInvocationId,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.shutting_down || !matches!(self.recovery, refresh::Operation::Ready(_)) {
            return false;
        }
        let Some(ConversationAttempt::Running(active)) = self.active_runs.0.get(&conversation_id)
        else {
            return false;
        };
        let Some(agent_run_id) = active.agent_run_id.as_ref() else {
            return false;
        };
        let Some(outcome) = active.approval_broker.resolve_for(
            &conversation_id,
            agent_run_id,
            &tool_invocation_id,
            ToolApprovalDecision::Approved {
                decided_by: "user".to_string(),
                reason: None,
            },
        ) else {
            return false;
        };
        debug_assert_eq!(outcome.conversation_id, conversation_id);
        debug_assert_eq!(&outcome.agent_run_id, agent_run_id);
        let _ = outcome.remaining_for_run;
        self.last_errors.remove(&conversation_id);
        cx.notify();
        true
    }

    pub(crate) fn deny_tool_invocation(
        &mut self,
        conversation_id: ConversationId,
        tool_invocation_id: ToolInvocationId,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.shutting_down || !matches!(self.recovery, refresh::Operation::Ready(_)) {
            return false;
        }
        let Some(ConversationAttempt::Running(active)) = self.active_runs.0.get(&conversation_id)
        else {
            return false;
        };
        let Some(agent_run_id) = active.agent_run_id.as_ref() else {
            return false;
        };
        let Some(outcome) = active.approval_broker.resolve_for(
            &conversation_id,
            agent_run_id,
            &tool_invocation_id,
            ToolApprovalDecision::Denied {
                decided_by: "user".to_string(),
                reason: None,
            },
        ) else {
            return false;
        };
        debug_assert_eq!(outcome.conversation_id, conversation_id);
        debug_assert_eq!(&outcome.agent_run_id, agent_run_id);
        let _ = outcome.remaining_for_run;
        self.last_errors.remove(&conversation_id);
        cx.notify();
        true
    }

    pub(crate) fn can_decide_tool_invocation(
        &self,
        conversation_id: &ConversationId,
        agent_run_id: &AgentRunId,
        tool_invocation_id: &ToolInvocationId,
    ) -> bool {
        if self.shutting_down || !matches!(self.recovery, refresh::Operation::Ready(_)) {
            return false;
        }
        let Some(ConversationAttempt::Running(active)) = self.active_runs.0.get(conversation_id)
        else {
            return false;
        };
        if active.agent_run_id.as_ref() != Some(agent_run_id) {
            return false;
        }
        active
            .approval_broker
            .is_pending_for(conversation_id, agent_run_id, tool_invocation_id)
    }

    fn next_active_run_key(&mut self) -> ActiveRunKey {
        let key = ActiveRunKey(self.next_run_key);
        self.next_run_key = self.next_run_key.wrapping_add(1);
        key
    }

    fn spawn_event_listener(
        &self,
        rx: Receiver<RuntimePublication>,
        run_key: ActiveRunKey,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            while let Ok(publication) = rx.recv().await {
                match publication {
                    RuntimePublication::Event(runtime_event) => {
                        let Some(this) = this.upgrade() else {
                            break;
                        };
                        this.update(cx, |store, cx| {
                            store.handle_runtime_event(run_key, runtime_event, cx);
                        });
                    }
                    RuntimePublication::ToolApprovalAvailabilityChanged {
                        conversation_id,
                        agent_run_id,
                        tool_invocation_id,
                    } => {
                        let Some(this) = this.upgrade() else {
                            break;
                        };
                        this.update(cx, |store, cx| {
                            if !store.accepts_runtime_publication(&conversation_id, run_key) {
                                return;
                            }
                            cx.emit(ConversationRuntimeEvent::ToolApprovalAvailabilityChanged {
                                conversation_id,
                                agent_run_id,
                                tool_invocation_id,
                            });
                            cx.notify();
                        });
                    }
                    RuntimePublication::Drain(acknowledgement) => {
                        let _ = acknowledgement.send(()).await;
                        break;
                    }
                }
            }
        })
    }

    fn handle_runtime_event(
        &mut self,
        run_key: ActiveRunKey,
        runtime_event: jaco_agent::AgentRuntimeEvent,
        cx: &mut Context<Self>,
    ) {
        match runtime_event {
            jaco_agent::AgentRuntimeEvent::ConversationCommitted {
                conversation,
                changes,
            } => {
                let conversation = *conversation;
                let conversation_id = conversation.id.clone();
                if !self.accepts_runtime_publication(&conversation_id, run_key) {
                    return;
                }
                super::registry::publish_changes(
                    conversation_id.clone(),
                    Some(conversation.clone()),
                    changes.clone(),
                    cx,
                );
            }
            jaco_agent::AgentRuntimeEvent::ConversationTimelineChanged {
                conversation_id,
                changes,
            } => {
                if !self.accepts_runtime_publication(&conversation_id, run_key) {
                    return;
                }
                super::registry::publish_changes(
                    conversation_id.clone(),
                    None,
                    changes.clone(),
                    cx,
                );
            }
            jaco_agent::AgentRuntimeEvent::AgentRunStarted {
                agent_run_id,
                conversation_id,
            } => {
                let Some(ConversationAttempt::Running(active)) =
                    self.active_runs.0.get_mut(&conversation_id)
                else {
                    return;
                };
                if active.key != run_key {
                    return;
                }
                active.agent_run_id = Some(agent_run_id);
            }
            jaco_agent::AgentRuntimeEvent::AgentRunStatusChanged {
                agent_run_id: _,
                status: _,
            } => {}
        }
        cx.notify();
    }

    fn accepts_runtime_publication(
        &self,
        conversation_id: &ConversationId,
        run_key: ActiveRunKey,
    ) -> bool {
        let project_id = self
            .active_runs
            .0
            .get(conversation_id)
            .and_then(ConversationAttempt::project_id);
        if self.archive_fences.blocks(conversation_id, project_id) {
            return false;
        }
        matches!(
            self.active_runs.0.get(conversation_id),
            Some(ConversationAttempt::Running(active) | ConversationAttempt::Stopping(active))
                if active.key == run_key
        )
    }

    fn finish_submission(
        &mut self,
        ticket: ConversationSubmissionTicket,
        kind: ConversationSubmissionKind,
        result: Result<AgentRunRequest, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let conversation_id = ticket.conversation_id.clone();
        let key = ticket.attempt_key;
        match result {
            Err(error) => {
                if !(&mut self.active_runs).transition(SubmissionFailed {
                    conversation_id,
                    key,
                }) {
                    return false;
                }
                cx.emit(ConversationRuntimeEvent::SubmissionFailed {
                    ticket,
                    kind,
                    error,
                });
            }
            Ok(request) => {
                let project_id = self
                    .active_runs
                    .0
                    .get(&conversation_id)
                    .and_then(ConversationAttempt::project_id)
                    .cloned();
                if !(&mut self.active_runs).transition(SubmissionCommitted {
                    conversation_id: conversation_id.clone(),
                    key,
                }) {
                    return false;
                }
                cx.emit(ConversationRuntimeEvent::SubmissionCommitted {
                    ticket: ticket.clone(),
                    kind,
                });

                let launch = if self.shutting_down
                    || !matches!(self.recovery, refresh::Operation::Ready(_))
                {
                    Err(self
                        .recovery
                        .problem()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "conversation runtime is recovering".to_string()))
                } else {
                    self.prepare_active_run(request, project_id, key, cx)
                };
                match launch {
                    Ok(active_run) => {
                        let started = (&mut self.active_runs).transition(StartRun {
                            conversation_id: conversation_id.clone(),
                            key,
                            active_run,
                        });
                        if !started {
                            return false;
                        }
                        super::registry::retain_active(conversation_id.clone(), cx);
                        cx.emit(ConversationRuntimeEvent::RunStarted { ticket });
                    }
                    Err(error) => {
                        let removed = (&mut self.active_runs).transition(SubmissionFailed {
                            conversation_id,
                            key,
                        });
                        if !removed {
                            return false;
                        }
                        cx.emit(ConversationRuntimeEvent::RunLaunchFailed { ticket, error });
                    }
                }
            }
        }
        cx.notify();
        true
    }

    fn finish_run(
        &mut self,
        conversation_id: ConversationId,
        run_key: ActiveRunKey,
        result: Result<AgentRunHandle, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(ConversationAttempt::Running(active)) = self.active_runs.0.get(&conversation_id)
        else {
            return false;
        };
        if active.key != run_key {
            return false;
        }
        if let Some(agent_run_id) = active.agent_run_id.as_ref() {
            active
                .approval_broker
                .cancel_all_for_run(&conversation_id, agent_run_id);
        } else {
            active.approval_broker.cancel_all();
        }
        if !(&mut self.active_runs).transition(RunFinished {
            conversation_id: conversation_id.clone(),
            key: run_key,
        }) {
            return false;
        }

        super::registry::release_active(&conversation_id, cx);
        if let Err(err) = result {
            let sessions = self.openai_sessions.clone();
            let failed_conversation_id = conversation_id.clone();
            cx.spawn(async move |_, _| {
                sessions.close_conversation(&failed_conversation_id).await;
            })
            .detach();
            self.last_errors.insert(conversation_id.clone(), err);
        }
        cx.emit(ConversationRuntimeEvent::RunFinished {
            ticket: ConversationSubmissionTicket {
                conversation_id,
                attempt_key: run_key,
            },
        });
        cx.notify();
        true
    }

    fn finish_stop(
        &mut self,
        conversation_id: ConversationId,
        run_key: ActiveRunKey,
        agent_run_id: Option<AgentRunId>,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !(&mut self.active_runs).transition(StopFinished {
            conversation_id: conversation_id.clone(),
            key: run_key,
        }) {
            return false;
        }

        super::registry::release_active(&conversation_id, cx);
        match result {
            Ok(()) => {
                super::registry::refresh_conversation(&conversation_id, cx);
            }
            Err(error) => {
                event!(
                    Level::ERROR,
                    %error,
                    %conversation_id,
                    ?agent_run_id,
                    "cancel active conversation runs failed"
                );
                self.last_errors.insert(conversation_id.clone(), error);
            }
        }
        cx.emit(ConversationRuntimeEvent::RunFinished {
            ticket: ConversationSubmissionTicket {
                conversation_id,
                attempt_key: run_key,
            },
        });
        cx.notify();
        true
    }
}

fn finish_submission_from_task(
    store: WeakEntity<ConversationRuntimeStore>,
    ticket: ConversationSubmissionTicket,
    kind: ConversationSubmissionKind,
    result: Result<AgentRunRequest, String>,
    cx: &mut AsyncApp,
) {
    if let Err(err) = store.update(cx, |store, cx| {
        store.finish_submission(ticket, kind, result, cx);
    }) {
        event!(Level::ERROR, error = ?err, "finish conversation submission failed");
    }
}

fn finish_run_from_task(
    store: WeakEntity<ConversationRuntimeStore>,
    conversation_id: ConversationId,
    run_key: ActiveRunKey,
    result: Result<AgentRunHandle, String>,
    cx: &mut AsyncApp,
) {
    if let Err(err) = store.update(cx, |store, cx| {
        store.finish_run(conversation_id, run_key, result, cx);
    }) {
        event!(Level::ERROR, error = ?err, "finish conversation agent run failed");
    }
}

fn finish_stop_from_task(
    store: WeakEntity<ConversationRuntimeStore>,
    conversation_id: ConversationId,
    run_key: ActiveRunKey,
    agent_run_id: Option<AgentRunId>,
    result: Result<(), String>,
    cx: &mut AsyncApp,
) {
    if let Err(err) = store.update(cx, |store, cx| {
        store.finish_stop(conversation_id, run_key, agent_run_id, result, cx);
    }) {
        event!(Level::ERROR, error = ?err, "finish stopping conversation agent run failed");
    }
}

async fn drain_runtime_publications(tx: &Sender<RuntimePublication>) -> Result<(), String> {
    let (acknowledgement_tx, acknowledgement_rx) = smol::channel::bounded(1);
    tx.send(RuntimePublication::Drain(acknowledgement_tx))
        .await
        .map_err(|_| "conversation runtime publication listener is unavailable".to_string())?;
    acknowledgement_rx.recv().await.map_err(|_| {
        "conversation runtime publication listener did not acknowledge drain".to_string()
    })
}

pub(crate) fn create(cx: &mut App) -> JacoResult<Entity<ConversationRuntimeStore>> {
    let store = cx.new(|_| ConversationRuntimeStore::new());
    request_recovery(store.clone(), cx)?;
    Ok(store)
}

pub(crate) fn retry_recovery_if_needed(store: &Entity<ConversationRuntimeStore>, cx: &mut App) {
    let should_retry = {
        let runtime = store.read(cx);
        !runtime.shutting_down
            && matches!(
                runtime.recovery,
                refresh::Operation::Unavailable(_) | refresh::Operation::Degraded(_)
            )
    };
    if should_retry && let Err(error) = request_recovery(store.clone(), cx) {
        event!(
            Level::ERROR,
            ?error,
            "retry conversation runtime recovery failed"
        );
    }
}

fn request_recovery(store: Entity<ConversationRuntimeStore>, cx: &mut App) -> JacoResult<()> {
    let persistence = database::ready_agent_persistence(cx)?;
    let openai_sessions = store.read(cx).openai_sessions.clone();
    let completion_store = store.downgrade();
    let task = cx.spawn(async move |cx| {
        let result = AgentRuntime::new(persistence)
            .with_openai_session_pool(openai_sessions)
            .recover_interrupted_runs()
            .await
            .map_err(|error| ConversationRuntimeProblem(error.to_string()));
        if let Ok(recovered) = &result
            && !recovered.is_empty()
        {
            event!(
                Level::WARN,
                recovered_count = recovered.len(),
                "recovered interrupted jaco agent runs"
            );
        }
        let failed = result.is_err();
        let _ = completion_store.update(cx, |runtime, cx| {
            if runtime.recovery.is_running() {
                runtime.recovery.transition(Complete(result.map(|_| ())));
                cx.notify();
            }
        });
        if failed {
            event!(Level::ERROR, "recover interrupted jaco agent runs failed");
            cx.update(database::request_refresh);
        }
    });
    store.update(cx, |runtime, cx| {
        match runtime.recovery {
            refresh::Operation::Idle(_) => runtime.recovery.transition(Load(task)),
            refresh::Operation::Unavailable(_) | refresh::Operation::Degraded(_) => {
                runtime.recovery.transition(Retry(task));
            }
            refresh::Operation::Loading(_)
            | refresh::Operation::Ready(_)
            | refresh::Operation::Refreshing(_)
            | refresh::Operation::Retrying(_)
            | refresh::Operation::RefreshingDegraded(_) => {}
        }
        cx.notify();
    });
    Ok(())
}

async fn run_agent_with_saved_provider(
    persistence: Arc<dyn AgentPersistence>,
    provider: ProviderRecord,
    request: AgentRunRequest,
    tx: Sender<RuntimePublication>,
    approval_broker: Arc<ConversationApprovalBroker>,
    openai_sessions: OpenAiResponsesSessionPool,
    cx: &mut AsyncApp,
) -> Result<AgentRunHandle, String> {
    let observer = AgentRuntimeObserver::new(move |event| {
        if let Err(err) = tx.send_blocking(RuntimePublication::Event(event)) {
            event!(Level::ERROR, error = ?err, "send conversation runtime event failed");
        }
    });
    let runtime = AgentRuntime::new(persistence)
        .with_openai_session_pool(openai_sessions)
        .with_approval_broker(approval_broker);
    let mut request = match crate::state::mcp::prepare_run_request(request, cx).await {
        Ok(prepared) => prepared.request,
        Err(err) => {
            return gpui_tokio::Tokio::spawn(cx, async move {
                runtime
                    .record_setup_failed_run(*err.request, err.message, Some(&observer))
                    .await
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string());
        }
    };
    let agent_run = runtime
        .begin_run(&mut request, Some(observer))
        .await
        .map_err(|err| err.to_string())?;
    let secrets = match ProviderSecretStore::read_values(cx, &provider.secret_refs).await {
        Ok(secrets) => secrets,
        Err(err) => {
            return gpui_tokio::Tokio::spawn(cx, async move {
                runtime
                    .record_setup_failed_started_run(agent_run, err)
                    .await
            })
            .await
            .map_err(|err| err.to_string())?
            .map_err(|err| err.to_string());
        }
    };
    gpui_tokio::Tokio::spawn(cx, async move {
        runtime
            .run_started_with_saved_provider(agent_run, request, provider, secrets)
            .await
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database;
    use gpui::{Subscription, WindowHandle};
    use jaco_agent::AgentRunHandleStatus;
    use jaco_core::{
        AgentEngineKind, AgentRunInput, AgentRunStatus, AgentRunTriggerKind, AgentRuntimeSnapshot,
        ApprovalRequestPayload, ApprovalStatus, ContentPart, ConversationEntryPayload,
        ConversationEntryStatus, ConversationMetadata, ConversationSettingsSnapshot, ProjectKind,
        ProjectMetadata, ProviderSettingsPayload, ToolApprovalMode, ToolApprovalPolicy,
        ToolArguments, ToolExecutionPolicy, ToolInvocationInput, ToolInvocationStatus,
        ToolNameStrategy, ToolPolicySnapshot, ToolSource, TranscriptRole,
        conservative_model_capabilities,
    };
    use jaco_db::{
        FreshRepository, NewAgentRun, NewConversation, NewConversationEntry, NewProject,
        NewToolInvocation, NewToolInvocationApproval,
    };
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use tempfile::{TempDir, tempdir};

    struct RuntimeEventRecorder {
        events: Arc<Mutex<Vec<ConversationRuntimeEvent>>>,
        _subscription: Subscription,
    }

    impl RuntimeEventRecorder {
        fn new(
            store: Entity<ConversationRuntimeStore>,
            cx: &mut Context<RuntimeEventRecorder>,
        ) -> Self {
            let events = Arc::new(Mutex::new(Vec::new()));
            let observed_events = events.clone();
            let subscription = cx.subscribe(
                &store,
                move |_recorder, _store, event: &ConversationRuntimeEvent, _cx| {
                    observed_events.lock().unwrap().push(event.clone());
                },
            );
            Self {
                events,
                _subscription: subscription,
            }
        }
    }

    fn active_run(key: ActiveRunKey) -> ActiveRun {
        active_run_with_token(key, AgentCancellationToken::new())
    }

    fn active_run_with_token(
        key: ActiveRunKey,
        cancellation_token: AgentCancellationToken,
    ) -> ActiveRun {
        let (publications, _publication_receiver) = smol::channel::unbounded();
        ActiveRun {
            key,
            project_id: Some("project-1".to_string()),
            agent_run_id: Some("run-1".to_string()),
            cancellation_token,
            approval_broker: Arc::new(ConversationApprovalBroker::new(publications)),
            task: Task::ready(()),
            _event_task: Task::ready(()),
        }
    }

    fn active_run_with_agent_id(key: ActiveRunKey, agent_run_id: AgentRunId) -> ActiveRun {
        ActiveRun {
            agent_run_id: Some(agent_run_id),
            ..active_run(key)
        }
    }

    fn submission_ticket(
        conversation_id: ConversationId,
        attempt_key: ActiveRunKey,
    ) -> ConversationSubmissionTicket {
        ConversationSubmissionTicket {
            conversation_id,
            attempt_key,
        }
    }

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[gpui::test]
    fn duplicate_submit_is_ignored_and_drops_its_task(cx: &mut gpui::TestAppContext) {
        let conversation_id = "conversation-1".to_string();
        let mut active_runs = ActiveRuns::default();
        assert!((&mut active_runs).transition(SubmitAttempt {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(1),
            project_id: Some("project-1".to_string()),
            task: Task::ready(()),
        }));

        let dropped = Arc::new(AtomicBool::new(false));
        let guard = DropFlag(dropped.clone());
        let duplicate_task = cx.update(|cx| {
            cx.spawn(async move |_| {
                let _guard = guard;
                std::future::pending::<()>().await;
            })
        });
        assert!(!(&mut active_runs).transition(SubmitAttempt {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(2),
            project_id: Some("project-1".to_string()),
            task: duplicate_task,
        }));
        cx.run_until_parked();

        assert!(dropped.load(Ordering::SeqCst));
        assert!(matches!(
            active_runs.0.get(&conversation_id),
            Some(ConversationAttempt::Submitting(submitting))
                if submitting.key == ActiveRunKey(1)
        ));
    }

    #[test]
    fn different_conversations_can_submit_in_parallel() {
        let mut active_runs = ActiveRuns::default();
        for (conversation_id, key) in [
            ("conversation-1".to_string(), ActiveRunKey(1)),
            ("conversation-2".to_string(), ActiveRunKey(2)),
        ] {
            assert!((&mut active_runs).transition(SubmitAttempt {
                conversation_id,
                key,
                project_id: Some("project-1".to_string()),
                task: Task::ready(()),
            }));
        }

        assert_eq!(active_runs.0.len(), 2);
        assert!(matches!(
            active_runs.0.get("conversation-1"),
            Some(ConversationAttempt::Submitting(submitting))
                if submitting.key == ActiveRunKey(1)
        ));
        assert!(matches!(
            active_runs.0.get("conversation-2"),
            Some(ConversationAttempt::Submitting(submitting))
                if submitting.key == ActiveRunKey(2)
        ));
    }

    #[test]
    fn submitting_attempt_blocks_conversation_archive_fence() {
        let conversation_id = "conversation-1".to_string();
        let project_id = "project-1".to_string();
        let mut runtime = ConversationRuntimeStore::new_ready_for_test();
        assert!((&mut runtime.active_runs).transition(SubmitAttempt {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(1),
            project_id: Some(project_id.clone()),
            task: Task::ready(()),
        }));

        let error = runtime
            .reserve_conversation_archive(conversation_id.clone(), project_id)
            .expect_err("submitting attempt must block archive admission");

        assert!(matches!(
            error,
            jaco_db::DbError::ConversationHasActiveRun {
                conversation_id: blocked_id
            } if blocked_id == conversation_id
        ));
    }

    #[test]
    fn every_active_attempt_state_blocks_project_archive_fence() {
        let conversation_id = "conversation-1".to_string();
        let project_id = "project-1".to_string();
        let attempts = [
            ConversationAttempt::Submitting(SubmissionAttempt {
                key: ActiveRunKey(1),
                project_id: Some(project_id.clone()),
                task: Task::ready(()),
            }),
            ConversationAttempt::Running(active_run(ActiveRunKey(2))),
            ConversationAttempt::Stopping(active_run(ActiveRunKey(3))),
        ];

        for attempt in attempts {
            let mut runtime = ConversationRuntimeStore::new_ready_for_test();
            runtime
                .active_runs
                .0
                .insert(conversation_id.clone(), attempt);
            let error = runtime
                .reserve_project_archive(project_id.clone())
                .expect_err("active attempt must block project archive admission");
            assert!(matches!(
                error,
                jaco_db::DbError::ConversationHasActiveRun {
                    conversation_id: blocked_id
                } if blocked_id == conversation_id
            ));
        }
    }

    #[test]
    fn project_archive_fence_blocks_same_project_submission() {
        let project_id = "project-1".to_string();
        let mut runtime = ConversationRuntimeStore::new_ready_for_test();
        let ticket = runtime
            .reserve_project_archive(project_id.clone())
            .expect("reserve project archive fence");

        assert_eq!(
            runtime.ensure_submission_available(&"conversation-1".to_string(), Some(&project_id)),
            Err(ConversationSubmissionError::Busy)
        );
        assert!(runtime.release_archive_fence(&ticket));
    }

    #[test]
    fn project_archive_fence_allows_unrelated_project_submission() {
        let mut runtime = ConversationRuntimeStore::new_ready_for_test();
        let ticket = runtime
            .reserve_project_archive("project-1".to_string())
            .expect("reserve project archive fence");

        assert_eq!(
            runtime.ensure_submission_available(
                &"conversation-2".to_string(),
                Some(&"project-2".to_string())
            ),
            Ok(())
        );
        assert!(runtime.release_archive_fence(&ticket));
    }

    #[test]
    fn stale_archive_ticket_cannot_release_replacement_fence() {
        let project_id = "project-1".to_string();
        let mut runtime = ConversationRuntimeStore::new_ready_for_test();
        let stale = runtime
            .reserve_project_archive(project_id.clone())
            .expect("reserve initial project archive fence");
        assert!(runtime.release_archive_fence(&stale));
        let current = runtime
            .reserve_project_archive(project_id.clone())
            .expect("reserve replacement project archive fence");

        assert!(!runtime.release_archive_fence(&stale));
        assert_eq!(
            runtime.ensure_submission_available(&"conversation-1".to_string(), Some(&project_id)),
            Err(ConversationSubmissionError::Busy)
        );
        assert!(runtime.release_archive_fence(&current));
    }

    #[test]
    fn stale_submission_completion_cannot_replace_current_attempt() {
        let conversation_id = "conversation-1".to_string();
        let mut active_runs = ActiveRuns::default();
        assert!((&mut active_runs).transition(SubmitAttempt {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(3),
            project_id: Some("project-1".to_string()),
            task: Task::ready(()),
        }));

        assert!(!(&mut active_runs).transition(StartRun {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(2),
            active_run: active_run(ActiveRunKey(2)),
        }));
        assert!(!(&mut active_runs).transition(SubmissionFailed {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(2),
        }));
        assert!(matches!(
            active_runs.0.get(&conversation_id),
            Some(ConversationAttempt::Submitting(submitting))
                if submitting.key == ActiveRunKey(3)
        ));

        assert!((&mut active_runs).transition(SubmissionFailed {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(3),
        }));
        assert!(!active_runs.contains_key(&conversation_id));
    }

    #[gpui::test]
    fn submission_failure_drops_the_owned_task(cx: &mut gpui::TestAppContext) {
        let conversation_id = "conversation-1".to_string();
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = DropFlag(dropped.clone());
        let task = cx.update(|cx| {
            cx.spawn(async move |_| {
                let _guard = guard;
                std::future::pending::<()>().await;
            })
        });
        let mut active_runs = ActiveRuns::default();
        assert!((&mut active_runs).transition(SubmitAttempt {
            conversation_id: conversation_id.clone(),
            key: ActiveRunKey(4),
            project_id: Some("project-1".to_string()),
            task,
        }));

        assert!((&mut active_runs).transition(SubmissionFailed {
            conversation_id,
            key: ActiveRunKey(4),
        }));
        cx.run_until_parked();
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[gpui::test]
    fn publication_drain_acknowledges_after_queued_events_are_applied(
        cx: &mut gpui::TestAppContext,
    ) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();
        let agent_run_id = "run-final".to_string();
        let event_applied = Arc::new(AtomicBool::new(false));

        let driver = cx.update(|cx| {
            store.update(cx, |runtime, cx| {
                let (tx, rx) = smol::channel::unbounded();
                let event_task = runtime.spawn_event_listener(rx, ActiveRunKey(0), cx);
                runtime.active_runs.insert(
                    conversation_id.clone(),
                    ActiveRun {
                        agent_run_id: None,
                        _event_task: event_task,
                        ..active_run(ActiveRunKey(0))
                    },
                );
                tx.send_blocking(RuntimePublication::Event(
                    jaco_agent::AgentRuntimeEvent::AgentRunStarted {
                        agent_run_id: agent_run_id.clone(),
                        conversation_id: conversation_id.clone(),
                    },
                ))
                .expect("queue runtime event");

                let store = store.downgrade();
                let event_applied = event_applied.clone();
                let conversation_id = conversation_id.clone();
                let agent_run_id = agent_run_id.clone();
                cx.spawn(async move |_, cx| {
                    drain_runtime_publications(&tx)
                        .await
                        .expect("drain runtime publications");
                    let applied = store
                        .update(cx, |runtime, _| {
                            runtime.active_agent_run_id(&conversation_id).as_ref()
                                == Some(&agent_run_id)
                        })
                        .unwrap_or(false);
                    event_applied.store(applied, Ordering::SeqCst);
                })
            })
        });

        cx.run_until_parked();
        assert!(event_applied.load(Ordering::SeqCst));
        drop(driver);
    }

    #[gpui::test]
    fn stale_runtime_publication_cannot_mutate_a_new_attempt(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |runtime, cx| {
                runtime.active_runs.insert(
                    conversation_id.clone(),
                    ActiveRun {
                        agent_run_id: None,
                        ..active_run(ActiveRunKey(2))
                    },
                );
                runtime.handle_runtime_event(
                    ActiveRunKey(1),
                    jaco_agent::AgentRuntimeEvent::AgentRunStarted {
                        agent_run_id: "stale-run".to_string(),
                        conversation_id: conversation_id.clone(),
                    },
                    cx,
                );
                assert!(runtime.active_agent_run_id(&conversation_id).is_none());

                runtime.handle_runtime_event(
                    ActiveRunKey(2),
                    jaco_agent::AgentRuntimeEvent::AgentRunStarted {
                        agent_run_id: "current-run".to_string(),
                        conversation_id: conversation_id.clone(),
                    },
                    cx,
                );
                assert_eq!(
                    runtime.active_agent_run_id(&conversation_id).as_deref(),
                    Some("current-run")
                );
            });
        });
    }

    #[gpui::test]
    fn init_recovers_persisted_running_runs(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let (conversation_id, agent_run_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            (conversation_id, agent_run_id)
        });

        let runtime = cx.update(|cx| create(cx).expect("initialize conversation runtime"));
        cx.run_until_parked();
        cx.update(|cx| {
            let repository = test_repository(cx);
            let agent_run = repository
                .get_agent_run(&agent_run_id)
                .expect("load recovered run")
                .expect("recovered run exists");
            assert_eq!(agent_run.status, AgentRunStatus::Failed);
            assert_eq!(agent_run.error.as_ref().unwrap().code, "interrupted");
            assert!(!runtime.read(cx).is_running(&conversation_id));
        });
    }

    #[gpui::test]
    fn init_recovers_persisted_waiting_approval_runs(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        let runtime = cx.update(|cx| create(cx).expect("initialize conversation runtime"));
        cx.run_until_parked();
        cx.update(|cx| {
            let repository = test_repository(cx);
            let agent_run = repository
                .get_agent_run(&agent_run_id)
                .expect("load recovered run")
                .expect("recovered run exists");
            assert_eq!(agent_run.status, AgentRunStatus::Failed);
            assert_eq!(agent_run.error.as_ref().unwrap().code, "interrupted");
            assert!(!runtime.read(cx).is_running(&conversation_id));

            let invocation = repository
                .get_tool_invocation(&approval_id)
                .unwrap()
                .unwrap();
            assert_eq!(invocation.status, ToolInvocationStatus::Failed);
            assert_eq!(invocation.error.as_ref().unwrap().code, "interrupted");
            assert_eq!(
                invocation.approval.as_ref().map(|approval| approval.status),
                Some(ApprovalStatus::Canceled)
            );
            assert!(
                invocation
                    .approval
                    .as_ref()
                    .is_some_and(|approval| approval.decision.is_none())
            );
            assert_eq!(
                tool_result_texts(&repository, &conversation_id),
                vec!["agent run was interrupted before reaching a terminal state".to_string()]
            );
        });
    }

    #[gpui::test]
    fn stop_run_keeps_conversation_gated_until_cleanup_finishes(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let cancellation_token = AgentCancellationToken::new();
        let (conversation_id, agent_run_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            (conversation_id, agent_run_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    ActiveRun {
                        agent_run_id: Some(agent_run_id.clone()),
                        ..active_run_with_token(ActiveRunKey(0), cancellation_token.clone())
                    },
                );
                store
                    .last_errors
                    .insert(conversation_id.clone(), "runtime canceled".to_string());

                assert!(store.stop_run(&conversation_id, cx));
                assert!(!store.stop_run(&conversation_id, cx));
                assert!(!store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Err("stale run completion".to_string()),
                    cx,
                ));
                assert_eq!(
                    store.run_status(&conversation_id),
                    ConversationRunStatus::Stopping
                );
                assert!(store.is_running(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });

        assert!(cancellation_token.is_cancelled());
        cx.update(|cx| {
            assert!(recorder.read(cx).events.lock().unwrap().is_empty());
        });
        cx.run_until_parked();

        cx.update(|cx| {
            let repository = test_repository(cx);
            let run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(run.status, AgentRunStatus::Canceled);
            assert!(run.error.is_none());
            assert_eq!(
                store.read(cx).run_status(&conversation_id),
                ConversationRunStatus::Idle
            );
            let events = recorder.read(cx).events.lock().unwrap().clone();
            assert!(events.contains(&ConversationRuntimeEvent::RunFinished {
                ticket: submission_ticket(conversation_id.clone(), ActiveRunKey(0)),
            }));
        });
    }

    #[gpui::test]
    fn finish_run_records_uncanceled_error(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(0)));
                store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Err("provider failed".to_string()),
                    cx,
                );

                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("provider failed")
                );
            });
        });
    }

    #[gpui::test]
    fn finish_run_removes_matching_active_run(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();
        let agent_run = jaco_db::AgentRunRecord {
            id: "run-1".to_string(),
            conversation_id: conversation_id.clone(),
            trigger_entry_id: "user-1".to_string(),
            trigger_kind: AgentRunTriggerKind::User,
            status: AgentRunStatus::Completed,
            input: AgentRunInput {
                prompt_snapshot: None,
                provider_id: "provider".to_string(),
                model_id: "model".to_string(),
                settings_snapshot: run_settings(),
                runtime_snapshot: AgentRuntimeSnapshot {
                    engine: AgentEngineKind::Rig,
                    engine_version: "test".to_string(),
                    skill_catalog_hash: None,
                    tool_name_strategy: ToolNameStrategy::Direct,
                },
                max_steps: 8,
            },
            output: None,
            error: None,
            created_at: time::OffsetDateTime::now_utc(),
            started_at: Some(time::OffsetDateTime::now_utc()),
            completed_at: Some(time::OffsetDateTime::now_utc()),
            updated_at: time::OffsetDateTime::now_utc(),
        };

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(0)));
                assert!(store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(0),
                    Ok(AgentRunHandle {
                        agent_run,
                        output: None,
                        status: AgentRunHandleStatus::Finished,
                        events: Vec::new(),
                        steps: Vec::new(),
                    }),
                    cx
                ));
                assert!(!store.active_runs.contains_key(&conversation_id));
            });
        });
    }

    #[gpui::test]
    fn app_owned_completion_does_not_require_window(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();

        let completion = cx.update(|cx| {
            store.update(cx, |store, _cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(0)));
            });
            let store = store.downgrade();
            let conversation_id = conversation_id.clone();
            cx.spawn(async move |cx| {
                finish_run_from_task(
                    store,
                    conversation_id,
                    ActiveRunKey(0),
                    Err("provider failed".to_string()),
                    cx,
                );
            })
        });

        cx.run_until_parked();
        drop(completion);
        cx.update(|cx| {
            store.update(cx, |store, _cx| {
                assert!(!store.active_runs.contains_key(&conversation_id));
                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("provider failed")
                );
            });
        });
    }

    #[gpui::test]
    fn deny_tool_invocation_resolves_matching_pending_approval(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                let active = active_run_with_agent_id(ActiveRunKey(0), agent_run_id.clone());
                let mut receiver = active.approval_broker.register_pending_for_test(
                    conversation_id.clone(),
                    agent_run_id.clone(),
                    approval_id.clone(),
                );
                store.active_runs.insert(conversation_id.clone(), active);
                store
                    .last_errors
                    .insert(conversation_id.clone(), "previous error".to_string());

                assert!(store.can_decide_tool_invocation(
                    &conversation_id,
                    &agent_run_id,
                    &approval_id
                ));

                assert!(store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                assert!(store.active_runs.contains_key(&conversation_id));
                assert_eq!(
                    receiver.try_recv().unwrap(),
                    ToolApprovalDecision::Denied {
                        decided_by: "user".to_string(),
                        reason: None,
                    }
                );
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });
    }

    #[gpui::test]
    fn approval_availability_publication_emits_the_exact_key(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let recorder = cx.update(|cx| cx.new(|cx| RuntimeEventRecorder::new(store.clone(), cx)));
        let conversation_id = "conversation-1".to_string();
        let agent_run_id = "run-1".to_string();
        let tool_invocation_id = "invocation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |runtime, cx| {
                let (publications, publication_receiver) = smol::channel::unbounded();
                let event_task =
                    runtime.spawn_event_listener(publication_receiver, ActiveRunKey(0), cx);
                let approval_broker = Arc::new(ConversationApprovalBroker::new(publications));
                let _decision = approval_broker.register_pending_for_test(
                    conversation_id.clone(),
                    agent_run_id.clone(),
                    tool_invocation_id.clone(),
                );
                runtime.active_runs.insert(
                    conversation_id.clone(),
                    ActiveRun {
                        agent_run_id: Some(agent_run_id.clone()),
                        approval_broker,
                        _event_task: event_task,
                        ..active_run(ActiveRunKey(0))
                    },
                );
            });
        });

        cx.run_until_parked();
        cx.update(|cx| {
            assert!(recorder.read(cx).events.lock().unwrap().contains(
                &ConversationRuntimeEvent::ToolApprovalAvailabilityChanged {
                    conversation_id,
                    agent_run_id,
                    tool_invocation_id,
                }
            ));
        });
    }

    #[test]
    fn can_decide_tool_invocation_requires_ready_exact_running_authority() {
        let mut runtime = ConversationRuntimeStore::new_ready_for_test();
        let conversation_id = "conversation-1".to_string();
        let agent_run_id = "run-1".to_string();
        let tool_invocation_id = "invocation-1".to_string();
        let active = active_run_with_agent_id(ActiveRunKey(0), agent_run_id.clone());
        let _decision = active.approval_broker.register_pending_for_test(
            conversation_id.clone(),
            agent_run_id.clone(),
            tool_invocation_id.clone(),
        );
        runtime.active_runs.insert(conversation_id.clone(), active);

        assert!(runtime.can_decide_tool_invocation(
            &conversation_id,
            &agent_run_id,
            &tool_invocation_id
        ));
        assert!(!runtime.can_decide_tool_invocation(
            &conversation_id,
            &"run-2".to_string(),
            &tool_invocation_id
        ));
        assert!(!runtime.can_decide_tool_invocation(
            &"conversation-2".to_string(),
            &agent_run_id,
            &tool_invocation_id
        ));
        assert!(!runtime.can_decide_tool_invocation(
            &conversation_id,
            &agent_run_id,
            &"invocation-2".to_string()
        ));

        runtime.shutting_down = true;
        assert!(!runtime.can_decide_tool_invocation(
            &conversation_id,
            &agent_run_id,
            &tool_invocation_id
        ));
        assert!(
            !ConversationRuntimeStore::new_ready_for_test().can_decide_tool_invocation(
                &conversation_id,
                &agent_run_id,
                &tool_invocation_id
            )
        );
        assert!(!ConversationRuntimeStore::new().can_decide_tool_invocation(
            &conversation_id,
            &agent_run_id,
            &tool_invocation_id
        ));
    }

    #[gpui::test]
    fn denial_with_wrong_pending_conversation_preserves_authority_and_last_error(
        cx: &mut gpui::TestAppContext,
    ) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();
        let agent_run_id = "run-1".to_string();
        let tool_invocation_id = "invocation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                let active = active_run_with_agent_id(ActiveRunKey(0), agent_run_id.clone());
                let mut decision = active.approval_broker.register_pending_for_test(
                    "conversation-2".to_string(),
                    agent_run_id.clone(),
                    tool_invocation_id.clone(),
                );
                store.active_runs.insert(conversation_id.clone(), active);
                store
                    .last_errors
                    .insert(conversation_id.clone(), "previous error".to_string());

                assert!(!store.deny_tool_invocation(
                    conversation_id.clone(),
                    tool_invocation_id.clone(),
                    cx
                ));
                assert!(decision.try_recv().is_err());
                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("previous error")
                );
            });
        });
    }

    #[gpui::test]
    fn denial_while_shutting_down_preserves_authority_and_last_error(
        cx: &mut gpui::TestAppContext,
    ) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();
        let agent_run_id = "run-1".to_string();
        let tool_invocation_id = "invocation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                let active = active_run_with_agent_id(ActiveRunKey(0), agent_run_id.clone());
                let mut decision = active.approval_broker.register_pending_for_test(
                    conversation_id.clone(),
                    agent_run_id,
                    tool_invocation_id.clone(),
                );
                store.active_runs.insert(conversation_id.clone(), active);
                store.shutting_down = true;
                store
                    .last_errors
                    .insert(conversation_id.clone(), "previous error".to_string());

                assert!(!store.deny_tool_invocation(
                    conversation_id.clone(),
                    tool_invocation_id,
                    cx
                ));
                assert!(decision.try_recv().is_err());
                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("previous error")
                );
            });
        });
    }

    #[gpui::test]
    fn approve_tool_invocation_without_active_waiting_run_is_ignored(
        cx: &mut gpui::TestAppContext,
    ) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let window = open_runtime_test_window(cx);
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        let approved = cx.update(|cx| {
            window
                .update(cx, |_view, window, cx| {
                    store.update(cx, |store, cx| {
                        store
                            .last_errors
                            .insert(conversation_id.clone(), "previous error".to_string());
                        store.approve_tool_invocation(
                            conversation_id.clone(),
                            approval_id.clone(),
                            window,
                            cx,
                        )
                    })
                })
                .unwrap()
        });
        assert!(!approved);

        cx.update(|cx| {
            let repository = test_repository(cx);
            let invocation = repository
                .get_tool_invocation(&approval_id)
                .unwrap()
                .unwrap();
            assert_eq!(invocation.status, ToolInvocationStatus::AwaitingApproval);
            assert_eq!(
                invocation.approval.as_ref().map(|approval| approval.status),
                Some(ApprovalStatus::Pending)
            );
            let agent_run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(agent_run.status, AgentRunStatus::Running);
            assert!(!store.read(cx).is_running(&conversation_id));
            store.update(cx, |store, _cx| {
                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("previous error")
                );
            });
        });
    }

    #[gpui::test]
    fn deny_tool_invocation_without_active_waiting_run_is_ignored(cx: &mut gpui::TestAppContext) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                assert!(!store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                assert!(!store.active_runs.contains_key(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });

        cx.update(|cx| {
            let repository = test_repository(cx);
            let invocation = repository
                .get_tool_invocation(&approval_id)
                .unwrap()
                .unwrap();
            assert_eq!(invocation.status, ToolInvocationStatus::AwaitingApproval);
            assert_eq!(
                invocation.approval.as_ref().map(|approval| approval.status),
                Some(ApprovalStatus::Pending)
            );
            let agent_run = repository.get_agent_run(&agent_run_id).unwrap().unwrap();
            assert_eq!(agent_run.status, AgentRunStatus::Running);
        });
    }

    #[gpui::test]
    fn deny_tool_invocation_ignores_stale_action_without_pending_broker(
        cx: &mut gpui::TestAppContext,
    ) {
        let _dir = init_runtime_test(cx);
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let (conversation_id, agent_run_id, approval_id) = cx.update(|cx| {
            let repository = test_repository(cx);
            let conversation_id = insert_conversation_with_user_item(&repository);
            let agent_run_id = insert_agent_run(&repository, &conversation_id);
            let approval_id = insert_approval_for_run(&repository, &agent_run_id);
            (conversation_id, agent_run_id, approval_id)
        });

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store.active_runs.insert(
                    conversation_id.clone(),
                    active_run_with_agent_id(ActiveRunKey(0), agent_run_id.clone()),
                );
                store
                    .last_errors
                    .insert(conversation_id.clone(), "previous error".to_string());

                assert!(!store.deny_tool_invocation(
                    conversation_id.clone(),
                    approval_id.clone(),
                    cx
                ));
                let Some(ConversationAttempt::Running(active)) =
                    store.active_runs.0.get(&conversation_id)
                else {
                    panic!("stale denial must not clear the active run");
                };
                assert_eq!(active.agent_run_id.as_ref(), Some(&agent_run_id));
                assert_eq!(
                    store.take_last_error(&conversation_id).as_deref(),
                    Some("previous error")
                );
            });
        });
    }

    #[gpui::test]
    fn finish_run_ignores_stale_run_key(cx: &mut gpui::TestAppContext) {
        let store = cx.update(|cx| cx.new(|_| ConversationRuntimeStore::new_ready_for_test()));
        let conversation_id = "conversation-1".to_string();

        cx.update(|cx| {
            store.update(cx, |store, cx| {
                store
                    .active_runs
                    .insert(conversation_id.clone(), active_run(ActiveRunKey(2)));

                assert!(!store.finish_run(
                    conversation_id.clone(),
                    ActiveRunKey(1),
                    Err("old run failed".to_string()),
                    cx
                ));
                assert!(store.active_runs.contains_key(&conversation_id));
                assert!(store.take_last_error(&conversation_id).is_none());
            });
        });
    }

    fn init_runtime_test(cx: &mut gpui::TestAppContext) -> TempDir {
        let dir = tempdir().unwrap();
        cx.update(|cx| {
            database::install_for_test(cx, dir.path());
            crate::features::conversation::resources::ConversationResourcesStore::install_global(
                cx,
                crate::features::conversation::resources::ConversationResourcesState::AwaitingDatabase,
            );
        });
        dir
    }

    fn test_repository(cx: &App) -> FreshRepository {
        database::with_ready_repository(cx, |repository| Ok(repository.clone())).unwrap()
    }

    fn open_runtime_test_window(cx: &mut gpui::TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |_window, cx| cx.new(|_| TestView))
                .expect("open runtime test window")
        })
    }

    struct TestView;

    impl gpui::Render for TestView {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    fn insert_conversation_with_user_item(repository: &FreshRepository) -> ConversationId {
        let project = repository
            .insert_project(NewProject {
                path: "/tmp/jaco-runtime-test".to_string(),
                display_name: "Runtime Test".to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: None,
                    last_active_conversation_id: None,
                },
            })
            .unwrap();
        let conversation = repository
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: "Runtime Test".to_string(),
                pinned: false,
                prompt_id: None,
                default_provider_id: None,
                default_model_id: None,
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: conversation_settings(),
            })
            .unwrap();
        repository
            .append_conversation_entry(NewConversationEntry {
                conversation_id: conversation.id.clone(),
                status: ConversationEntryStatus::Completed,
                agent_run_id: None,
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Message {
                    role: TranscriptRole::User,
                    content: vec![ContentPart::Text {
                        text: "hello".to_string(),
                    }],
                },
            })
            .unwrap();
        conversation.id
    }

    fn insert_agent_run(
        repository: &FreshRepository,
        conversation_id: &ConversationId,
    ) -> AgentRunId {
        let trigger_entry_id = repository
            .conversation_entries(conversation_id)
            .unwrap()
            .last()
            .unwrap()
            .id
            .clone();
        repository
            .insert_agent_run(NewAgentRun {
                conversation_id: conversation_id.to_string(),
                trigger_entry_id: trigger_entry_id.clone(),
                trigger_kind: AgentRunTriggerKind::User,
                input: AgentRunInput {
                    prompt_snapshot: None,
                    provider_id: "provider".to_string(),
                    model_id: "model".to_string(),
                    settings_snapshot: run_settings(),
                    runtime_snapshot: AgentRuntimeSnapshot {
                        engine: AgentEngineKind::Rig,
                        engine_version: "test".to_string(),
                        skill_catalog_hash: None,
                        tool_name_strategy: ToolNameStrategy::Direct,
                    },
                    max_steps: 8,
                },
            })
            .unwrap()
            .id
    }

    fn insert_approval_for_run(
        repository: &FreshRepository,
        agent_run_id: &AgentRunId,
    ) -> ToolInvocationId {
        let invocation = repository
            .insert_tool_invocation(NewToolInvocation {
                agent_run_id: agent_run_id.clone(),
                provider_step_id: None,
                status: ToolInvocationStatus::AwaitingApproval,
                input: ToolInvocationInput {
                    source: ToolSource::Local,
                    namespace: None,
                    tool_name: "echo".to_string(),
                    runtime_tool_name: "echo".to_string(),
                    call_id: "call-approval".to_string(),
                    arguments: ToolArguments {
                        value: serde_json::json!({"text": "hi"}),
                    },
                    approval_policy: ToolApprovalPolicy::OnRequest,
                    execution_policy: ToolExecutionPolicy::Foreground,
                },
                output: None,
                error: None,
            })
            .unwrap();
        let invocation = repository
            .request_tool_invocation_approval(
                &invocation.id,
                NewToolInvocationApproval {
                    request: ApprovalRequestPayload {
                        reason: "approve echo".to_string(),
                        tool_source: ToolSource::Local,
                        tool_name: "echo".to_string(),
                        arguments_preview: "{\"text\":\"hi\"}".to_string(),
                        access_requests: Vec::new(),
                    },
                    expires_at: None,
                },
            )
            .unwrap();
        invocation.id
    }

    fn tool_result_texts(
        repository: &FreshRepository,
        conversation_id: &ConversationId,
    ) -> Vec<String> {
        repository
            .conversation_entries(conversation_id)
            .unwrap()
            .into_iter()
            .filter_map(|item| match item.payload {
                ConversationEntryPayload::ToolResult(result) => Some(result.content),
                _ => None,
            })
            .flatten()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(text),
                _ => None,
            })
            .collect()
    }

    fn conversation_settings() -> ConversationSettingsSnapshot {
        ConversationSettingsSnapshot {
            prompt: None,
            provider_id: Some("provider".to_string()),
            model_id: Some("model".to_string()),
            model_capabilities: Some(conservative_model_capabilities("openai")),
            tool_policy: tool_policy(),
        }
    }

    fn run_settings() -> jaco_core::RunSettingsSnapshot {
        jaco_core::RunSettingsSnapshot {
            prompt: None,
            provider_id: "provider".to_string(),
            model_id: "model".to_string(),
            model_capabilities: conservative_model_capabilities("openai"),
            provider_settings: ProviderSettingsPayload {
                provider_kind: "openai".to_string(),
                fields: Vec::new(),
            },
            reasoning_selection: None,
            tool_policy: tool_policy(),
        }
    }

    fn tool_policy() -> ToolPolicySnapshot {
        ToolPolicySnapshot {
            approval_policy: ToolApprovalPolicy::Never,
            enabled_sources: vec![ToolSource::Local],
            max_steps: 8,
            approval_mode: ToolApprovalMode::RequestApproval,
            permission_scope: None,
        }
    }
}
