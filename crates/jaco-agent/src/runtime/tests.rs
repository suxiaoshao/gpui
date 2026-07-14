use super::*;
use crate::{
    LocalTool, McpConnector, ProviderSecretValues, RegisteredToolDefinition, ToolApprovalBroker,
    ToolApprovalDecision, ToolApprovalRequest, ToolDefinition, ToolExecutor, ToolRunPolicy,
};
use async_trait::async_trait;
use jaco_db::{
    AgentRunFinalEntry, ConversationEntryRecord, ConversationRecord, FinishAgentRun, FreshStore,
    NewConversation, NewConversationEntry, NewProject, NewProvider, NewProviderModel,
    NewProviderStep, NewToolInvocation, NewToolInvocationApproval, ProviderModelRecord,
    ProviderRecord, ProviderStepRecord, ToolInvocationRecord, UpdateProviderStepStatus,
    UpdateToolInvocationStatus,
};
use rig_core::{
    OneOrMany,
    agent::{PromptHook, ToolCallHookAction},
    completion::{
        AssistantContent, CompletionError, CompletionRequest, CompletionResponse,
        Message as RigMessage,
    },
    message::UserContent,
    streaming::{RawStreamingChoice, StreamingCompletionResponse, StreamingResult},
    test_utils::{MockCompletionModel, MockResponse, MockStreamEvent, MockTurn},
};
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, ListToolsResult,
        PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
};
use serde_json::json;
use std::{
    collections::VecDeque,
    fs,
    future::pending,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tempfile::TempDir;
use tokio::sync::{Notify, RwLock, oneshot};

struct ManualApprovalBroker {
    pending: Mutex<VecDeque<ManualPendingApproval>>,
    notify: Notify,
}

struct ManualPendingApproval {
    request: ToolApprovalRequest,
    sender: oneshot::Sender<ToolApprovalDecision>,
}

impl Default for ManualApprovalBroker {
    fn default() -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
        }
    }
}

impl ManualApprovalBroker {
    async fn wait_for_request(&self) -> ToolApprovalRequest {
        loop {
            if let Some(request) = self
                .pending
                .lock()
                .unwrap()
                .front()
                .map(|pending| pending.request.clone())
            {
                return request;
            }
            self.notify.notified().await;
        }
    }

    fn resolve_next(&self, decision: ToolApprovalDecision) {
        let pending = self
            .pending
            .lock()
            .unwrap()
            .pop_front()
            .expect("pending approval exists");
        let _ = pending.sender.send(decision);
    }
}

impl ToolApprovalBroker for ManualApprovalBroker {
    fn request_tool_approval<'a>(
        &'a self,
        request: ToolApprovalRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolApprovalDecision> + Send + 'a>>
    {
        Box::pin(async move {
            let (sender, receiver) = oneshot::channel();
            self.pending
                .lock()
                .unwrap()
                .push_back(ManualPendingApproval { request, sender });
            self.notify.notify_waiters();
            receiver.await.unwrap_or(ToolApprovalDecision::Canceled)
        })
    }
}

#[tokio::test]
async fn no_tool_run_persists_provider_step_and_final_message() {
    let fixture = Fixture::new("no-tool");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let model = MockCompletionModel::text("hello from model");
    let handle = runtime
        .run_with_model(fixture.request(), model)
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert_eq!(
        handle.output.unwrap().stopped_reason,
        AgentStoppedReason::Completed
    );
    assert_eq!(
        fixture
            .repo
            .provider_steps_for_run(&handle.agent_run.id)
            .unwrap()
            .len(),
        1
    );
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert!(items.iter().any(|item| matches!(
        &item.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            content,
        } if content[0].search_text() == Some("hello from model")
    )));
}

#[tokio::test]
async fn streaming_text_delta_updates_single_assistant_item() {
    let fixture = Fixture::new("streaming-text");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let model = MockCompletionModel::from_stream_turns([[
        MockStreamEvent::text("hello "),
        MockStreamEvent::text("world"),
        MockStreamEvent::final_response_with_total_tokens(7),
    ]]);

    let handle = runtime
        .run_with_model(fixture.streaming_request(), model)
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    let output = handle.output.as_ref().unwrap();
    assert_eq!(output.stopped_reason, AgentStoppedReason::Completed);

    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let assistant_items = items
        .iter()
        .filter(|item| {
            matches!(
                item.payload,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::Assistant,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(assistant_items.len(), 1);
    assert_eq!(
        assistant_items[0].status,
        ConversationEntryStatus::Completed
    );
    assert_eq!(
        Some(output.final_entry_id.as_str()),
        Some(assistant_items[0].id.as_str())
    );
    assert!(matches!(
        &assistant_items[0].payload,
        ConversationEntryPayload::Message { content, .. }
            if content[0].search_text() == Some("hello world")
    ));

    let provider_steps = fixture
        .repo
        .provider_steps_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(provider_steps[0].status, ProviderStepStatus::Completed);
    let usage_events = fixture
        .repo
        .usage_events_for_provider_step(&provider_steps[0].id)
        .unwrap();
    assert_eq!(usage_events.len(), 1);
    assert_eq!(usage_events[0].usage.total_tokens, 7);
    assert!(handle.events.iter().any(|event| matches!(
        event,
        AgentRunEvent::ProviderStepEvent {
            event: ProviderStepEvent::TextDelta { text, .. },
            ..
        } if text == "world"
    )));
}

#[tokio::test]
async fn streaming_provider_open_error_stays_before_later_user_entry_after_reload() {
    let fixture = Fixture::new("streaming-provider-open-error");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let db_path = fixture.db_path.clone();
    let conversation_id = fixture.conversation.id.clone();
    let initial_user_entry_id = fixture.user_item.id.clone();

    let handle = runtime
        .run_with_model(fixture.streaming_request(), FailBeforeFirstTokenModel)
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Failed);
    let error = handle.agent_run.error.clone().expect("run error");
    let output = handle.output.clone().expect("failed run output");
    let error_entry_id = output.final_entry_id.clone();

    let later_user = fixture
        .repo
        .append_conversation_entry(NewConversationEntry {
            conversation_id: conversation_id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content: vec![ContentPart::Text {
                    text: "later user message".to_string(),
                }],
            },
        })
        .unwrap();

    drop(runtime);
    drop(fixture.repo);
    let reopened = FreshStore::open(&db_path).unwrap().repository();
    let timeline = reopened
        .conversation_timeline_records(&conversation_id)
        .unwrap()
        .unwrap();
    assert_eq!(timeline.items.len(), 3);
    assert!(matches!(
        timeline.items[0].payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            ..
        }
    ));
    assert_eq!(timeline.items[0].id, initial_user_entry_id);
    assert!(matches!(
        timeline.items[1].payload,
        ConversationEntryPayload::Error(_)
    ));
    assert!(matches!(
        timeline.items[2].payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            ..
        }
    ));
    assert_eq!(timeline.items[1].id, error_entry_id);
    assert_eq!(timeline.items[2].id, later_user.id);
    assert!(
        timeline
            .items
            .windows(2)
            .all(|items| items[0].seq < items[1].seq)
    );
    assert_eq!(
        timeline.items[1].agent_run_id.as_deref(),
        Some(handle.agent_run.id.as_str())
    );
    assert_eq!(
        timeline.items[1].payload,
        ConversationEntryPayload::Error(error.clone())
    );

    let persisted_run = reopened
        .get_agent_run(&handle.agent_run.id)
        .unwrap()
        .unwrap();
    assert_eq!(persisted_run.status, AgentRunStatus::Failed);
    assert_eq!(persisted_run.error, Some(error));
    assert_eq!(persisted_run.output.unwrap().final_entry_id, error_entry_id);
}

#[tokio::test]
async fn blocking_provider_error_persists_final_error_entry() {
    let fixture = Fixture::new("blocking-provider-error");
    let runtime = AgentRuntime::new(fixture.repo.clone());

    let handle = runtime
        .run_with_model(fixture.request(), FailBeforeFirstTokenModel)
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Failed);
    let error_entry_id = handle
        .output
        .as_ref()
        .expect("blocking provider error output")
        .final_entry_id
        .clone();
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let error_entry = items.iter().find(|item| item.id == error_entry_id).unwrap();
    assert!(matches!(
        error_entry.payload,
        ConversationEntryPayload::Error(_)
    ));
    assert_eq!(error_entry.status, ConversationEntryStatus::Failed);
}

#[tokio::test]
async fn completed_without_output_uses_status_final_entry() {
    let fixture = Fixture::new("completed-without-output");
    let runtime = AgentRuntime::new(fixture.repo.clone());

    let handle = runtime
        .run_with_model(fixture.request(), MockCompletionModel::text(""))
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert_eq!(handle.agent_run.error, None);
    assert_eq!(
        handle.output.as_ref().unwrap().stopped_reason,
        AgentStoppedReason::Completed
    );
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let final_entry_id = &handle.output.as_ref().unwrap().final_entry_id;
    let final_entry = items
        .iter()
        .find(|item| &item.id == final_entry_id)
        .expect("completed run final entry");
    assert_eq!(final_entry.status, ConversationEntryStatus::Completed);
    assert!(matches!(
        final_entry.payload,
        ConversationEntryPayload::Status(ConversationStatusEntry {
            code: ConversationStatusCode::CompletedWithoutOutput,
            message: None,
        })
    ));
    assert!(!items.iter().any(|item| matches!(
        item.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            ..
        }
    )));
}

#[tokio::test]
async fn partial_stream_failure_keeps_partial_entry_and_finishes_with_error() {
    let fixture = Fixture::new("streaming-provider-error-after-text");
    let runtime = AgentRuntime::new(fixture.repo.clone());

    let handle = runtime
        .run_with_model(fixture.streaming_request(), FailAfterTextModel)
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Failed);
    let final_entry_id = handle
        .output
        .as_ref()
        .expect("partial failure output")
        .final_entry_id
        .clone();
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let partial = items
        .iter()
        .find(|item| {
            matches!(
                &item.payload,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::Assistant,
                    content,
                } if content[0].search_text() == Some("partial")
            )
        })
        .expect("partial assistant entry");
    assert_eq!(partial.status, ConversationEntryStatus::Failed);
    let final_entry = items
        .iter()
        .find(|item| item.id == final_entry_id)
        .expect("failed run final entry");
    assert!(matches!(
        &final_entry.payload,
        ConversationEntryPayload::Error(_)
    ));
    assert!(partial.seq < final_entry.seq);
    assert!(items.iter().any(|item| {
        matches!(
            item.payload,
            ConversationEntryPayload::Error(ref error)
                if Some(error) == handle.agent_run.error.as_ref()
        )
    }));
    assert!(items.windows(2).all(|items| items[0].seq < items[1].seq));
}

