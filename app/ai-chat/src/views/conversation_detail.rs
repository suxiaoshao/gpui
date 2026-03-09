use crate::{
    components::{
        chat_form::{ChatForm, ChatFormEvent},
        message::MessageView,
    },
    i18n::I18n,
};
use gpui::actions;
use gpui::{
    AlignItems, AnyElement, App, AppContext, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ListAlignment, ListState, ParentElement, Render, SharedString, Styled,
    Subscription, Task, Window, div, list, prelude::FluentBuilder, px,
};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    v_flex,
};

actions!([DetailEscape]);

pub(crate) trait ConversationDetailViewExt: Sized + 'static {
    type Message: Clone + crate::components::message::MessageViewExt<Id = Self::MessageId>;
    type MessageId: Copy + Eq + 'static;
    type Revision: Clone + PartialEq + Eq + 'static;

    fn title(&self, cx: &App) -> SharedString;
    fn subtitle(&self, _cx: &App) -> Option<SharedString> {
        None
    }
    fn header_leading(&self, _cx: &App) -> Option<AnyElement> {
        None
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
    fn auto_scroll_new_messages_when_at_end(&self) -> bool {
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
    is_pinned_to_end: bool,
    pub(crate) chat_form: Entity<ChatForm>,
    pub(crate) _subscriptions: Vec<Subscription>,
    pub(crate) task: Option<RunningTask<T::MessageId>>,
}

impl<T: ConversationDetailViewExt> ConversationDetailView<T> {
    pub(crate) fn new_with_detail(detail: T, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let alignment = detail.message_list_alignment();
        let auto_scroll_new_messages = detail.auto_scroll_new_messages_when_at_end();
        let focus_handle = cx.focus_handle();
        if detail.focus_on_init() {
            focus_handle.focus(window);
        }
        let message_list = ListState::new(0, alignment, px(1000.));
        if auto_scroll_new_messages && alignment == ListAlignment::Bottom {
            let view = cx.entity().downgrade();
            message_list.set_scroll_handler(move |event, _window, cx| {
                let _ = view.update(cx, |view, _cx| {
                    view.is_pinned_to_end = list_is_pinned_to_end(alignment, event.is_scrolled);
                });
            });
        }
        let chat_form = cx.new(|cx| ChatForm::new(window, cx));
        let _subscriptions = vec![cx.subscribe_in(
            &chat_form,
            window,
            |this, _chat_form, event: &ChatFormEvent, window, cx| match event {
                ChatFormEvent::SendRequested => T::on_send_requested(this, window, cx),
                ChatFormEvent::PauseRequested => T::on_pause_requested(this, cx),
            },
        )];
        Self {
            detail,
            focus_handle,
            message_list,
            message_revisions: Vec::new(),
            is_pinned_to_end: auto_scroll_new_messages && alignment == ListAlignment::Bottom,
            chat_form,
            _subscriptions,
            task: None,
        }
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

    pub(crate) fn sync_message_list(&mut self, next_revisions: Vec<T::Revision>) {
        if self.message_list.item_count() != self.message_revisions.len() {
            self.message_list.reset(next_revisions.len());
            self.message_revisions = next_revisions;
            return;
        }

        if self.message_revisions == next_revisions {
            return;
        }

        let first_diff = self
            .message_revisions
            .iter()
            .zip(next_revisions.iter())
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| self.message_revisions.len().min(next_revisions.len()));

        self.message_list.splice(
            first_diff..self.message_revisions.len(),
            next_revisions.len().saturating_sub(first_diff),
        );
        self.message_revisions = next_revisions;
    }

    fn maybe_reveal_latest_message(&mut self, previous_count: usize) {
        let next_count = self.message_revisions.len();
        if should_reveal_latest_message(
            self.detail.auto_scroll_new_messages_when_at_end(),
            self.is_pinned_to_end,
            previous_count,
            next_count,
        ) {
            self.message_list.scroll_to_reveal_item(next_count - 1);
        }
    }
}

impl<T: ConversationDetailViewExt> Render for ConversationDetailView<T> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message_revisions = self.detail.message_revisions(cx);
        let previous_count = self.message_revisions.len();
        self.sync_message_list(message_revisions);
        self.maybe_reveal_latest_message(previous_count);

        let title = self.detail.title(cx);
        let subtitle = self
            .detail
            .subtitle(cx)
            .filter(|subtitle| !subtitle.as_ref().trim().is_empty());
        let header_leading = self.detail.header_leading(cx);
        let supports_clear = self.detail.supports_clear();
        let supports_save = self.detail.supports_save();
        let element_prefix = self.detail.element_prefix();
        let clear_tooltip = cx.global::<I18n>().t("tooltip-clear-conversation");
        let save_tooltip = cx.global::<I18n>().t("tooltip-save-conversation");
        let message_list = self.message_list.clone();
        let this = cx.entity().downgrade();
        let has_subtitle = subtitle.is_some();
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
            .pb_2()
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
                        if !(supports_clear || supports_save) {
                            return this;
                        }

                        this.child(h_flex().items_center().gap_1().map(|this| {
                            let this = if supports_clear {
                                this.child(
                                    Button::new(SharedString::from(format!(
                                        "{element_prefix}-clear"
                                    )))
                                    .icon(IconName::Delete)
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
                                this.child(
                                    Button::new(SharedString::from(format!(
                                        "{element_prefix}-save"
                                    )))
                                    .icon(IconName::Inbox)
                                    .ghost()
                                    .small()
                                    .disabled(self.has_running_task())
                                    .tooltip(save_tooltip.clone())
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        T::save(view, window, cx);
                                    })),
                                )
                            } else {
                                this
                            }
                        }))
                    }),
            )
            .child(Divider::horizontal())
            .child(
                div()
                    .id(SharedString::from(format!("{element_prefix}-content")))
                    .flex_1()
                    .overflow_hidden()
                    .relative()
                    .w_full()
                    .child(
                        list(message_list.clone(), move |ix, _window, cx| {
                            this.upgrade()
                                .and_then(|view| {
                                    view.read(cx)
                                        .detail
                                        .message_at(ix, cx)
                                        .map(MessageView::new)
                                })
                                .map(|message| message.into_any_element())
                                .unwrap_or_else(|| div().into_any_element())
                        })
                        .size_full(),
                    )
                    .vertical_scrollbar(&message_list),
            )
            .child({
                let mut footer = v_flex();
                footer.style().align_items = Some(AlignItems::Stretch);
                footer
                    .w_full()
                    .flex_initial()
                    .px_2()
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

fn list_is_pinned_to_end(alignment: ListAlignment, is_scrolled: bool) -> bool {
    alignment == ListAlignment::Bottom && !is_scrolled
}

fn should_reveal_latest_message(
    auto_scroll_new_messages: bool,
    is_pinned_to_end: bool,
    previous_count: usize,
    next_count: usize,
) -> bool {
    auto_scroll_new_messages
        && is_pinned_to_end
        && previous_count > 0
        && next_count > previous_count
}

#[cfg(test)]
mod tests {
    use super::{RunningTask, list_is_pinned_to_end, should_reveal_latest_message};
    use gpui::{ListAlignment, Task};

    #[test]
    fn running_task_binds_and_matches_messages() {
        let task = Task::ready(());
        let mut running_task = RunningTask::new(task);
        running_task.bind_messages(Some(1usize), Some(2usize));

        assert!(running_task.contains_message(1));
        assert!(running_task.contains_message(2));
        assert!(!running_task.contains_message(3));
        assert_eq!(running_task.message_ids(), [Some(1), Some(2)]);
    }

    #[test]
    fn bottom_aligned_list_is_pinned_only_when_not_scrolled() {
        assert!(list_is_pinned_to_end(ListAlignment::Bottom, false));
        assert!(!list_is_pinned_to_end(ListAlignment::Bottom, true));
        assert!(!list_is_pinned_to_end(ListAlignment::Top, false));
    }

    #[test]
    fn reveal_latest_message_only_when_at_end_and_count_grows() {
        assert!(should_reveal_latest_message(true, true, 1, 2));
        assert!(!should_reveal_latest_message(true, false, 1, 2));
        assert!(!should_reveal_latest_message(true, true, 0, 1));
        assert!(!should_reveal_latest_message(true, true, 2, 2));
        assert!(!should_reveal_latest_message(false, true, 1, 2));
    }
}
