use crate::{
    components::{
        chat_form::{ChatForm, ChatFormEvent},
        message::{MessageView, MessageViewExt},
    },
    foundation::assets::IconName,
    foundation::i18n::I18n,
    state::ConversationDraft,
};
use gpui::actions;
use gpui::{
    AlignItems, AnyElement, App, AppContext, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ListAlignment, ListState, ParentElement, Render, SharedString, Styled,
    Subscription, Task, Window, div, list, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    text::TextViewState,
    v_flex,
};

actions!([DetailEscape]);

pub(crate) trait MessageRevisionExt {
    type Id: Copy + Eq + 'static;

    fn message_id(&self) -> Self::Id;
}

pub(crate) trait ConversationDetailViewExt: Sized + 'static {
    type Message: Clone + crate::components::message::MessageViewExt<Id = Self::MessageId>;
    type MessageId: Copy + Eq + 'static;
    type Revision: Clone + PartialEq + Eq + MessageRevisionExt<Id = Self::MessageId> + 'static;

    fn title(&self, cx: &App) -> SharedString;
    fn subtitle(&self, _cx: &App) -> Option<SharedString> {
        None
    }
    fn header_leading(&self, _cx: &App) -> Option<AnyElement> {
        None
    }
    fn empty_state(&self, _cx: &App) -> Option<AnyElement> {
        None
    }
    fn header_actions(
        _view: &mut ConversationDetailView<Self>,
        _window: &mut Window,
        _cx: &mut Context<ConversationDetailView<Self>>,
    ) -> Vec<AnyElement> {
        Vec::new()
    }
    fn key_context(&self) -> Option<&'static str> {
        None
    }
    fn focus_on_init(&self) -> bool {
        false
    }
    fn element_prefix(&self) -> SharedString;
    fn message_list_alignment(&self) -> ListAlignment {
        ListAlignment::Top
    }
    fn measure_all_message_list(&self) -> bool {
        false
    }
    fn initially_reveal_latest_message(&self) -> bool {
        false
    }

    fn message_revisions(&self, cx: &App) -> Vec<Self::Revision>;
    fn message_at(&self, index: usize, cx: &App) -> Option<Self::Message>;

    fn on_send_requested(
        view: &mut ConversationDetailView<Self>,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<Self>>,
    );
    fn on_pause_requested(
        view: &mut ConversationDetailView<Self>,
        cx: &mut Context<ConversationDetailView<Self>>,
    );
    fn on_chat_form_state_changed(
        _view: &mut ConversationDetailView<Self>,
        _window: &mut Window,
        _cx: &mut Context<ConversationDetailView<Self>>,
    ) {
    }
    fn on_escape(
        _view: &mut ConversationDetailView<Self>,
        _window: &mut Window,
        _cx: &mut Context<ConversationDetailView<Self>>,
    ) {
    }

    fn supports_clear(&self) -> bool {
        false
    }
    fn clear(
        _view: &mut ConversationDetailView<Self>,
        _window: &mut Window,
        _cx: &mut Context<ConversationDetailView<Self>>,
    ) {
    }

    fn supports_save(&self) -> bool {
        false
    }
    fn save(
        _view: &mut ConversationDetailView<Self>,
        _window: &mut Window,
        _cx: &mut Context<ConversationDetailView<Self>>,
    ) {
    }
}

pub(crate) struct RunningTask<I> {
    user_message_id: Option<I>,
    assistant_message_id: Option<I>,
    _task: Task<()>,
}

impl<I: Copy + Eq> RunningTask<I> {
    pub(crate) fn new(task: Task<()>) -> Self {
        Self {
            user_message_id: None,
            assistant_message_id: None,
            _task: task,
        }
    }

    pub(crate) fn bind_messages(
        &mut self,
        user_message_id: Option<I>,
        assistant_message_id: Option<I>,
    ) {
        self.user_message_id = user_message_id;
        self.assistant_message_id = assistant_message_id;
    }

    pub(crate) fn contains_message(&self, message_id: I) -> bool {
        self.user_message_id == Some(message_id) || self.assistant_message_id == Some(message_id)
    }

    pub(crate) fn message_ids(&self) -> [Option<I>; 2] {
        [self.user_message_id, self.assistant_message_id]
    }
}

pub(crate) struct ConversationDetailView<T: ConversationDetailViewExt> {
    pub(crate) detail: T,
    focus_handle: FocusHandle,
    pub(crate) message_list: ListState,
    pub(crate) message_revisions: Vec<T::Revision>,
    message_ids: Vec<T::MessageId>,
    message_text_states: Vec<MessageTextState<T::MessageId>>,
    initial_message_reveal: InitialMessageReveal,
    pub(crate) chat_form: Entity<ChatForm>,
    pub(crate) _subscriptions: Vec<Subscription>,
    pub(crate) task: Option<RunningTask<T::MessageId>>,
}