#[tokio::test]
async fn retry_runs_share_trigger_entry_and_keep_entry_sequence() {
    let fixture = Fixture::new("retry-order");
    let runtime = AgentRuntime::new(fixture.repo.clone());

    let first = runtime
        .run_with_model(fixture.streaming_request(), FailBeforeFirstTokenModel)
        .await
        .unwrap();
    let second = runtime
        .run_with_model(
            fixture.request(),
            MockCompletionModel::text("retry success"),
        )
        .await
        .unwrap();

    assert_ne!(first.agent_run.id, second.agent_run.id);
    assert_eq!(
        first.agent_run.trigger_entry_id,
        second.agent_run.trigger_entry_id
    );
    assert_eq!(first.agent_run.status, AgentRunStatus::Failed);
    assert_eq!(second.agent_run.status, AgentRunStatus::Completed);

    let entries = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert_eq!(entries.len(), 3);
    assert!(matches!(
        entries[0].payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::User,
            ..
        }
    ));
    assert_eq!(
        entries[1].agent_run_id.as_deref(),
        Some(first.agent_run.id.as_str())
    );
    assert!(matches!(
        entries[1].payload,
        ConversationEntryPayload::Error(_)
    ));
    assert_eq!(
        entries[2].agent_run_id.as_deref(),
        Some(second.agent_run.id.as_str())
    );
    assert!(matches!(
        entries[2].payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            ..
        }
    ));
    assert!(
        entries
            .windows(2)
            .all(|entries| entries[0].seq < entries[1].seq)
    );
    assert_eq!(
        first.agent_run.output.as_ref().unwrap().final_entry_id,
        entries[1].id
    );
    assert_eq!(
        second.agent_run.output.as_ref().unwrap().final_entry_id,
        entries[2].id
    );
}

#[tokio::test]
async fn streaming_provider_step_stays_running_until_final_event() {
    let fixture = Fixture::new("streaming-step-running");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let repo = fixture.repo.clone();
    let request = fixture.streaming_request();
    let task = tokio::spawn(async move {
        runtime
            .run_with_model(
                request,
                DelayedFinalStreamModel {
                    delay: Duration::from_millis(100),
                },
            )
            .await
    });

    let mut observed_step = None;
    for _ in 0..50 {
        for run in repo.agent_runs_by_status(AgentRunStatus::Running).unwrap() {
            let steps = repo.provider_steps_for_run(&run.id).unwrap();
            if let Some(step) = steps.first() {
                observed_step = Some((run.id.clone(), step.clone()));
                break;
            }
        }
        if observed_step.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let (agent_run_id, provider_step) =
        observed_step.expect("streaming provider step should be inserted before final response");
    assert_eq!(provider_step.status, ProviderStepStatus::Running);
    assert!(provider_step.response_snapshot.is_none());
    assert!(provider_step.completed_at.is_none());

    let handle = task.await.unwrap().unwrap();
    assert_eq!(handle.agent_run.id, agent_run_id);
    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    let provider_step = repo
        .get_provider_step(&provider_step.id)
        .unwrap()
        .expect("provider step should still exist");
    assert_eq!(provider_step.status, ProviderStepStatus::Completed);
    assert!(provider_step.response_snapshot.is_some());
}

#[tokio::test]
async fn streaming_reasoning_delta_updates_single_reasoning_item() {
    let fixture = Fixture::new("streaming-reasoning");
    let runtime = AgentRuntime::new(fixture.repo.clone());

    let handle = runtime
        .run_with_model(fixture.streaming_request(), ReasoningStreamModel)
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let reasoning_items = items
        .iter()
        .filter(|item| matches!(item.payload, ConversationEntryPayload::Reasoning { .. }))
        .collect::<Vec<_>>();
    assert_eq!(reasoning_items.len(), 1);
    assert_eq!(
        reasoning_items[0].status,
        ConversationEntryStatus::Completed
    );
    assert!(matches!(
        &reasoning_items[0].payload,
        ConversationEntryPayload::Reasoning { text, summary: None }
            if text == "thinking now"
    ));
    assert!(handle.events.iter().any(|event| matches!(
        event,
        AgentRunEvent::ProviderStepEvent {
            event: ProviderStepEvent::ReasoningDelta { text, .. },
            ..
        } if text == "now"
    )));
}

#[tokio::test]
async fn streaming_tool_call_is_persisted_only_by_hook() {
    let fixture = Fixture::new("streaming-tool");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.streaming_request();
    request
        .tool_registry
        .register_local_tool(EchoTool::new(ToolApprovalPolicy::Never))
        .unwrap();
    let model = MockCompletionModel::from_stream_turns([
        vec![MockStreamEvent::tool_call(
            "call_1",
            "echo",
            json!({"text": "hi"}),
        )],
        vec![
            MockStreamEvent::text("done"),
            MockStreamEvent::final_response_with_total_tokens(5),
        ],
    ]);

    let handle = runtime.run_with_model(request, model).await.unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].status, ToolInvocationStatus::Succeeded);

    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.payload, ConversationEntryPayload::ToolCall(_)))
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.payload, ConversationEntryPayload::ToolResult(_)))
            .count(),
        1
    );
    assert!(items.iter().any(|item| matches!(
        &item.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            content,
        } if content[0].search_text() == Some("done")
    )));
}

#[tokio::test]
async fn streaming_approval_required_preserves_partial_text() {
    let fixture = Fixture::new("streaming-approval");
    let broker = Arc::new(ManualApprovalBroker::default());
    let runtime = AgentRuntime::new(fixture.repo.clone()).with_approval_broker(broker.clone());
    let mut request = fixture.streaming_request();
    let cancellation_token = request.cancellation_token.clone();
    request
        .tool_registry
        .register_local_tool(EchoTool::new(ToolApprovalPolicy::Always))
        .unwrap();
    let model = MockCompletionModel::from_stream_turns([[
        MockStreamEvent::text("partial answer"),
        MockStreamEvent::tool_call("call_1", "echo", json!({"text": "hi"})),
    ]]);

    let repo = fixture.repo.clone();
    let conversation_id = fixture.conversation.id.clone();
    let run = tokio::spawn(async move { runtime.run_with_model(request, model).await });
    let approval_request = broker.wait_for_request().await;

    let items = repo.conversation_entries(&conversation_id).unwrap();
    let assistant_item = items
        .iter()
        .find(|item| {
            matches!(
                item.payload,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::Assistant,
                    ..
                }
            )
        })
        .unwrap();
    assert!(matches!(
        &assistant_item.payload,
        ConversationEntryPayload::Message { content, .. }
            if content[0].search_text() == Some("partial answer")
    ));
    let invocations = repo
        .tool_invocations_for_run(&approval_request.agent_run_id)
        .unwrap();
    assert_eq!(
        invocations[0]
            .approval
            .as_ref()
            .map(|approval| approval.status),
        Some(ApprovalStatus::Pending)
    );

    let provider_steps = repo
        .provider_steps_for_run(&approval_request.agent_run_id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(provider_steps[0].status, ProviderStepStatus::Running);

    cancellation_token.cancel();
    broker.resolve_next(ToolApprovalDecision::Canceled);
    let handle = run.await.unwrap().unwrap();
    assert_eq!(handle.agent_run.status, AgentRunStatus::Canceled);
}

#[tokio::test]
async fn streaming_cancellation_marks_running_item_and_provider_step_canceled() {
    let fixture = Fixture::new("streaming-cancel");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let request = fixture.streaming_request();
    let model = CancelAfterTextStreamModel {
        cancellation_token: request.cancellation_token.clone(),
    };

    let handle = runtime.run_with_model(request, model).await.unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Canceled);
    assert_eq!(
        handle.output.as_ref().unwrap().stopped_reason,
        AgentStoppedReason::Canceled
    );
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let assistant_item = items
        .iter()
        .find(|item| {
            matches!(
                item.payload,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::Assistant,
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(assistant_item.status, ConversationEntryStatus::Canceled);
    assert!(matches!(
        &assistant_item.payload,
        ConversationEntryPayload::Message { content, .. }
            if content[0].search_text() == Some("partial")
    ));

    let provider_steps = fixture
        .repo
        .provider_steps_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(provider_steps[0].status, ProviderStepStatus::Canceled);
}

#[tokio::test]
async fn non_streaming_cancellation_before_response_persistence_marks_run_canceled() {
    let fixture = Fixture::new("non-streaming-cancel");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let request = fixture.request();
    let model = CancelDuringCompletionModel {
        cancellation_token: request.cancellation_token.clone(),
    };

    let handle = runtime.run_with_model(request, model).await.unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Canceled);
    assert_eq!(
        handle.output.as_ref().unwrap().stopped_reason,
        AgentStoppedReason::Canceled
    );
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert!(!items.iter().any(|item| matches!(
        item.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            ..
        }
    )));
    let final_entry_id = &handle.output.as_ref().unwrap().final_entry_id;
    let final_entry = items
        .iter()
        .find(|item| &item.id == final_entry_id)
        .expect("canceled run final entry");
    assert_eq!(final_entry.status, ConversationEntryStatus::Canceled);
    assert!(matches!(
        final_entry.payload,
        ConversationEntryPayload::Status(ConversationStatusEntry {
            code: ConversationStatusCode::Canceled,
            message: None,
        })
    ));
    let provider_steps = fixture
        .repo
        .provider_steps_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(provider_steps[0].status, ProviderStepStatus::Canceled);
    assert_eq!(provider_steps[0].error.as_ref().unwrap().code, "canceled");
    let usage_events = fixture
        .repo
        .usage_events_for_provider_step(&provider_steps[0].id)
        .unwrap();
    assert!(usage_events.is_empty());
}

