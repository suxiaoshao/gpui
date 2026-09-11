use super::loading::ReadMessage;
use super::*;
use crate::foundation::model_scope;

pub(crate) struct ThinkingLevels {
    pub model: Option<(String, String)>,
    pub levels: Vec<String>,
}
impl Session {
    pub fn model_identity(&self) -> Option<(String, String)> {
        self.state
            .as_ref()?
            .model
            .as_ref()
            .map(|model| (model.provider.clone(), model.id.clone()))
    }
    pub fn model_options(&self) -> &[Model] {
        self.models.data().map(Vec::as_slice).unwrap_or_default()
    }
    pub fn models_loading(&self) -> bool {
        self.models.running()
            || self.connection_purpose == ConnectionPurpose::ModelOptions
                && self.instance.is_some()
                && self.state.is_none()
                && self.error.is_none()
    }
    pub fn levels(&self) -> &[String] {
        self.thinking_levels
            .data()
            .filter(|data| data.model == self.model_identity())
            .map(|data| data.levels.as_slice())
            .unwrap_or_default()
    }
    pub fn fork_options(&self) -> &[protocol::ForkMessage] {
        if self.fork_messages.running()
            || self.fork_messages.error().is_some()
            || self.instance.is_none()
        {
            return &[];
        }
        self.fork_messages
            .data()
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
    pub fn settings_busy(&self) -> bool {
        self.busy()
            || self.command.running()
            || self.model_change.running()
            || !self.pending_ui.is_empty()
    }
    fn next_read(&mut self) -> u64 {
        self.read_serial += 1;
        self.read_serial
    }
    pub(super) fn reset_reads(&mut self) {
        self.core_read.finish(None);
        self.models.reset();
        self.thinking_levels.reset();
        self.stats.reset();
        self.fork_messages.reset();
        self.model_change.finish(Ok(None));
    }
}
impl ConversationState {
    pub(super) fn refresh_auxiliary(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(s) = self.sessions.get(key) else {
            return;
        };
        let models = matches!(s.models, ReadState::Idle);
        let thinking = matches!(s.thinking_levels, ReadState::Idle);
        if models {
            self.read_models(key, cx);
        }
        if thinking {
            self.read_thinking(key, cx);
        }
        self.refresh_stats(key, cx);
        self.refresh_fork_messages(key, cx);
    }
    pub fn refresh_models(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(s) = self.sessions.get(key) else {
            return;
        };
        if self.draining || s.settings_busy() || s.models.running() || s.thinking_levels.running() {
            return;
        }
        if s.instance.is_none() || s.state.is_none() {
            self.connect_for(key, ConnectionPurpose::ModelOptions, cx);
            if self.sessions[key].state.is_none()
                && self.sessions[key].connection_purpose == ConnectionPurpose::Conversation
                && self.client(key, cx).is_some()
            {
                self.refresh(key, cx);
            }
            return;
        }
        if s.model_change.unconfirmed() {
            self.change_model_setting(key, None, cx);
            return;
        }
        self.read_models(key, cx);
        self.read_thinking(key, cx);
    }
    pub(super) fn read_models(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if self.draining || s.models.running() {
            return;
        }
        let binding = s.binding;
        let id = s.next_read();
        let cwd = s.info.cwd.clone();
        let agent = self.discovery.agent.clone();
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let result = async {
                let models = client
                    .get_available_models()
                    .await
                    .map_err(|e| e.to_string())?
                    .models;
                smol::unblock(move || {
                    let scope = model_scope::load(&agent, &cwd).map_err(|e| e.to_string())?;
                    Ok(model_scope::filter(models, scope.as_deref()))
                })
                .await
            }
            .await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this
                    .sessions
                    .get_mut(&task_key)
                    .filter(|s| s.binding == binding)
                {
                    s.models.transition(ReadMessage::Finish { id, result });
                    cx.notify();
                }
            });
        });
        self.sessions
            .get_mut(key)
            .unwrap()
            .models
            .transition(ReadMessage::Start { id, task });
        cx.notify();
    }
    pub(super) fn read_thinking(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if self.draining || s.thinking_levels.running() || s.model_change.running() {
            return;
        }
        let binding = s.binding;
        let revision = s.model_revision;
        let model = s.model_identity();
        let id = s.next_read();
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let result = client
                .get_available_thinking_levels()
                .await
                .map(|value| ThinkingLevels {
                    model,
                    levels: value.levels,
                })
                .map_err(|e| e.to_string());
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this
                    .sessions
                    .get_mut(&task_key)
                    .filter(|s| s.binding == binding && s.model_revision == revision)
                {
                    s.thinking_levels
                        .transition(ReadMessage::Finish { id, result });
                    cx.notify();
                }
            });
        });
        self.sessions
            .get_mut(key)
            .unwrap()
            .thinking_levels
            .transition(ReadMessage::Start { id, task });
        cx.notify();
    }
    pub fn refresh_stats(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if self.draining || s.state.is_none() {
            return;
        }
        s.stats.transition(ReadMessage::Cancel);
        let binding = s.binding;
        let revision = s.event_revision;
        let model_revision = s.model_revision;
        let id = s.next_read();
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let result = client.get_session_stats().await.map_err(|e| e.to_string());
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this
                    .sessions
                    .get_mut(&task_key)
                    .filter(|s| s.binding == binding && s.stats.accepts(id))
                {
                    if s.event_revision != revision || s.model_revision != model_revision {
                        s.stats.transition(ReadMessage::Cancel);
                    } else {
                        s.stats.transition(ReadMessage::Finish { id, result });
                    }
                    cx.notify();
                }
            });
        });
        self.sessions
            .get_mut(key)
            .unwrap()
            .stats
            .transition(ReadMessage::Start { id, task });
        cx.notify();
    }
    pub fn refresh_fork_messages(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(client) = self.client(key, cx) else {
            return;
        };
        let s = self.sessions.get_mut(key).unwrap();
        if self.draining || s.state.is_none() {
            return;
        }
        s.fork_messages.transition(ReadMessage::Cancel);
        let binding = s.binding;
        let revision = s.event_revision;
        let id = s.next_read();
        let task_key = key.to_owned();
        let task = cx.spawn(async move |owner, cx| {
            let result = client
                .get_fork_messages()
                .await
                .map(|value| value.messages)
                .map_err(|e| e.to_string());
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this
                    .sessions
                    .get_mut(&task_key)
                    .filter(|s| s.binding == binding && s.fork_messages.accepts(id))
                {
                    if s.event_revision != revision {
                        s.fork_messages.transition(ReadMessage::Cancel);
                    } else {
                        s.fork_messages
                            .transition(ReadMessage::Finish { id, result });
                    }
                    s.content_revision += 1;
                    cx.notify();
                }
            });
        });
        let s = self.sessions.get_mut(key).unwrap();
        s.fork_messages.transition(ReadMessage::Start { id, task });
        s.content_revision += 1;
        cx.notify();
    }
}