struct MessageTextState<I> {
    id: I,
    state: Entity<TextViewState>,
    _subscription: Subscription,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct InitialMessageReveal {
    enabled: bool,
    pending: bool,
}

impl InitialMessageReveal {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            pending: enabled,
        }
    }

    fn record_sync_operation(&mut self, operation: &MessageListSyncOperation) {
        if self.enabled && matches!(operation, MessageListSyncOperation::Reset { .. }) {
            self.pending = true;
        }
    }

    fn take_if_ready(&mut self, message_count: usize) -> bool {
        if self.enabled && self.pending && message_count > 0 {
            self.pending = false;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum MessageListSyncOperation {
    None,
    Reset {
        count: usize,
    },
    Remeasure {
        range: std::ops::Range<usize>,
    },
    Splice {
        old_range: std::ops::Range<usize>,
        count: usize,
    },
}

impl<T: ConversationDetailViewExt> ConversationDetailView<T> {
    pub(crate) fn new_with_detail(detail: T, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let alignment = detail.message_list_alignment();
        let initially_reveal_latest_message = detail.initially_reveal_latest_message();
        let should_focus = detail.focus_on_init();
        let focus_handle = cx.focus_handle();
        if should_focus {
            focus_handle.focus(window, cx);
        }
        let message_list = if detail.measure_all_message_list() {
            ListState::new(0, alignment, px(1000.)).measure_all()
        } else {
            ListState::new(0, alignment, px(1000.))
        };
        let chat_form = cx.new(|cx| ChatForm::new(window, cx));
        let _subscriptions = vec![cx.subscribe_in(
            &chat_form,
            window,
            |this, _chat_form, event: &ChatFormEvent, window, cx| match event {
                ChatFormEvent::SendRequested => T::on_send_requested(this, window, cx),
                ChatFormEvent::PauseRequested => T::on_pause_requested(this, cx),
                ChatFormEvent::StateChanged => T::on_chat_form_state_changed(this, window, cx),
            },
        )];
        let mut this = Self {
            detail,
            focus_handle,
            message_list,
            message_revisions: Vec::new(),
            message_ids: Vec::new(),
            message_text_states: Vec::new(),
            initial_message_reveal: InitialMessageReveal::new(initially_reveal_latest_message),
            chat_form,
            _subscriptions,
            task: None,
        };
        if should_focus {
            this.focus_chat_form(window, cx);
        }
        this
    }

    pub(crate) fn has_running_task(&self) -> bool {
        self.task.is_some()
    }

    pub(crate) fn can_start_task(&self) -> bool {
        !self.has_running_task()
    }

    pub(crate) fn set_chat_form_running(
        &mut self,
        running: bool,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.set_running(running, cx));
    }

    pub(crate) fn focus_chat_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.focus_input(window, cx));
    }

    pub(crate) fn restore_chat_form_draft(
        &mut self,
        draft: ConversationDraft,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.restore_draft(draft, window, cx)
        });
    }

    pub(crate) fn send_chat_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        T::on_send_requested(self, window, cx);
    }

    pub(crate) fn set_running_task(
        &mut self,
        task: Task<()>,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        self.task = Some(RunningTask::new(task));
        self.set_chat_form_running(true, cx);
    }

    pub(crate) fn bind_running_task_messages(
        &mut self,
        user_message_id: Option<T::MessageId>,
        assistant_message_id: Option<T::MessageId>,
    ) {
        if let Some(task) = self.task.as_mut() {
            task.bind_messages(user_message_id, assistant_message_id);
        }
    }

    pub(crate) fn clear_running_task_for_message(
        &mut self,
        message_id: Option<T::MessageId>,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        let should_clear = self.task.as_ref().is_some_and(|task| {
            message_id.is_none_or(|message_id| task.contains_message(message_id))
        });
        if should_clear {
            self.task = None;
            self.set_chat_form_running(false, cx);
        }
    }

    fn sync_message_list(&mut self, next_revisions: Vec<T::Revision>) {
        let list_count = self.message_list.item_count();
        let operation =
            message_list_sync_operation(list_count, &self.message_revisions, &next_revisions);
        self.initial_message_reveal
            .record_sync_operation(&operation);

        match operation {
            MessageListSyncOperation::None => return,
            MessageListSyncOperation::Reset { count } => self.message_list.reset(count),
            MessageListSyncOperation::Remeasure { range } => {
                self.message_list.remeasure_items(range);
            }
            MessageListSyncOperation::Splice { old_range, count } => {
                self.message_list.splice(old_range, count);
            }
        }
        self.message_revisions = next_revisions;
    }

    fn sync_message_text_states(&mut self, cx: &mut Context<ConversationDetailView<T>>) {
        if !self.detail.measure_all_message_list() {
            return;
        }

        let sources_label = cx.global::<I18n>().t("field-sources");
        let mut next_ids = Vec::with_capacity(self.message_revisions.len());

        for index in 0..self.message_revisions.len() {
            let Some(message) = self.detail.message_at(index, cx) else {
                continue;
            };
            let message_id = message.id();
            let source = message.content().display_markdown(&sources_label);
            self.ensure_message_text_state(message_id, &source, cx);
            next_ids.push(message_id);
        }

        self.message_text_states
            .retain(|entry| next_ids.contains(&entry.id));
        self.message_ids = next_ids;
    }

    fn ensure_message_text_state(
        &mut self,
        message_id: T::MessageId,
        source: &str,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        if let Some(entry) = self
            .message_text_states
            .iter()
            .find(|entry| entry.id == message_id)
        {
            entry
                .state
                .update(cx, |state, cx| state.set_text(source, cx));
            return;
        }

        let state = cx.new(|cx| TextViewState::markdown(source, cx));
        let subscription = cx.observe(&state, move |this, _, cx| {
            if let Some(index) = this
                .message_ids
                .iter()
                .position(|current_id| *current_id == message_id)
            {
                this.message_list.remeasure_items(index..index + 1);
                cx.notify();
            }
        });

        self.message_text_states.push(MessageTextState {
            id: message_id,
            state,
            _subscription: subscription,
        });
    }

    fn message_text_state(&self, message_id: T::MessageId) -> Option<Entity<TextViewState>> {
        self.message_text_states
            .iter()
            .find(|entry| entry.id == message_id)
            .map(|entry| entry.state.clone())
    }

    fn schedule_initial_message_reveal(
        &mut self,
        window: &Window,
        cx: &mut Context<ConversationDetailView<T>>,
    ) {
        if !self
            .initial_message_reveal
            .take_if_ready(self.message_revisions.len())
        {
            return;
        }

        cx.defer_in(window, |this, _window, cx| {
            if this.message_revisions.is_empty() {
                return;
            }
            this.message_list.scroll_to_end();
            cx.notify();
        });
    }
}