#[tokio::test]
async fn non_streaming_cancellation_during_provider_open_does_not_wait_for_response() {
    let fixture = Fixture::new("non-streaming-open-cancel");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let request = fixture.request();
    let model = CancelAndPendCompletionModel {
        cancellation_token: request.cancellation_token.clone(),
    };

    let handle = tokio::time::timeout(
        Duration::from_secs(1),
        runtime.run_with_model(request, model),
    )
    .await
    .expect("canceled provider open should not hang")
    .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Canceled);
    assert_eq!(
        handle.output.as_ref().unwrap().stopped_reason,
        AgentStoppedReason::Canceled
    );
    let provider_steps = fixture
        .repo
        .provider_steps_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(provider_steps[0].status, ProviderStepStatus::Canceled);
    assert_eq!(provider_steps[0].error.as_ref().unwrap().code, "canceled");
}

#[tokio::test]
async fn streaming_cancellation_during_provider_open_does_not_record_provider_error() {
    let fixture = Fixture::new("streaming-open-cancel");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let request = fixture.streaming_request();
    let model = CancelAndPendStreamOpenModel {
        cancellation_token: request.cancellation_token.clone(),
    };

    let handle = tokio::time::timeout(
        Duration::from_secs(1),
        runtime.run_with_model(request, model),
    )
    .await
    .expect("canceled stream open should not hang")
    .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Canceled);
    let provider_steps = fixture
        .repo
        .provider_steps_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(provider_steps[0].status, ProviderStepStatus::Canceled);
    assert_eq!(provider_steps[0].error.as_ref().unwrap().code, "canceled");
}

#[tokio::test]
async fn streaming_cancellation_while_waiting_for_next_chunk_finishes_immediately() {
    let fixture = Fixture::new("streaming-next-cancel");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let request = fixture.streaming_request();
    let model = CancelAndPendNextStreamModel {
        cancellation_token: request.cancellation_token.clone(),
    };

    let handle = tokio::time::timeout(
        Duration::from_secs(1),
        runtime.run_with_model(request, model),
    )
    .await
    .expect("canceled stream next should not hang")
    .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Canceled);
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let assistant_item = items
        .iter()
        .find(|item| {
            matches!(
                item.payload,
                ConversationEntryPayload::Message {
                    role: TranscriptRole::Assistant,
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(assistant_item.status, ConversationEntryStatus::Canceled);
    let provider_steps = fixture
        .repo
        .provider_steps_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(provider_steps[0].status, ProviderStepStatus::Canceled);
}

#[tokio::test]
async fn streaming_disabled_uses_non_streaming_prompt() {
    let fixture = Fixture::new("streaming-disabled");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let model = MockCompletionModel::text("non-stream response");

    let handle = runtime
        .run_with_model(fixture.request(), model.clone())
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert_eq!(model.request_count(), 1);
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert!(items.iter().any(|item| matches!(
        &item.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            content,
        } if content[0].search_text() == Some("non-stream response")
    )));
}

#[tokio::test]
async fn enabled_builtin_tools_are_exposed_to_rig_requests() {
    let fixture = Fixture::new("builtin-tools");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.project_root = Some(fixture.dir.path().to_path_buf());
    let model = MockCompletionModel::text("ok");

    runtime
        .run_with_model(request, model.clone())
        .await
        .unwrap();

    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    let tool_names = requests[0]
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec![
            "read_file",
            "list_directory",
            "find_path",
            "grep",
            "write_file",
            "edit_file"
        ]
    );
}

#[tokio::test]
async fn full_access_builtin_tool_does_not_persist_empty_approval_entries() {
    let fixture = Fixture::new("builtin-full-access");
    fs::write(fixture.dir.path().join("hello.txt"), "hello from fixture").unwrap();
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.project_root = Some(fixture.dir.path().to_path_buf());
    request.settings_snapshot.tool_policy.permission_scope = Some(ToolPermissionScopeSnapshot {
        project_roots: vec![fixture.dir.path().to_string_lossy().into_owned()],
        external_read_requires_approval: false,
        external_write_requires_approval: true,
    });
    request.settings_snapshot.tool_policy.approval_mode = ToolApprovalMode::FullAccess;
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "read_file", json!({"path": "hello.txt"})),
        MockTurn::text("done"),
    ]);

    let handle = runtime.run_with_model(request, model).await.unwrap();
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].status, ToolInvocationStatus::Succeeded);
    assert_eq!(invocations[0].approval, None);

    let entries = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert!(entries.iter().all(|entry| {
        !matches!(
            entry.payload,
            ConversationEntryPayload::ApprovalRequest(_)
                | ConversationEntryPayload::ApprovalDecision(_)
        )
    }));
    assert!(
        entries
            .iter()
            .any(|entry| matches!(entry.payload, ConversationEntryPayload::ToolCall(_)))
    );
    assert!(
        entries
            .iter()
            .any(|entry| matches!(entry.payload, ConversationEntryPayload::ToolResult(_)))
    );
}

#[tokio::test]
async fn rig_tool_call_persists_tool_call_and_result() {
    let fixture = Fixture::new("tool-run");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request
        .tool_registry
        .register_local_tool(EchoTool::new(ToolApprovalPolicy::Never))
        .unwrap();
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "echo", json!({"text": "hi"})),
        MockTurn::text("done"),
    ]);

    let handle = runtime.run_with_model(request, model).await.unwrap();
    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].status, ToolInvocationStatus::Succeeded);
    assert_eq!(invocations[0].runtime_tool_name, "echo");
    let output = invocations[0].output.as_ref().unwrap();
    assert_eq!(
        output.content,
        vec![ContentPart::Text {
            text: "hi".to_string()
        }]
    );
    assert_eq!(
        output.structured_output.as_ref().unwrap().value,
        json!({"text": "hi"})
    );

    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert!(
        items
            .iter()
            .any(|item| matches!(item.payload, ConversationEntryPayload::ToolCall(_)))
    );
    assert!(
        items
            .iter()
            .any(|item| matches!(item.payload, ConversationEntryPayload::ToolResult(_)))
    );
    let tool_result = items
        .iter()
        .find_map(|item| match &item.payload {
            ConversationEntryPayload::ToolResult(result) => Some(result),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        tool_result.content,
        vec![ContentPart::Text {
            text: "hi".to_string()
        }]
    );
    assert_eq!(
        tool_result.structured_output.as_ref().unwrap().value,
        json!({"text": "hi"})
    );
    assert!(!tool_result.is_error);
}

#[tokio::test]
async fn tool_execution_cancellation_does_not_persist_tool_output() {
    let fixture = Fixture::new("tool-cancel-during-await");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.guards.tool_timeout = Duration::from_millis(50);
    request
        .tool_registry
        .register_local_tool(CancelDuringTool {
            cancellation_token: request.cancellation_token.clone(),
        })
        .unwrap();
    let model = MockCompletionModel::new([MockTurn::tool_call(
        "call_1",
        "cancel_during_tool",
        json!({}),
    )]);

    let handle = runtime.run_with_model(request, model).await.unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Canceled);
    assert_eq!(
        handle.output.as_ref().unwrap().stopped_reason,
        AgentStoppedReason::Canceled
    );
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].status, ToolInvocationStatus::Canceled);
    assert_eq!(invocations[0].error.as_ref().unwrap().code, "canceled");
    assert_eq!(tool_result_texts(&fixture), vec!["runtime canceled"]);
}

#[tokio::test]
async fn tool_error_output_is_persisted_without_reconstructing_from_model_text() {
    let fixture = Fixture::new("tool-error-output");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request
        .tool_registry
        .register_local_tool(ErrorTool)
        .unwrap();
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "error_tool", json!({"code": "E_BAD"})),
        MockTurn::text("handled"),
    ]);

    let handle = runtime.run_with_model(request, model).await.unwrap();
    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].status, ToolInvocationStatus::Failed);
    assert_eq!(invocations[0].error.as_ref().unwrap().code, "tool_error");
    let output = invocations[0].output.as_ref().unwrap();
    assert!(output.is_error);
    assert_eq!(
        output.content,
        vec![ContentPart::Text {
            text: "human readable error".to_string()
        }]
    );
    assert_eq!(
        output.structured_output.as_ref().unwrap().value,
        json!({"code": "E_BAD"})
    );
    assert_eq!(
        output.raw_output.as_ref().unwrap().value,
        json!({"raw": "details"})
    );

    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let tool_result = items
        .iter()
        .find_map(|item| match &item.payload {
            ConversationEntryPayload::ToolResult(result) => Some(result),
            _ => None,
        })
        .unwrap();
    assert!(tool_result.is_error);
    assert_eq!(
        tool_result.content,
        vec![ContentPart::Text {
            text: "human readable error".to_string()
        }]
    );
    assert_eq!(
        tool_result.structured_output.as_ref().unwrap().value,
        json!({"code": "E_BAD"})
    );
    assert_eq!(
        tool_result.raw_output.as_ref().unwrap().value,
        json!({"raw": "details"})
    );
}

#[tokio::test]
async fn recoverable_builtin_argument_error_is_returned_to_model() {
    let fixture = Fixture::new("recoverable-builtin-args");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.project_root = Some(fixture.dir.path().to_path_buf());
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "find_path", json!({"path": "."})),
        MockTurn::text("recovered"),
    ]);

    let handle = runtime
        .run_with_model(request, model.clone())
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert_eq!(model.request_count(), 2);
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].status, ToolInvocationStatus::Failed);
    let error = invocations[0].error.as_ref().unwrap();
    assert_eq!(error.code, "tool_invalid_arguments");
    assert!(
        error
            .message
            .contains("Invalid arguments for tool find_path")
    );
    assert!(error.message.contains("query"));
    assert!(invocations[0].output.as_ref().unwrap().is_error);

    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.payload, ConversationEntryPayload::ToolCall(_)))
            .count(),
        1
    );
    let tool_result = items
        .iter()
        .find_map(|item| match &item.payload {
            ConversationEntryPayload::ToolResult(result) => Some(result),
            _ => None,
        })
        .unwrap();
    assert!(tool_result.is_error);
    assert_eq!(tool_result.call_id, invocations[0].call_id);
    assert!(
        tool_result.content[0]
            .search_text()
            .is_some_and(|text| text.contains("query"))
    );
    assert!(items.iter().any(|item| matches!(
        &item.payload,
        ConversationEntryPayload::Message {
            role: TranscriptRole::Assistant,
            content,
        } if content[0].search_text() == Some("recovered")
    )));
}

