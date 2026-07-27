use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use jaco_agent::AgentPersistence;
use jaco_core::{
    AgentRunId, AgentRunStatus, ConversationEntryId, ConversationEntryPayload,
    ConversationEntryStatus, ConversationId, ProviderStepId, ToolInvocationApproval,
    ToolInvocationId,
};
use jaco_db::{
    AgentRunRecord, ConversationCommit, ConversationEntryRecord, ConversationTimelineRecords,
    FinishAgentRun, FinishedAgentRun, FreshRepository, FreshStore, NewAgentRun,
    NewConversationEntry, NewProviderStep, NewToolInvocation, NewToolInvocationApproval,
    NewUsageEvent, ProviderStepRecord, ToolInvocationApprovalOutcome, ToolInvocationRecord,
    UpdateAgentRunStatus, UpdateProviderStepStatus, UpdateToolInvocationStatus, UsageEventRecord,
};

use crate::{
    database::{DatabaseTarget, DatabaseTargetLease},
    errors::{JacoError, JacoResult},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DatabaseSessionKey(pub(crate) u64);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DatabaseBinding {
    pub(crate) target: DatabaseTarget,
    pub(crate) session_key: DatabaseSessionKey,
}

pub(crate) struct DatabaseSession {
    active: Option<ActiveDatabaseSession>,
    shutting_down: bool,
}

pub(crate) struct ActiveDatabaseSession {
    pub(crate) store: FreshStore,
    pub(crate) lease: DatabaseTargetLease,
    persistence: Arc<dyn AgentPersistence>,
    executor: SessionDatabaseExecutor,
}

impl ActiveDatabaseSession {
    pub(crate) fn new(store: FreshStore, lease: DatabaseTargetLease) -> Self {
        let executor = SessionDatabaseExecutor::new(store.clone());
        Self {
            persistence: Arc::new(SessionAgentPersistence::new(executor.clone())),
            executor,
            store,
            lease,
        }
    }

    pub(crate) fn active_jobs(&self) -> usize {
        self.executor.active_jobs()
    }
}

impl DatabaseSession {
    pub(crate) fn new(store: FreshStore, lease: DatabaseTargetLease) -> Self {
        Self {
            active: Some(ActiveDatabaseSession::new(store, lease)),
            shutting_down: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn repository(&self) -> JacoResult<FreshRepository> {
        if self.shutting_down {
            return Err(JacoError::Config(
                "database session is shutting down".to_string(),
            ));
        }
        self.active
            .as_ref()
            .map(|active| active.store.repository())
            .ok_or_else(|| JacoError::Config("database session is paused".to_string()))
    }

    pub(crate) fn agent_persistence(&self) -> JacoResult<Arc<dyn AgentPersistence>> {
        if self.shutting_down {
            return Err(JacoError::Config(
                "database session is shutting down".to_string(),
            ));
        }
        self.active
            .as_ref()
            .map(|active| active.persistence.clone())
            .ok_or_else(|| JacoError::Config("database session is paused".to_string()))
    }

    pub(crate) fn executor(&self) -> JacoResult<SessionDatabaseExecutor> {
        if self.shutting_down {
            return Err(JacoError::Config(
                "database session is shutting down".to_string(),
            ));
        }
        self.active
            .as_ref()
            .map(|active| active.executor.clone())
            .ok_or_else(|| JacoError::Config("database session is paused".to_string()))
    }

    pub(crate) fn take_active(&mut self) -> Option<ActiveDatabaseSession> {
        self.shutting_down = true;
        if let Some(active) = self.active.as_ref() {
            active.executor.begin_draining();
        }
        self.active.take()
    }
}

struct DatabaseActivity {
    accepting: AtomicBool,
    active_jobs: AtomicUsize,
    provider_mutation: AtomicBool,
    project_mutation: AtomicBool,
    prompt_mutation: AtomicBool,
    shortcut_mutation: AtomicBool,
    conversation_mutation: AtomicBool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CatalogMutation {
    Provider,
    Project,
    Prompt,
    Shortcut,
    Conversation,
}

#[derive(Clone)]
pub(crate) struct SessionDatabaseExecutor {
    store: FreshStore,
    activity: Arc<DatabaseActivity>,
}

impl SessionDatabaseExecutor {
    fn new(store: FreshStore) -> Self {
        Self {
            store,
            activity: Arc::new(DatabaseActivity {
                accepting: AtomicBool::new(true),
                active_jobs: AtomicUsize::new(0),
                provider_mutation: AtomicBool::new(false),
                project_mutation: AtomicBool::new(false),
                prompt_mutation: AtomicBool::new(false),
                shortcut_mutation: AtomicBool::new(false),
                conversation_mutation: AtomicBool::new(false),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(store: FreshStore) -> Self {
        Self::new(store)
    }

    pub(crate) async fn execute<R, F>(&self, command: F) -> jaco_db::Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    {
        #[cfg(test)]
        {
            let _permit = DatabaseJobPermit::acquire(self.activity.clone())?;
            command(&self.store.repository())
        }
        #[cfg(not(test))]
        {
            let store = self.store.clone();
            let activity = self.activity.clone();
            smol::unblock(move || {
                let _permit = DatabaseJobPermit::acquire(activity)?;
                let repository = store.repository();
                command(&repository)
            })
            .await
        }
    }

    pub(crate) async fn validate(&self) -> jaco_db::Result<()> {
        #[cfg(test)]
        {
            let _permit = DatabaseJobPermit::acquire(self.activity.clone())?;
            self.store.validate().map_err(Into::into)
        }
        #[cfg(not(test))]
        {
            let store = self.store.clone();
            let activity = self.activity.clone();
            smol::unblock(move || {
                let _permit = DatabaseJobPermit::acquire(activity)?;
                store.validate().map_err(Into::into)
            })
            .await
        }
    }

    pub(crate) async fn mutate<R, F>(
        &self,
        resource: CatalogMutation,
        command: F,
    ) -> jaco_db::Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    {
        #[cfg(test)]
        {
            let _permit = DatabaseJobPermit::acquire(self.activity.clone())?;
            let _mutation = CatalogMutationPermit::acquire(self.activity.clone(), resource)?;
            command(&self.store.repository())
        }
        #[cfg(not(test))]
        {
            let store = self.store.clone();
            let activity = self.activity.clone();
            smol::unblock(move || {
                let _permit = DatabaseJobPermit::acquire(activity.clone())?;
                let _mutation = CatalogMutationPermit::acquire(activity, resource)?;
                let repository = store.repository();
                command(&repository)
            })
            .await
        }
    }

    pub(crate) async fn mutate_two<R, F>(
        &self,
        first: CatalogMutation,
        second: CatalogMutation,
        command: F,
    ) -> jaco_db::Result<R>
    where
        R: Send + 'static,
        F: FnOnce(&FreshRepository) -> jaco_db::Result<R> + Send + 'static,
    {
        debug_assert_ne!(first, second);
        #[cfg(test)]
        {
            let _permit = DatabaseJobPermit::acquire(self.activity.clone())?;
            let _first = CatalogMutationPermit::acquire(self.activity.clone(), first)?;
            let _second = CatalogMutationPermit::acquire(self.activity.clone(), second)?;
            command(&self.store.repository())
        }
        #[cfg(not(test))]
        {
            let store = self.store.clone();
            let activity = self.activity.clone();
            smol::unblock(move || {
                let _permit = DatabaseJobPermit::acquire(activity.clone())?;
                let _first = CatalogMutationPermit::acquire(activity.clone(), first)?;
                let _second = CatalogMutationPermit::acquire(activity, second)?;
                let repository = store.repository();
                command(&repository)
            })
            .await
        }
    }

    pub(crate) fn begin_draining(&self) {
        self.activity.accepting.store(false, Ordering::Release);
    }

    pub(crate) fn active_jobs(&self) -> usize {
        self.activity.active_jobs.load(Ordering::Acquire)
    }
}

struct CatalogMutationPermit {
    activity: Arc<DatabaseActivity>,
    resource: CatalogMutation,
}

impl CatalogMutationPermit {
    fn acquire(
        activity: Arc<DatabaseActivity>,
        resource: CatalogMutation,
    ) -> jaco_db::Result<Self> {
        let flag = match resource {
            CatalogMutation::Provider => &activity.provider_mutation,
            CatalogMutation::Project => &activity.project_mutation,
            CatalogMutation::Prompt => &activity.prompt_mutation,
            CatalogMutation::Shortcut => &activity.shortcut_mutation,
            CatalogMutation::Conversation => &activity.conversation_mutation,
        };
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                jaco_db::DbError::Invariant(format!("{resource:?} mutation is already in progress"))
            })?;
        Ok(Self { activity, resource })
    }
}

impl Drop for CatalogMutationPermit {
    fn drop(&mut self) {
        let flag = match self.resource {
            CatalogMutation::Provider => &self.activity.provider_mutation,
            CatalogMutation::Project => &self.activity.project_mutation,
            CatalogMutation::Prompt => &self.activity.prompt_mutation,
            CatalogMutation::Shortcut => &self.activity.shortcut_mutation,
            CatalogMutation::Conversation => &self.activity.conversation_mutation,
        };
        flag.store(false, Ordering::Release);
    }
}

struct DatabaseJobPermit {
    activity: Arc<DatabaseActivity>,
}

impl DatabaseJobPermit {
    fn acquire(activity: Arc<DatabaseActivity>) -> jaco_db::Result<Self> {
        if !activity.accepting.load(Ordering::Acquire) {
            return Err(jaco_db::DbError::Invariant(
                "database session is draining".to_string(),
            ));
        }
        activity.active_jobs.fetch_add(1, Ordering::AcqRel);
        if !activity.accepting.load(Ordering::Acquire) {
            activity.active_jobs.fetch_sub(1, Ordering::AcqRel);
            return Err(jaco_db::DbError::Invariant(
                "database session is draining".to_string(),
            ));
        }
        Ok(Self { activity })
    }
}

impl Drop for DatabaseJobPermit {
    fn drop(&mut self) {
        self.activity.active_jobs.fetch_sub(1, Ordering::AcqRel);
    }
}

struct SessionAgentPersistence {
    executor: SessionDatabaseExecutor,
}

impl SessionAgentPersistence {
    fn new(executor: SessionDatabaseExecutor) -> Self {
        Self { executor }
    }
}

macro_rules! repository_call {
    ($self:ident, $method:ident ( $($argument:expr),* $(,)? )) => {{
        $self
            .executor
            .execute(move |repository| repository.$method($($argument),*))
            .await
    }};
}

#[async_trait]
impl AgentPersistence for SessionAgentPersistence {
    async fn conversation_timeline(
        &self,
        id: ConversationId,
    ) -> jaco_db::Result<Option<ConversationTimelineRecords>> {
        repository_call!(self, conversation_timeline_records(&id))
    }

    async fn conversation_entries(
        &self,
        id: ConversationId,
    ) -> jaco_db::Result<Vec<ConversationEntryRecord>> {
        repository_call!(self, conversation_entries(&id))
    }

    async fn append_conversation_entry(
        &self,
        input: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<ConversationEntryRecord>> {
        repository_call!(self, append_conversation_entry(input))
    }

    async fn update_conversation_entry_payload(
        &self,
        id: ConversationEntryId,
        status: ConversationEntryStatus,
        payload: ConversationEntryPayload,
    ) -> jaco_db::Result<ConversationCommit<ConversationEntryRecord>> {
        repository_call!(
            self,
            update_conversation_entry_payload(&id, status, payload)
        )
    }

    async fn insert_agent_run(&self, input: NewAgentRun) -> jaco_db::Result<AgentRunRecord> {
        repository_call!(self, insert_agent_run(input))
    }

    async fn get_agent_run(&self, id: AgentRunId) -> jaco_db::Result<Option<AgentRunRecord>> {
        repository_call!(self, get_agent_run(&id))
    }

    async fn agent_runs_for_conversation(
        &self,
        id: ConversationId,
    ) -> jaco_db::Result<Vec<AgentRunRecord>> {
        repository_call!(self, agent_runs_for_conversation(&id))
    }

    async fn agent_runs_by_status(
        &self,
        status: AgentRunStatus,
    ) -> jaco_db::Result<Vec<AgentRunRecord>> {
        repository_call!(self, agent_runs_by_status(status))
    }

    async fn update_agent_run_status(
        &self,
        id: AgentRunId,
        update: UpdateAgentRunStatus,
    ) -> jaco_db::Result<AgentRunRecord> {
        repository_call!(self, update_agent_run_status(&id, update))
    }

    async fn finish_agent_run(
        &self,
        id: AgentRunId,
        finish: FinishAgentRun,
    ) -> jaco_db::Result<ConversationCommit<FinishedAgentRun>> {
        repository_call!(self, finish_agent_run(&id, finish))
    }

    async fn next_provider_step_seq(&self, id: AgentRunId) -> jaco_db::Result<i32> {
        repository_call!(self, next_provider_step_seq(&id))
    }

    async fn insert_provider_step(
        &self,
        input: NewProviderStep,
    ) -> jaco_db::Result<ProviderStepRecord> {
        repository_call!(self, insert_provider_step(input))
    }

    async fn provider_steps_for_run(
        &self,
        id: AgentRunId,
    ) -> jaco_db::Result<Vec<ProviderStepRecord>> {
        repository_call!(self, provider_steps_for_run(&id))
    }

    async fn update_provider_step_status(
        &self,
        id: ProviderStepId,
        update: UpdateProviderStepStatus,
    ) -> jaco_db::Result<ProviderStepRecord> {
        repository_call!(self, update_provider_step_status(&id, update))
    }

    async fn insert_usage_event(&self, input: NewUsageEvent) -> jaco_db::Result<UsageEventRecord> {
        repository_call!(self, insert_usage_event(input))
    }

    async fn insert_tool_invocation(
        &self,
        input: NewToolInvocation,
    ) -> jaco_db::Result<ToolInvocationRecord> {
        repository_call!(self, insert_tool_invocation(input))
    }

    async fn tool_invocations_for_run(
        &self,
        id: AgentRunId,
    ) -> jaco_db::Result<Vec<ToolInvocationRecord>> {
        repository_call!(self, tool_invocations_for_run(&id))
    }

    async fn update_tool_invocation_status(
        &self,
        id: ToolInvocationId,
        update: UpdateToolInvocationStatus,
    ) -> jaco_db::Result<ToolInvocationRecord> {
        repository_call!(self, update_tool_invocation_status(&id, update))
    }

    async fn append_entries_and_update_tool_invocation(
        &self,
        entries: Vec<NewConversationEntry>,
        id: ToolInvocationId,
        update: UpdateToolInvocationStatus,
        approval: Option<ToolInvocationApproval>,
    ) -> jaco_db::Result<ConversationCommit<(Vec<ConversationEntryRecord>, ToolInvocationRecord)>>
    {
        repository_call!(
            self,
            append_conversation_entries_and_update_tool_invocation_full(
                entries, &id, update, approval
            )
        )
    }

    async fn request_tool_invocation_approval_with_entry(
        &self,
        id: ToolInvocationId,
        approval: NewToolInvocationApproval,
        entry: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>> {
        repository_call!(
            self,
            request_tool_invocation_approval_with_entry(&id, approval, entry)
        )
    }

    async fn decide_tool_invocation_approval_with_entry(
        &self,
        id: ToolInvocationId,
        outcome: ToolInvocationApprovalOutcome,
        next_status: jaco_core::ToolInvocationStatus,
        entry: NewConversationEntry,
    ) -> jaco_db::Result<ConversationCommit<(ConversationEntryRecord, ToolInvocationRecord)>> {
        repository_call!(
            self,
            decide_tool_invocation_approval_with_entry(&id, outcome, next_status, entry)
        )
    }
}

impl fmt::Debug for DatabaseSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseSession")
            .field("active", &self.active.is_some())
            .field("shutting_down", &self.shutting_down)
            .finish()
    }
}