impl<T: ConversationDetailViewExt> Render for ConversationDetailView<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message_revisions = self.detail.message_revisions(cx);
        self.sync_message_list(message_revisions);
        self.sync_message_text_states(cx);
        self.schedule_initial_message_reveal(window, cx);
        let message_count = self.message_revisions.len();

        let title = self.detail.title(cx);
        let subtitle = self
            .detail
            .subtitle(cx)
            .filter(|subtitle| !subtitle.as_ref().trim().is_empty());
        let header_leading = self.detail.header_leading(cx);
        let supports_clear = self.detail.supports_clear();
        let supports_save = self.detail.supports_save();
        let header_actions = T::header_actions(self, window, cx);
        let element_prefix = self.detail.element_prefix();
        let clear_tooltip = cx.global::<I18n>().t("tooltip-clear-conversation");
        let save_tooltip = cx.global::<I18n>().t("tooltip-save-conversation");
        let message_list = self.message_list.clone();
        let this = cx.entity().downgrade();
        let has_subtitle = subtitle.is_some();
        let empty_state = (message_count == 0)
            .then(|| self.detail.empty_state(cx))
            .flatten();
        let header_body = match subtitle {
            Some(subtitle) => h_flex()
                .gap_2()
                .items_start()
                .when_some(header_leading, |this, leading| this.child(leading))
                .child(
                    v_flex().gap_1().child(Label::new(title).text_xl()).child(
                        Label::new(subtitle)
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
                )
                .into_any_element(),
            None => h_flex()
                .gap_2()
                .items_center()
                .when_some(header_leading, |this, leading| this.child(leading))
                .child(Label::new(title).text_xl())
                .into_any_element(),
        };

        let mut content = v_flex()
            .size_full()
            .overflow_hidden()
            .pb_4()
            .track_focus(&self.focus_handle)
            .child(
                h_flex()
                    .flex_initial()
                    .p_2()
                    .gap_2()
                    .map(|this| {
                        if has_subtitle {
                            this.items_start()
                        } else {
                            this.items_center()
                        }
                    })
                    .justify_between()
                    .child(header_body)
                    .map(|this| {
                        if !(supports_clear || supports_save || !header_actions.is_empty()) {
                            return this;
                        }

                        this.child(h_flex().items_center().gap_1().map(|this| {
                            let this = if supports_clear {
                                this.child(
                                    Button::new(SharedString::from(format!(
                                        "{element_prefix}-clear"
                                    )))
                                    .icon(IconName::BrushCleaning)
                                    .ghost()
                                    .small()
                                    .disabled(self.has_running_task())
                                    .tooltip(clear_tooltip.clone())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        T::clear(view, window, cx);
                                    })),
                                )
                            } else {
                                this
                            };

                            if supports_save {
                                let this = this.child(
                                    Button::new(SharedString::from(format!(
                                        "{element_prefix}-save"
                                    )))
                                    .icon(IconName::Save)
                                    .ghost()
                                    .small()
                                    .disabled(self.has_running_task())
                                    .tooltip(save_tooltip.clone())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        T::save(view, window, cx);
                                    })),
                                );
                                this.children(header_actions)
                            } else {
                                this.children(header_actions)
                            }
                        }))
                    }),
            )
            .child(Divider::horizontal())
            .child({
                let content = div()
                    .id(SharedString::from(format!("{element_prefix}-content")))
                    .flex_1()
                    .overflow_hidden()
                    .relative()
                    .w_full();
                if let Some(empty_state) = empty_state {
                    content.child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .p_6()
                            .child(empty_state),
                    )
                } else {
                    content
                        .child(
                            list(message_list.clone(), move |ix, _window, cx| {
                                this.upgrade()
                                    .and_then(|view| {
                                        let view = view.read(cx);
                                        view.detail.message_at(ix, cx).map(|message| {
                                            let text_state = view.message_text_state(message.id());
                                            MessageView::new(message).with_text_state(text_state)
                                        })
                                    })
                                    .map(|message| message.into_any_element())
                                    .unwrap_or_else(|| div().into_any_element())
                            })
                            .size_full(),
                        )
                        .vertical_scrollbar(&message_list)
                }
            })
            .child({
                let mut footer = v_flex();
                footer.style().align_items = Some(AlignItems::Stretch);
                footer
                    .w_full()
                    .flex_initial()
                    .px_4()
                    .child(self.chat_form.clone())
            });

        if let Some(key_context) = self.detail.key_context() {
            content = content.key_context(key_context).on_action(cx.listener(
                |view, _: &DetailEscape, window, cx| {
                    T::on_escape(view, window, cx);
                },
            ));
        }

        content
    }
}