#[tokio::test]
async fn recoverable_unknown_tool_is_returned_to_model() {
    let fixture = Fixture::new("recoverable-unknown-tool");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "missing_tool", json!({"path": "."})),
        MockTurn::text("recovered"),
    ]);

    let handle = runtime
        .run_with_model(fixture.request(), model.clone())
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert_eq!(model.request_count(), 2);
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].source, ToolSource::Local);
    assert_eq!(invocations[0].tool_name, "missing_tool");
    assert_eq!(invocations[0].runtime_tool_name, "missing_tool");
    assert_eq!(
        invocations[0].input.approval_policy,
        ToolApprovalPolicy::Never
    );
    assert_eq!(
        invocations[0].input.execution_policy,
        ToolExecutionPolicy::Foreground
    );
    assert_eq!(invocations[0].status, ToolInvocationStatus::Failed);
    assert_eq!(
        invocations[0].error.as_ref().unwrap().code,
        "tool_not_found"
    );

    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert!(items.iter().any(|item| matches!(
        &item.payload,
        ConversationEntryPayload::ToolCall(call)
            if call.runtime_tool_name == "missing_tool"
    )));
    assert!(items.iter().any(|item| matches!(
        &item.payload,
        ConversationEntryPayload::ToolResult(result)
            if result.is_error
                && result.content[0]
                    .search_text()
                    .is_some_and(|text| text == "No tool named missing_tool exists")
    )));
}

#[tokio::test]
async fn recoverable_missing_runtime_tool_is_returned_to_model() {
    let fixture = Fixture::new("recoverable-missing-runtime");
    let request = fixture.request();
    let agent_run = fixture
        .repo
        .insert_agent_run(new_agent_run_input(&request))
        .unwrap();
    let definition = RegisteredToolDefinition {
        source: ToolSource::Local,
        namespace: None,
        tool_name: "orphan_tool".to_string(),
        runtime_tool_name: "orphan_tool".to_string(),
        description: "Registered definition without executor".to_string(),
        parameters: json!({"type": "object"}),
        policy: ToolRunPolicy {
            approval_policy: ToolApprovalPolicy::Never,
            execution_policy: ToolExecutionPolicy::Foreground,
            timeout_ms: None,
        },
    };
    let context = PersistenceContext::new(
        fixture.repo.clone(),
        agent_run.id.clone(),
        fixture.conversation.id.clone(),
        fixture.provider.id.clone(),
        fixture.model.model_id.clone(),
        request.settings_snapshot.clone(),
        vec![fixture.user_item.id.clone()],
        vec![definition],
        Vec::new(),
        request.guards.max_tool_calls,
        request.guards.repeated_tool_call_limit,
        request.cancellation_token.clone(),
        None,
        None,
    );
    let hook = context.hook();

    let action = PromptHook::<MockCompletionModel>::on_tool_call(
        &hook,
        "orphan_tool",
        Some("call_1".to_string()),
        "internal_1",
        "{}",
    )
    .await;

    let reason = match action {
        ToolCallHookAction::Skip { reason } => reason,
        other => panic!("expected recoverable skip, got {other:?}"),
    };
    assert_eq!(reason, "Tool orphan_tool has no runtime executor");
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].status, ToolInvocationStatus::Failed);
    assert_eq!(
        invocations[0].error.as_ref().unwrap().code,
        "tool_runtime_unavailable"
    );
    assert!(invocations[0].output.as_ref().unwrap().is_error);
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.payload, ConversationEntryPayload::ToolCall(_)))
            .count(),
        1
    );
    assert_eq!(
        items
            .iter()
            .filter(|item| matches!(item.payload, ConversationEntryPayload::ToolResult(_)))
            .count(),
        1
    );
}

#[tokio::test]
async fn max_turns_is_persisted_as_max_steps_stop() {
    let fixture = Fixture::new("max-steps");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.guards.max_steps = 1;
    request
        .tool_registry
        .register_local_tool(EchoTool::new(ToolApprovalPolicy::Never))
        .unwrap();
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "echo", json!({"text": "one"})),
        MockTurn::tool_call("call_2", "echo", json!({"text": "two"})),
        MockTurn::tool_call("call_3", "echo", json!({"text": "three"})),
    ]);

    let handle = runtime.run_with_model(request, model).await.unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert_eq!(handle.agent_run.error, None);
    assert_eq!(
        handle.output.as_ref().unwrap().stopped_reason,
        AgentStoppedReason::MaxSteps
    );
    assert!(
            handle
                .events
                .iter()
                .any(|event| matches!(event, AgentRunEvent::Completed { output } if output.stopped_reason == AgentStoppedReason::MaxSteps))
        );
    assert!(
        !handle
            .events
            .iter()
            .any(|event| matches!(event, AgentRunEvent::Failed { .. }))
    );
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 3);
    assert!(
        invocations
            .iter()
            .all(|invocation| invocation.status == ToolInvocationStatus::Succeeded)
    );
    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let final_entry_id = &handle.output.as_ref().unwrap().final_entry_id;
    let final_entry = items
        .iter()
        .find(|item| &item.id == final_entry_id)
        .expect("max-steps run final entry");
    assert!(matches!(
        final_entry.payload,
        ConversationEntryPayload::Status(ConversationStatusEntry {
            code: ConversationStatusCode::MaxStepsReached,
            message: None,
        })
    ));
}

#[tokio::test]
async fn prompt_error_fails_active_tool_invocations() {
    let fixture = Fixture::new("tool-failure");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let agent_run = fixture
        .repo
        .insert_agent_run(new_agent_run_input(&fixture.request()))
        .unwrap();
    let invocation = fixture
        .repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run.id.clone(),
            provider_step_id: None,
            status: ToolInvocationStatus::Running,
            input: ToolInvocationInput {
                source: ToolSource::Local,
                namespace: None,
                tool_name: "echo".to_string(),
                runtime_tool_name: "echo".to_string(),
                call_id: "call_1".to_string(),
                arguments: ToolArguments {
                    value: json!({"text": "hi"}),
                },
                approval_policy: ToolApprovalPolicy::Never,
                execution_policy: ToolExecutionPolicy::Foreground,
            },
            output: None,
            error: None,
        })
        .unwrap();

    runtime
        .finalize_active_tool_invocations(
            &agent_run.id,
            &fixture.conversation.id,
            ToolInvocationStatus::Failed,
            run_error("prompt_error", "tool failed", true, None),
        )
        .unwrap();

    let invocation = fixture
        .repo
        .get_tool_invocation(&invocation.id)
        .unwrap()
        .unwrap();
    assert_eq!(invocation.status, ToolInvocationStatus::Failed);
    let error = invocation.error.as_ref().unwrap();
    assert_eq!(error.code, "prompt_error");
    assert_eq!(error.message, "tool failed");
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);

    let tool_results = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item.payload {
            ConversationEntryPayload::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(tool_results.len(), 1);
    assert_eq!(tool_results[0].call_id, "call_1");
    assert!(tool_results[0].is_error);
}

#[tokio::test]
async fn rmcp_tool_call_is_registered_and_persisted_with_source_server() {
    let fixture = Fixture::new("mcp-tool-run");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mcp_service = start_mcp_server(vec![make_mcp_tool("mcp_echo", "Echo over MCP")]).await;
    let tools = mcp_service.peer().list_all_tools().await.unwrap();

    let mut request = fixture.request();
    McpConnector::new()
        .register_rmcp_tools(
            &mut request.tool_registry,
            "test-server",
            tools,
            mcp_service.peer().clone(),
            ToolApprovalPolicy::Never,
            ToolExecutionPolicy::Foreground,
        )
        .unwrap();
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "mcp_echo", json!({"text": "hi"})),
        MockTurn::text("done"),
    ]);

    let handle = runtime.run_with_model(request, model).await.unwrap();
    let invocations = fixture
        .repo
        .tool_invocations_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(
        invocations[0].source,
        ToolSource::Mcp {
            server_id: "test-server".to_string(),
        }
    );
    assert_eq!(invocations[0].tool_name, "mcp_echo");
    assert_eq!(invocations[0].runtime_tool_name, "mcp_echo");
    assert_eq!(invocations[0].status, ToolInvocationStatus::Succeeded);
}

#[tokio::test]
async fn approval_policy_waits_inside_same_run_until_broker_decision() {
    let fixture = Fixture::new("approval");
    let broker = Arc::new(ManualApprovalBroker::default());
    let runtime = AgentRuntime::new(fixture.repo.clone()).with_approval_broker(broker.clone());
    let mut request = fixture.request();
    request
        .tool_registry
        .register_local_tool(EchoTool::new(ToolApprovalPolicy::OnRequest))
        .unwrap();
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "echo", json!({"text": "hi"})),
        MockTurn::text("done"),
    ]);

    let repo = fixture.repo.clone();
    let conversation_id = fixture.conversation.id.clone();
    let mut run = tokio::spawn(async move { runtime.run_with_model(request, model).await });
    let approval_request = broker.wait_for_request().await;

    let invocations = repo
        .tool_invocations_for_run(&approval_request.agent_run_id)
        .unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(
        invocations[0].status,
        ToolInvocationStatus::AwaitingApproval
    );
    assert_eq!(
        invocations[0]
            .approval
            .as_ref()
            .map(|approval| approval.status),
        Some(ApprovalStatus::Pending)
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(25), &mut run)
            .await
            .is_err()
    );

    broker.resolve_next(ToolApprovalDecision::Approved {
        decided_by: "user".to_string(),
        reason: Some("ok".to_string()),
    });
    let handle = run.await.unwrap().unwrap();

    assert_eq!(handle.agent_run.id, approval_request.agent_run_id);
    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert!(matches!(handle.status, AgentRunHandleStatus::Finished));
    assert_eq!(
        repo.agent_runs_for_conversation(&conversation_id)
            .unwrap()
            .len(),
        1
    );

    let invocation = repo
        .get_tool_invocation(&approval_request.tool_invocation_id)
        .unwrap()
        .unwrap();
    assert_eq!(invocation.status, ToolInvocationStatus::Succeeded);
    assert_eq!(
        invocation.approval.as_ref().map(|approval| approval.status),
        Some(ApprovalStatus::Approved)
    );
    let entries = repo
        .conversation_entries(&conversation_id)
        .unwrap()
        .into_iter()
        .filter(|entry| {
            entry.agent_run_id.as_deref() == Some(approval_request.agent_run_id.as_str())
        })
        .collect::<Vec<_>>();
    let approval_entries = entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.payload,
                ConversationEntryPayload::ApprovalRequest(_)
                    | ConversationEntryPayload::ApprovalDecision(_)
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(approval_entries.len(), 2);
    assert!(matches!(
        approval_entries[0].payload,
        ConversationEntryPayload::ApprovalRequest(_)
    ));
    assert!(matches!(
        approval_entries[1].payload,
        ConversationEntryPayload::ApprovalDecision(_)
    ));
    assert!(approval_entries[0].seq < approval_entries[1].seq);
}

#[tokio::test]
async fn denied_approval_writes_error_tool_result_and_continues_same_run() {
    let fixture = Fixture::new("approval-denied");
    let broker = Arc::new(ManualApprovalBroker::default());
    let runtime = AgentRuntime::new(fixture.repo.clone()).with_approval_broker(broker.clone());
    let mut request = fixture.request();
    request
        .tool_registry
        .register_local_tool(EchoTool::new(ToolApprovalPolicy::OnRequest))
        .unwrap();
    let model = MockCompletionModel::new([
        MockTurn::tool_call("call_1", "echo", json!({"text": "hi"})),
        MockTurn::text("handled denial"),
    ]);

    let repo = fixture.repo.clone();
    let conversation_id = fixture.conversation.id.clone();
    let run = tokio::spawn(async move { runtime.run_with_model(request, model).await });
    let approval_request = broker.wait_for_request().await;
    broker.resolve_next(ToolApprovalDecision::Denied {
        decided_by: "user".to_string(),
        reason: Some("not allowed".to_string()),
    });
    let handle = run.await.unwrap().unwrap();

    assert_eq!(handle.agent_run.id, approval_request.agent_run_id);
    assert_eq!(handle.agent_run.status, AgentRunStatus::Completed);
    assert_eq!(handle.agent_run.error, None);
    assert_eq!(
        repo.agent_runs_for_conversation(&conversation_id)
            .unwrap()
            .len(),
        1
    );

    let invocation = repo
        .get_tool_invocation(&approval_request.tool_invocation_id)
        .unwrap()
        .unwrap();
    assert_eq!(invocation.status, ToolInvocationStatus::Denied);
    assert_eq!(
        invocation.approval.as_ref().map(|approval| approval.status),
        Some(ApprovalStatus::Denied)
    );
    assert_eq!(
        invocation.error.as_ref().unwrap().code,
        "tool_approval_denied"
    );
    assert!(invocation.output.as_ref().unwrap().is_error);
    assert_eq!(tool_result_texts(&fixture), vec!["not allowed".to_string()]);
    let entries = repo
        .conversation_entries(&conversation_id)
        .unwrap()
        .into_iter()
        .filter(|entry| {
            entry.agent_run_id.as_deref() == Some(approval_request.agent_run_id.as_str())
        })
        .collect::<Vec<_>>();
    let approval_decision = entries
        .iter()
        .find(|entry| matches!(entry.payload, ConversationEntryPayload::ApprovalDecision(_)))
        .expect("denial decision entry");
    let tool_result = entries
        .iter()
        .find(|entry| matches!(entry.payload, ConversationEntryPayload::ToolResult(_)))
        .expect("denial tool result entry");
    assert!(approval_decision.seq < tool_result.seq);
}

#[test]
fn recovery_fails_active_child_execution_rows() {
    let fixture = Fixture::new("recovery-children");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let agent_run = insert_agent_run_with_status(&fixture, AgentRunStatus::Running);
    let provider_step = insert_provider_step(&fixture, &agent_run.id, ProviderStepStatus::Running);
    let invocation = insert_tool_invocation(
        &fixture,
        &agent_run.id,
        Some(provider_step.id.clone()),
        ToolInvocationStatus::Running,
    );

    let recovered = runtime.recover_interrupted_runs().unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, AgentRunStatus::Failed);
    assert_eq!(recovered[0].error.as_ref().unwrap().code, "interrupted");
    let final_entry_id = &recovered[0].output.as_ref().unwrap().final_entry_id;
    let entries = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let final_entry = entries
        .iter()
        .find(|entry| &entry.id == final_entry_id)
        .expect("recovery final error entry");
    assert!(matches!(
        &final_entry.payload,
        ConversationEntryPayload::Error(error) if error.code == "interrupted"
    ));
    assert_eq!(final_entry.status, ConversationEntryStatus::Failed);
    let recovered_again = runtime.recover_interrupted_runs().unwrap();
    assert!(recovered_again.is_empty());
    assert_eq!(
        fixture
            .repo
            .conversation_entries(&fixture.conversation.id)
            .unwrap()
            .iter()
            .filter(|entry| matches!(entry.payload, ConversationEntryPayload::Error(_)))
            .count(),
        1
    );

    let provider_step = fixture
        .repo
        .get_provider_step(&provider_step.id)
        .unwrap()
        .unwrap();
    assert_eq!(provider_step.status, ProviderStepStatus::Failed);
    assert_eq!(provider_step.error.as_ref().unwrap().code, "interrupted");
    let invocation = fixture
        .repo
        .get_tool_invocation(&invocation.id)
        .unwrap()
        .unwrap();
    assert_eq!(invocation.status, ToolInvocationStatus::Failed);
    assert_eq!(invocation.error.as_ref().unwrap().code, "interrupted");
    assert_eq!(
        tool_result_texts(&fixture),
        vec!["agent run was interrupted before reaching a terminal state".to_string()]
    );
}

#[test]
fn recovery_fails_waiting_approval_runs() {
    let fixture = Fixture::new("recovery-waiting");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let (agent_run, invocation) = insert_waiting_approval(&fixture);

    let recovered = runtime.recover_interrupted_runs().unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].status, AgentRunStatus::Failed);
    assert_eq!(recovered[0].error.as_ref().unwrap().code, "interrupted");

    let agent_run = fixture.repo.get_agent_run(&agent_run.id).unwrap().unwrap();
    assert_eq!(agent_run.status, AgentRunStatus::Failed);
    assert_eq!(agent_run.error.as_ref().unwrap().code, "interrupted");
    let invocation = fixture
        .repo
        .get_tool_invocation(&invocation.id)
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
        tool_result_texts(&fixture),
        vec!["agent run was interrupted before reaching a terminal state".to_string()]
    );
}

#[test]
fn cancel_running_run_terminalizes_active_children_without_run_error() {
    let fixture = Fixture::new("cancel-running");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let agent_run = insert_agent_run_with_status(&fixture, AgentRunStatus::Running);
    let provider_step = insert_provider_step(&fixture, &agent_run.id, ProviderStepStatus::Running);
    let invocation = insert_tool_invocation(
        &fixture,
        &agent_run.id,
        Some(provider_step.id.clone()),
        ToolInvocationStatus::Running,
    );
    let assistant_item = fixture
        .repo
        .append_conversation_entry(NewConversationEntry {
            conversation_id: fixture.conversation.id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: Some(agent_run.id.clone()),
            provider_step_id: Some(provider_step.id.clone()),
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::Assistant,
                content: vec![ContentPart::Text {
                    text: "partial answer".to_string(),
                }],
            },
        })
        .unwrap();
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observer = AgentRuntimeObserver::new({
        let events = events.clone();
        move |event| {
            events.lock().unwrap().push(event);
        }
    });

    let canceled = runtime
        .cancel_run(&agent_run.id, Some(&observer))
        .unwrap()
        .unwrap();

    assert_eq!(canceled.status, AgentRunStatus::Canceled);
    assert!(canceled.error.is_none());
    assert_eq!(
        canceled.output.as_ref().unwrap().stopped_reason,
        AgentStoppedReason::Canceled
    );
    assert_eq!(
        Some(canceled.output.as_ref().unwrap().final_entry_id.as_str()),
        Some(assistant_item.id.as_str())
    );
    assert_eq!(
        *events.lock().unwrap(),
        vec![AgentRuntimeEvent::AgentRunStatusChanged {
            agent_run_id: agent_run.id.clone(),
            status: AgentRunStatus::Canceled,
        }]
    );

    let provider_step = fixture
        .repo
        .get_provider_step(&provider_step.id)
        .unwrap()
        .unwrap();
    assert_eq!(provider_step.status, ProviderStepStatus::Canceled);
    assert_eq!(provider_step.error.as_ref().unwrap().code, "canceled");
    let invocation = fixture
        .repo
        .get_tool_invocation(&invocation.id)
        .unwrap()
        .unwrap();
    assert_eq!(invocation.status, ToolInvocationStatus::Canceled);
    assert_eq!(invocation.error.as_ref().unwrap().code, "canceled");
    assert_eq!(tool_result_texts(&fixture), vec!["runtime canceled"]);
}

#[test]
fn late_provider_and_tool_completion_after_cancel_do_not_override_terminal_state() {
    let fixture = Fixture::new("cancel-late-finish");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let agent_run = insert_agent_run_with_status(&fixture, AgentRunStatus::Running);
    let provider_step = insert_provider_step(&fixture, &agent_run.id, ProviderStepStatus::Running);
    let invocation = insert_tool_invocation(
        &fixture,
        &agent_run.id,
        Some(provider_step.id.clone()),
        ToolInvocationStatus::Running,
    );

    runtime.cancel_run(&agent_run.id, None).unwrap().unwrap();

    let provider_step = fixture
        .repo
        .update_provider_step_status(
            &provider_step.id,
            UpdateProviderStepStatus {
                status: ProviderStepStatus::Completed,
                response_snapshot: None,
                state_snapshot: None,
                error: None,
            },
        )
        .unwrap();
    assert_eq!(provider_step.status, ProviderStepStatus::Canceled);
    assert_eq!(provider_step.error.as_ref().unwrap().code, "canceled");

    let invocation = fixture
        .repo
        .update_tool_invocation_status(
            &invocation.id,
            UpdateToolInvocationStatus {
                status: ToolInvocationStatus::Succeeded,
                output: Some(ToolInvocationOutput {
                    content: vec![ContentPart::Text {
                        text: "late success".to_string(),
                    }],
                    structured_output: None,
                    raw_output: None,
                    is_error: false,
                }),
                error: None,
            },
        )
        .unwrap();
    assert_eq!(invocation.status, ToolInvocationStatus::Canceled);
    assert_eq!(invocation.error.as_ref().unwrap().code, "canceled");
    assert_eq!(tool_result_texts(&fixture), vec!["runtime canceled"]);
}