fn first_revision_diff<T: PartialEq>(
    previous_revisions: &[T],
    next_revisions: &[T],
) -> Option<usize> {
    previous_revisions
        .iter()
        .zip(next_revisions.iter())
        .position(|(left, right)| left != right)
        .or_else(|| {
            (previous_revisions.len() != next_revisions.len())
                .then(|| previous_revisions.len().min(next_revisions.len()))
        })
}

fn first_message_identity_diff<T: MessageRevisionExt>(
    previous_revisions: &[T],
    next_revisions: &[T],
    start_index: usize,
) -> Option<usize> {
    previous_revisions[start_index..]
        .iter()
        .zip(next_revisions[start_index..].iter())
        .position(|(left, right)| left.message_id() != right.message_id())
        .map(|offset| start_index + offset)
}

fn message_list_sync_operation<T: PartialEq + MessageRevisionExt>(
    list_item_count: usize,
    previous_revisions: &[T],
    next_revisions: &[T],
) -> MessageListSyncOperation {
    if list_item_count != previous_revisions.len() {
        return MessageListSyncOperation::Reset {
            count: next_revisions.len(),
        };
    }

    let Some(first_diff) = first_revision_diff(previous_revisions, next_revisions) else {
        return MessageListSyncOperation::None;
    };

    if previous_revisions.len() == next_revisions.len() {
        if let Some(first_identity_diff) =
            first_message_identity_diff(previous_revisions, next_revisions, first_diff)
        {
            MessageListSyncOperation::Splice {
                old_range: first_identity_diff..previous_revisions.len(),
                count: next_revisions.len().saturating_sub(first_identity_diff),
            }
        } else {
            MessageListSyncOperation::Remeasure {
                range: first_diff..next_revisions.len(),
            }
        }
    } else {
        MessageListSyncOperation::Splice {
            old_range: first_diff..previous_revisions.len(),
            count: next_revisions.len().saturating_sub(first_diff),
        }
    }
}

#[cfg(test)]
mod tests;