#[test]
fn cancel_waiting_approval_cancels_pending_approval() {
    let fixture = Fixture::new("cancel-waiting-approval");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let (agent_run, invocation) = insert_waiting_approval(&fixture);

    let canceled = runtime.cancel_run(&agent_run.id, None).unwrap().unwrap();

    assert_eq!(canceled.status, AgentRunStatus::Canceled);
    assert!(canceled.error.is_none());
    let invocation = fixture
        .repo
        .get_tool_invocation(&invocation.id)
        .unwrap()
        .unwrap();
    assert_eq!(invocation.status, ToolInvocationStatus::Canceled);
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
    assert_eq!(invocation.error.as_ref().unwrap().code, "canceled");
    assert_eq!(tool_result_texts(&fixture), vec!["runtime canceled"]);
}

#[test]
fn cancel_terminal_run_is_idempotent() {
    let fixture = Fixture::new("cancel-terminal");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let agent_run = insert_agent_run_with_status(&fixture, AgentRunStatus::Completed);
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observer = AgentRuntimeObserver::new({
        let events = events.clone();
        move |event| {
            events.lock().unwrap().push(event);
        }
    });

    let unchanged = runtime
        .cancel_run(&agent_run.id, Some(&observer))
        .unwrap()
        .unwrap();

    assert_eq!(unchanged.status, AgentRunStatus::Completed);
    assert!(events.lock().unwrap().is_empty());
    assert!(tool_result_texts(&fixture).is_empty());
}

#[tokio::test]
async fn setup_failure_marks_agent_run_failed() {
    let fixture = Fixture::new("setup-failure");
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.project_root = Some(fixture.dir.path().to_path_buf());
    request.skill_requests = vec![crate::SkillActivationRequest::new("missing-skill")];

    let error = runtime
        .run_with_model(request, MockCompletionModel::text("unused"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("missing-skill"));

    assert!(
        fixture
            .repo
            .agent_runs_by_status(AgentRunStatus::Running)
            .unwrap()
            .is_empty()
    );
    let failed = fixture
        .repo
        .agent_runs_by_status(AgentRunStatus::Failed)
        .unwrap();
    assert_eq!(failed.len(), 1);
    let payload = failed[0].error.as_ref().unwrap();
    assert_eq!(payload.code, "setup_error");
    assert!(payload.message.contains("missing-skill"));
    assert!(payload.retryable);
    let final_entry_id = &failed[0].output.as_ref().unwrap().final_entry_id;
    let entries = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let final_entry = entries
        .iter()
        .find(|entry| &entry.id == final_entry_id)
        .expect("setup failure final entry");
    assert!(matches!(
        &final_entry.payload,
        ConversationEntryPayload::Error(error) if error.code == "setup_error"
    ));
}

#[tokio::test]
async fn saved_provider_setup_failure_records_failed_run_and_error_item() {
    let fixture = Fixture::new("saved-provider-setup-failure");
    let runtime = AgentRuntime::new(fixture.repo.clone());

    let handle = runtime
        .run_with_saved_provider_observed(
            fixture.request(),
            fixture.provider.clone(),
            ProviderSecretValues::default(),
            None,
        )
        .await
        .unwrap();

    assert_eq!(handle.agent_run.status, AgentRunStatus::Failed);
    let error = handle.agent_run.error.as_ref().unwrap();
    assert_eq!(error.code, "setup_error");
    assert!(error.message.contains("missing provider secret `api_key`"));
    assert!(error.retryable);
    assert!(matches!(
        handle.events.as_slice(),
        [AgentRunEvent::Failed { error }] if error.code == "setup_error"
    ));

    let output = handle.output.as_ref().unwrap();
    assert_eq!(output.stopped_reason, AgentStoppedReason::Failed);
    let final_entry_id = &output.final_entry_id;
    assert_eq!(
        handle.steps,
        vec![AgentStep::ConversationEntry(final_entry_id.clone())]
    );

    let timeline = fixture
        .repo
        .conversation_timeline_records(&fixture.conversation.id)
        .unwrap()
        .unwrap();
    let error_item = timeline
        .items
        .iter()
        .find(|item| item.id == *final_entry_id)
        .unwrap();
    assert_eq!(
        error_item.agent_run_id.as_deref(),
        Some(handle.agent_run.id.as_str())
    );
    assert!(matches!(
        &error_item.payload,
        ConversationEntryPayload::Error(payload)
            if payload.code == "setup_error"
                && payload.message.contains("missing provider secret `api_key`")
    ));
}

#[tokio::test]
async fn skill_activation_is_persisted_as_snapshot() {
    let fixture = Fixture::new("skills");
    let skill_dir = fixture.dir.path().join(".agents/skills/rust");
    std::fs::create_dir_all(&skill_dir).unwrap();
    let skill_file = skill_dir.join("SKILL.md");
    std::fs::write(
        &skill_file,
        "---\nname: rust\ndescription: Rust workflow\n---\nUse cargo test.\n",
    )
    .unwrap();

    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.project_root = Some(fixture.dir.path().to_path_buf());
    request.skill_requests = vec![crate::SkillActivationRequest::new("rust")];
    let model = MockCompletionModel::text("ok");
    let handle = runtime
        .run_with_model(request, model.clone())
        .await
        .unwrap();
    std::fs::write(&skill_file, "---\nname: rust\n---\nUse cargo clippy.\n").unwrap();

    let items = fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap();
    let skill = items
        .iter()
        .find_map(|item| match &item.payload {
            ConversationEntryPayload::SkillActivation(skill) => Some(skill),
            _ => None,
        })
        .unwrap();
    assert_eq!(skill.name, "rust");
    assert_eq!(
        skill.content,
        vec![ContentPart::Text {
            text: "---\nname: rust\ndescription: Rust workflow\n---\nUse cargo test.\n".to_string(),
        }]
    );
    let skill_item = items
        .iter()
        .find(|item| matches!(item.payload, ConversationEntryPayload::SkillActivation(_)))
        .unwrap();
    let provider_steps = fixture
        .repo
        .provider_steps_for_run(&handle.agent_run.id)
        .unwrap();
    assert_eq!(provider_steps.len(), 1);
    assert_eq!(
        provider_steps[0].request_snapshot.input_item_ids,
        vec![fixture.user_item.id.clone(), skill_item.id.clone()]
    );

    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    let messages = requests[0].chat_history.iter().collect::<Vec<_>>();
    let last_message_text = rig_message_text(messages.last().unwrap());
    assert!(last_message_text.starts_with("hello\n<skill>\n<name>rust</name>"));
    assert!(last_message_text.contains("Use cargo test."));
    assert!(
        messages[..messages.len() - 1]
            .iter()
            .all(|message| !rig_message_text(message).contains("<skill>"))
    );
}

#[tokio::test]
async fn tool_history_replay_preserves_provider_call_ids() {
    let fixture = Fixture::new("tool-history");
    fixture
        .repo
        .append_conversation_entry(NewConversationEntry {
            conversation_id: fixture.conversation.id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::ToolCall(ToolCallEntry {
                tool_invocation_id: None,
                call_id: "call_previous".to_string(),
                source: ToolSource::Local,
                name: "echo".to_string(),
                runtime_tool_name: "echo".to_string(),
                arguments: ToolArguments {
                    value: json!({"text": "hi"}),
                },
            }),
        })
        .unwrap();
    fixture
        .repo
        .append_conversation_entry(NewConversationEntry {
            conversation_id: fixture.conversation.id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::ToolResult(ToolResultEntry {
                tool_invocation_id: None,
                call_id: "call_previous".to_string(),
                content: vec![ContentPart::Text {
                    text: "hi".to_string(),
                }],
                is_error: false,
                structured_output: None,
                raw_output: None,
            }),
        })
        .unwrap();
    let next_user_item = fixture
        .repo
        .append_conversation_entry(NewConversationEntry {
            conversation_id: fixture.conversation.id.clone(),
            status: ConversationEntryStatus::Completed,
            agent_run_id: None,
            provider_step_id: None,
            tool_invocation_id: None,
            provider_item_id: None,
            payload: ConversationEntryPayload::Message {
                role: TranscriptRole::User,
                content: vec![ContentPart::Text {
                    text: "continue".to_string(),
                }],
            },
        })
        .unwrap();
    let runtime = AgentRuntime::new(fixture.repo.clone());
    let mut request = fixture.request();
    request.trigger_entry_id = next_user_item.id;
    let model = MockCompletionModel::text("ok");

    runtime
        .run_with_model(request, model.clone())
        .await
        .unwrap();

    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    let messages = requests[0].chat_history.iter().collect::<Vec<_>>();
    let tool_call = messages
        .iter()
        .find_map(|message| match message {
            RigMessage::Assistant { content, .. } => {
                content.iter().find_map(|content| match content {
                    AssistantContent::ToolCall(call) => Some(call),
                    _ => None,
                })
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(tool_call.id, "call_previous");
    assert_eq!(tool_call.call_id.as_deref(), Some("call_previous"));

    let tool_result = messages
        .iter()
        .find_map(|message| match message {
            RigMessage::User { content } => content.iter().find_map(|content| match content {
                UserContent::ToolResult(result) => Some(result),
                _ => None,
            }),
            _ => None,
        })
        .unwrap();
    assert_eq!(tool_result.id, "call_previous");
    assert_eq!(tool_result.call_id.as_deref(), Some("call_previous"));
}

fn insert_waiting_approval(fixture: &Fixture) -> (AgentRunRecord, ToolInvocationRecord) {
    let agent_run = insert_agent_run_with_status(fixture, AgentRunStatus::Running);
    let invocation = insert_tool_invocation(
        fixture,
        &agent_run.id,
        None,
        ToolInvocationStatus::AwaitingApproval,
    );
    let request = ApprovalRequestPayload {
        reason: "approve echo".to_string(),
        tool_source: ToolSource::Local,
        tool_name: "echo".to_string(),
        arguments_preview: "{\"text\":\"hi\"}".to_string(),
        access_requests: Vec::new(),
    };
    let invocation = fixture
        .repo
        .request_tool_invocation_approval_with_entry(
            &invocation.id,
            NewToolInvocationApproval {
                request: request.clone(),
                expires_at: None,
            },
            NewConversationEntry {
                conversation_id: fixture.conversation.id.clone(),
                status: ConversationEntryStatus::WaitingForApproval,
                agent_run_id: Some(agent_run.id.clone()),
                provider_step_id: None,
                tool_invocation_id: Some(invocation.id.clone()),
                provider_item_id: None,
                payload: ConversationEntryPayload::ApprovalRequest(ApprovalRequestEntry {
                    tool_invocation_id: invocation.id.clone(),
                    request,
                }),
            },
        )
        .unwrap()
        .1;
    (agent_run, invocation)
}

fn insert_agent_run_with_status(fixture: &Fixture, status: AgentRunStatus) -> AgentRunRecord {
    let agent_run = fixture
        .repo
        .insert_agent_run(new_agent_run_input(&fixture.request()))
        .unwrap();
    if is_terminal_agent_run_status(status) {
        let (stopped_reason, code) = match status {
            AgentRunStatus::Completed => (
                AgentStoppedReason::Completed,
                ConversationStatusCode::CompletedWithoutOutput,
            ),
            AgentRunStatus::Canceled => (
                AgentStoppedReason::Canceled,
                ConversationStatusCode::Canceled,
            ),
            AgentRunStatus::Failed => (
                AgentStoppedReason::Failed,
                ConversationStatusCode::CompletedWithoutOutput,
            ),
            AgentRunStatus::Queued | AgentRunStatus::Running => unreachable!(),
        };
        return fixture
            .repo
            .finish_agent_run(
                &agent_run.id,
                FinishAgentRun {
                    status,
                    stopped_reason,
                    error: None,
                    final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                        conversation_id: fixture.conversation.id.clone(),
                        status: ConversationEntryStatus::Completed,
                        agent_run_id: Some(agent_run.id.clone()),
                        provider_step_id: None,
                        tool_invocation_id: None,
                        provider_item_id: None,
                        payload: ConversationEntryPayload::Status(ConversationStatusEntry {
                            code,
                            message: None,
                        }),
                    })),
                },
            )
            .unwrap()
            .run;
    }
    fixture
        .repo
        .update_agent_run_status(
            &agent_run.id,
            UpdateAgentRunStatus {
                status,
                error: None,
            },
        )
        .unwrap()
}

fn insert_provider_step(
    fixture: &Fixture,
    agent_run_id: &str,
    status: ProviderStepStatus,
) -> ProviderStepRecord {
    fixture
        .repo
        .insert_provider_step(NewProviderStep {
            agent_run_id: agent_run_id.to_string(),
            seq: fixture.repo.next_provider_step_seq(agent_run_id).unwrap(),
            status,
            request_snapshot: ProviderStepRequestSnapshot {
                provider_id: fixture.provider.id.clone(),
                model_id: fixture.model.model_id.clone(),
                input_item_ids: vec![fixture.user_item.id.clone()],
                snapshot_kind: ProviderStepSnapshotKind::RigCompletionRequest,
                request_body: ProviderRawPayload {
                    provider_kind: "test".to_string(),
                    value: json!({"messages": ["hello"]}),
                },
            },
            response_snapshot: None,
            state_snapshot: None,
            settings_snapshot: run_settings(&fixture.provider.id, &fixture.model.model_id),
            error: None,
        })
        .unwrap()
}

fn insert_tool_invocation(
    fixture: &Fixture,
    agent_run_id: &str,
    provider_step_id: Option<ProviderStepId>,
    status: ToolInvocationStatus,
) -> ToolInvocationRecord {
    fixture
        .repo
        .insert_tool_invocation(NewToolInvocation {
            agent_run_id: agent_run_id.to_string(),
            provider_step_id,
            status,
            input: ToolInvocationInput {
                source: ToolSource::Local,
                namespace: None,
                tool_name: "echo".to_string(),
                runtime_tool_name: "echo".to_string(),
                call_id: "call_approval".to_string(),
                arguments: ToolArguments {
                    value: json!({"text": "hi"}),
                },
                approval_policy: ToolApprovalPolicy::OnRequest,
                execution_policy: ToolExecutionPolicy::Foreground,
            },
            output: None,
            error: None,
        })
        .unwrap()
}

fn tool_result_texts(fixture: &Fixture) -> Vec<String> {
    fixture
        .repo
        .conversation_entries(&fixture.conversation.id)
        .unwrap()
        .into_iter()
        .filter_map(|item| match item.payload {
            ConversationEntryPayload::ToolResult(result) => {
                Some(result.content.into_iter().filter_map(|part| match part {
                    ContentPart::Text { text } => Some(text),
                    _ => None,
                }))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

struct Fixture {
    dir: TempDir,
    db_path: PathBuf,
    repo: FreshRepository,
    conversation: ConversationRecord,
    provider: ProviderRecord,
    model: ProviderModelRecord,
    user_item: ConversationEntryRecord,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let store = FreshStore::open_in_dir(dir.path()).unwrap();
        let db_path = store.path().to_path_buf();
        let repo = store.repository();
        let project = repo
            .insert_project(NewProject {
                path: dir.path().to_string_lossy().to_string(),
                display_name: name.to_string(),
                kind: ProjectKind::Normal,
                pinned: false,
                removed: false,
                metadata: ProjectMetadata {
                    scratch_reason: None,
                    git_root: Some(dir.path().to_string_lossy().to_string()),
                    last_active_conversation_id: None,
                },
            })
            .unwrap();
        let provider = repo
            .insert_provider(NewProvider {
                kind: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                enabled: true,
                settings: provider_settings(),
                secret_refs: ProviderSecretRefs { refs: Vec::new() },
            })
            .unwrap();
        let model = repo
            .upsert_provider_model(NewProviderModel {
                provider_id: provider.id.clone(),
                model_id: "gpt-5.2".to_string(),
                display_name: Some("GPT-5.2".to_string()),
                enabled: true,
                capabilities: model_capabilities(),
                metadata: ProviderModelMetadata {
                    display_name: Some("GPT-5.2".to_string()),
                    family: Some("gpt".to_string()),
                    raw: None,
                },
            })
            .unwrap();
        let conversation = repo
            .insert_conversation(NewConversation {
                project_id: project.id,
                title: name.to_string(),
                pinned: false,
                prompt_id: None,
                default_provider_id: Some(provider.id.clone()),
                default_model_id: Some(model.model_id.clone()),
                metadata: ConversationMetadata {
                    summary: None,
                    tags: Vec::new(),
                },
                settings_snapshot: ConversationSettingsSnapshot {
                    prompt: None,
                    provider_id: Some(provider.id.clone()),
                    model_id: Some(model.model_id.clone()),
                    model_capabilities: Some(model_capabilities()),
                    tool_policy: ToolPolicySnapshot {
                        approval_policy: ToolApprovalPolicy::Never,
                        enabled_sources: vec![ToolSource::Local],
                        max_steps: 8,
                        approval_mode: ToolApprovalMode::RequestApproval,
                        permission_scope: None,
                    },
                },
            })
            .unwrap();
        let user_item = repo
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
        Self {
            dir,
            db_path,
            repo,
            conversation,
            provider,
            model,
            user_item,
        }
    }

    fn request(&self) -> AgentRunRequest {
        self.request_with_streaming(false)
    }

    fn streaming_request(&self) -> AgentRunRequest {
        self.request_with_streaming(true)
    }

    fn request_with_streaming(&self, streaming: bool) -> AgentRunRequest {
        let mut settings = run_settings(&self.provider.id, &self.model.model_id);
        settings.model_capabilities.streaming = streaming;
        AgentRunRequest::new(
            self.conversation.id.clone(),
            self.user_item.id.clone(),
            self.provider.id.clone(),
            self.model.model_id.clone(),
            settings,
            AgentRuntimeSnapshot {
                engine: AgentEngineKind::Rig,
                engine_version: "0.37.0".to_string(),
                skill_catalog_hash: None,
                tool_name_strategy: ToolNameStrategy::Namespaced,
            },
        )
    }
}

#[derive(Clone)]
struct FailBeforeFirstTokenModel;

impl CompletionModel for FailBeforeFirstTokenModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        Err(CompletionError::ProviderError(
            "forced provider-open failure".to_string(),
        ))
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        Err(CompletionError::ProviderError(
            "forced provider-open failure".to_string(),
        ))
    }
}

#[derive(Clone)]
struct FailAfterTextModel;

impl CompletionModel for FailAfterTextModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        Err(CompletionError::ProviderError(
            "forced mid-stream failure".to_string(),
        ))
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        let stream: StreamingResult<Self::StreamingResponse> = Box::pin(futures::stream::iter([
            Ok(RawStreamingChoice::Message("partial".to_string())),
            Err(CompletionError::ProviderError(
                "forced mid-stream failure".to_string(),
            )),
        ]));
        Ok(StreamingCompletionResponse::stream(stream))
    }
}

#[derive(Clone)]
struct ReasoningStreamModel;

impl CompletionModel for ReasoningStreamModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        Err(CompletionError::ProviderError(
            "reasoning stream model only supports streaming".to_string(),
        ))
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        let mut usage = Usage::new();
        usage.total_tokens = 3;
        let stream: StreamingResult<Self::StreamingResponse> = Box::pin(futures::stream::iter([
            Ok(RawStreamingChoice::ReasoningDelta {
                id: Some("reasoning_1".to_string()),
                reasoning: "thinking ".to_string(),
            }),
            Ok(RawStreamingChoice::ReasoningDelta {
                id: Some("reasoning_1".to_string()),
                reasoning: "now".to_string(),
            }),
            Ok(RawStreamingChoice::FinalResponse(MockResponse::with_usage(
                usage,
            ))),
        ]));
        Ok(StreamingCompletionResponse::stream(stream))
    }
}

#[derive(Clone)]
struct DelayedFinalStreamModel {
    delay: Duration,
}

impl CompletionModel for DelayedFinalStreamModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self {
            delay: Duration::from_millis(0),
        }
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        Err(CompletionError::ProviderError(
            "delayed-final stream model only supports streaming".to_string(),
        ))
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        let delay = self.delay;
        let stream = futures::stream::unfold(false, move |finished| async move {
            if finished {
                None
            } else {
                tokio::time::sleep(delay).await;
                let mut usage = Usage::new();
                usage.total_tokens = 11;
                Some((
                    Ok(RawStreamingChoice::FinalResponse(MockResponse::with_usage(
                        usage,
                    ))),
                    true,
                ))
            }
        });
        let stream: StreamingResult<Self::StreamingResponse> = Box::pin(stream);
        Ok(StreamingCompletionResponse::stream(stream))
    }
}

#[derive(Clone)]
struct CancelAfterTextStreamModel {
    cancellation_token: tokio_util::sync::CancellationToken,
}

#[derive(Clone)]
struct CancelDuringCompletionModel {
    cancellation_token: tokio_util::sync::CancellationToken,
}

#[derive(Clone)]
struct CancelAndPendCompletionModel {
    cancellation_token: tokio_util::sync::CancellationToken,
}

#[derive(Clone)]
struct CancelAndPendStreamOpenModel {
    cancellation_token: tokio_util::sync::CancellationToken,
}

#[derive(Clone)]
struct CancelAndPendNextStreamModel {
    cancellation_token: tokio_util::sync::CancellationToken,
}

impl CompletionModel for CancelDuringCompletionModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        self.cancellation_token.cancel();
        Ok(CompletionResponse {
            choice: OneOrMany::one(AssistantContent::text("late response")),
            usage: Usage::new(),
            raw_response: MockResponse::new(),
            message_id: None,
        })
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        Err(CompletionError::ProviderError(
            "cancel-during-completion model only supports completion".to_string(),
        ))
    }
}

impl CompletionModel for CancelAndPendCompletionModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        self.cancellation_token.cancel();
        pending().await
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        Err(CompletionError::ProviderError(
            "cancel-and-pend-completion model only supports completion".to_string(),
        ))
    }
}

impl CompletionModel for CancelAndPendStreamOpenModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        Err(CompletionError::ProviderError(
            "cancel-and-pend-stream-open model only supports streaming".to_string(),
        ))
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        self.cancellation_token.cancel();
        pending().await
    }
}

impl CompletionModel for CancelAndPendNextStreamModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        Err(CompletionError::ProviderError(
            "cancel-and-pend-next stream model only supports streaming".to_string(),
        ))
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        let cancellation_token = self.cancellation_token.clone();
        let stream = futures::stream::unfold(0, move |state| {
            let cancellation_token = cancellation_token.clone();
            async move {
                match state {
                    0 => Some((Ok(RawStreamingChoice::Message("partial".to_string())), 1)),
                    1 => {
                        cancellation_token.cancel();
                        pending().await
                    }
                    _ => None,
                }
            }
        });
        let stream: StreamingResult<Self::StreamingResponse> = Box::pin(stream);
        Ok(StreamingCompletionResponse::stream(stream))
    }
}

impl CompletionModel for CancelAfterTextStreamModel {
    type Response = MockResponse;
    type StreamingResponse = MockResponse;
    type Client = ();

    fn make(_: &Self::Client, _: impl Into<String>) -> Self {
        Self {
            cancellation_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    async fn completion(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<CompletionResponse<Self::Response>, CompletionError> {
        Err(CompletionError::ProviderError(
            "cancel-after-text stream model only supports streaming".to_string(),
        ))
    }

    async fn stream(
        &self,
        _request: CompletionRequest,
    ) -> std::result::Result<StreamingCompletionResponse<Self::StreamingResponse>, CompletionError>
    {
        let cancellation_token = self.cancellation_token.clone();
        let stream = futures::stream::unfold(0, move |state| {
            let cancellation_token = cancellation_token.clone();
            async move {
                match state {
                    0 => Some((Ok(RawStreamingChoice::Message("partial".to_string())), 1)),
                    1 => {
                        cancellation_token.cancel();
                        Some((
                            Err(CompletionError::ProviderError(
                                "runtime canceled".to_string(),
                            )),
                            2,
                        ))
                    }
                    _ => None,
                }
            }
        });
        let stream: StreamingResult<Self::StreamingResponse> = Box::pin(stream);
        Ok(StreamingCompletionResponse::stream(stream))
    }
}

#[derive(Clone)]
struct CancelDuringTool {
    cancellation_token: crate::AgentCancellationToken,
}

#[async_trait]
impl ToolExecutor for CancelDuringTool {
    async fn execute(&self, _arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        self.cancellation_token.cancel();
        pending::<Result<ToolInvocationOutput>>().await
    }
}

impl LocalTool for CancelDuringTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: "cancel_during_tool".to_string(),
            description: "Cancel the current run while the tool is still executing.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
            policy: ToolRunPolicy {
                approval_policy: ToolApprovalPolicy::Never,
                execution_policy: ToolExecutionPolicy::Foreground,
                timeout_ms: None,
            },
        }
    }
}

#[derive(Clone)]
struct EchoTool {
    approval_policy: ToolApprovalPolicy,
}

impl EchoTool {
    fn new(approval_policy: ToolApprovalPolicy) -> Self {
        Self { approval_policy }
    }
}

#[async_trait]
impl ToolExecutor for EchoTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        Ok(ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: arguments
                    .get("text")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
            }],
            structured_output: Some(StructuredOutput { value: arguments }),
            raw_output: None,
            is_error: false,
        })
    }
}

#[async_trait]
impl LocalTool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: "echo".to_string(),
            description: "Echo text".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                }
            }),
            policy: ToolRunPolicy {
                approval_policy: self.approval_policy,
                execution_policy: ToolExecutionPolicy::Foreground,
                timeout_ms: None,
            },
        }
    }
}

#[derive(Clone)]
struct ErrorTool;

#[async_trait]
impl ToolExecutor for ErrorTool {
    async fn execute(&self, arguments: serde_json::Value) -> Result<ToolInvocationOutput> {
        Ok(ToolInvocationOutput {
            content: vec![ContentPart::Text {
                text: "human readable error".to_string(),
            }],
            structured_output: Some(StructuredOutput { value: arguments }),
            raw_output: Some(ProviderRawPayload {
                provider_kind: "test".to_string(),
                value: json!({"raw": "details"}),
            }),
            is_error: true,
        })
    }
}

#[async_trait]
impl LocalTool for ErrorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            source: ToolSource::Local,
            namespace: None,
            name: "error_tool".to_string(),
            description: "Return an error output".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "code": { "type": "string" }
                }
            }),
            policy: ToolRunPolicy {
                approval_policy: ToolApprovalPolicy::Never,
                execution_policy: ToolExecutionPolicy::Foreground,
                timeout_ms: None,
            },
        }
    }
}

fn run_settings(provider_id: &str, model_id: &str) -> RunSettingsSnapshot {
    RunSettingsSnapshot {
        prompt: Some(PromptContent {
            text: "You are useful.".to_string(),
        }),
        provider_id: provider_id.to_string(),
        model_id: model_id.to_string(),
        model_capabilities: model_capabilities(),
        provider_settings: provider_settings(),
        reasoning_selection: None,
        tool_policy: ToolPolicySnapshot {
            approval_policy: ToolApprovalPolicy::Never,
            enabled_sources: vec![ToolSource::Local],
            max_steps: 8,
            approval_mode: ToolApprovalMode::RequestApproval,
            permission_scope: None,
        },
    }
}

fn provider_settings() -> ProviderSettingsPayload {
    ProviderSettingsPayload {
        provider_kind: "openai".to_string(),
        fields: Vec::new(),
    }
}

fn model_capabilities() -> ModelCapabilitiesSnapshot {
    ModelCapabilitiesSnapshot {
        text_input: true,
        text_output: true,
        streaming: true,
        image_input: None,
        file_input: None,
        audio_input: false,
        image_generation: false,
        tool_calling: Some(ToolCallingCapabilitySnapshot {
            parallel_tool_calls: true,
        }),
        hosted_web_search: true,
        remote_mcp: false,
        reasoning: None,
        structured_output: true,
        stateful_response_continuation: true,
        extension: ProviderCapabilityExtensionSnapshot::OpenAi {
            responses_api: true,
            raw: None,
        },
    }
}

fn rig_message_text(message: &RigMessage) -> String {
    match message {
        RigMessage::System { content } => content.clone(),
        RigMessage::User { content } => content
            .iter()
            .filter_map(|content| match content {
                UserContent::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        RigMessage::Assistant { .. } => String::new(),
    }
}

#[derive(Clone)]
struct DynamicMcpServer {
    tools: Arc<RwLock<Vec<Tool>>>,
}

impl DynamicMcpServer {
    fn new(tools: Vec<Tool>) -> Self {
        Self {
            tools: Arc::new(RwLock::new(tools)),
        }
    }
}

impl ServerHandler for DynamicMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::LATEST)
            .with_server_info(Implementation::new("jaco-agent-test", "0.1.0"))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(
            self.tools.read().await.clone(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text(format!(
            "called {}",
            request.name
        ))]))
    }
}

fn make_mcp_tool(name: &str, description: &str) -> Tool {
    Tool::new(
        name.to_string(),
        description.to_string(),
        Arc::new(serde_json::Map::new()),
    )
}

async fn start_mcp_server(
    tools: Vec<Tool>,
) -> rmcp::service::RunningService<rmcp::service::RoleClient, ()> {
    let server = DynamicMcpServer::new(tools);
    let (client_to_server, server_from_client) = tokio::io::duplex(8192);
    let (server_to_client, client_from_server) = tokio::io::duplex(8192);

    tokio::spawn(async move {
        let service = server
            .serve((server_from_client, server_to_client))
            .await
            .expect("server failed to start");
        service.waiting().await.expect("server error");
    });

    ().serve((client_from_server, client_to_server))
        .await
        .expect("client failed to connect")
}
