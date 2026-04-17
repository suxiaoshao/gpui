use crate::{
    components::{
        chat_form::{ChatForm, ChatFormEvent},
        message::MessageView,
    },
    i18n::I18n,
    state::ConversationDraft,
};
use gpui::actions;
use gpui::{
    AlignItems, AnyElement, App, AppContext, Context, Entity, FocusHandle, InteractiveElement,
    IntoElement, ListAlignment, ListOffset, ListState, ParentElement, Render, SharedString, Styled,
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
    pub(crate) chat_form: Entity<ChatForm>,
    pub(crate) _subscriptions: Vec<Subscription>,
    pub(crate) task: Option<RunningTask<T::MessageId>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MessageListChange {
    item_count_increased: bool,
    latest_revision_changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreservedScrollOffset {
    item_ix: usize,
    offset_in_item: gpui::Pixels,
}

impl<T: ConversationDetailViewExt> ConversationDetailView<T> {
    pub(crate) fn new_with_detail(detail: T, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let alignment = detail.message_list_alignment();
        let auto_scroll_new_messages = detail.auto_scroll_new_messages_when_at_end();
        let should_focus = detail.focus_on_init();
        let focus_handle = cx.focus_handle();
        if should_focus {
            focus_handle.focus(window, cx);
        }
        let message_list = if should_measure_all_message_list(auto_scroll_new_messages) {
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

    fn sync_message_list(&mut self, next_revisions: Vec<T::Revision>) -> MessageListChange {
        let change = message_list_change(&self.message_revisions, &next_revisions);

        if self.message_list.item_count() != self.message_revisions.len() {
            self.message_list.reset(next_revisions.len());
            self.message_revisions = next_revisions;
            return change;
        }

        if self.message_revisions == next_revisions {
            return change;
        }

        let first_diff = self
            .message_revisions
            .iter()
            .zip(next_revisions.iter())
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| self.message_revisions.len().min(next_revisions.len()));
        let preserved_scroll_offset = preserved_tail_item_scroll_offset(
            &self.message_list,
            self.message_revisions.len(),
            next_revisions.len(),
            first_diff,
        );

        self.message_list.splice(
            first_diff..self.message_revisions.len(),
            next_revisions.len().saturating_sub(first_diff),
        );
        if let Some(scroll_offset) = preserved_scroll_offset {
            self.message_list.scroll_to(ListOffset {
                item_ix: scroll_offset.item_ix,
                offset_in_item: scroll_offset.offset_in_item,
            });
        }
        self.message_revisions = next_revisions;
        change
    }

    fn maybe_reveal_latest_message(&mut self, change: MessageListChange, was_at_end: bool) {
        if should_reveal_latest_message(
            self.detail.auto_scroll_new_messages_when_at_end(),
            was_at_end,
            change,
            self.message_revisions.len(),
        ) {
            scroll_to_message_end(
                &self.message_list,
                self.detail.message_list_alignment(),
                self.message_revisions.len(),
            );
        }
    }
}

impl<T: ConversationDetailViewExt> Render for ConversationDetailView<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message_revisions = self.detail.message_revisions(cx);
        let alignment = self.detail.message_list_alignment();
        let was_at_end = list_is_at_end(&self.message_list, alignment);
        let change = self.sync_message_list(message_revisions);
        self.maybe_reveal_latest_message(change, was_at_end);

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
                                let this = this.child(
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
                                );
                                this.children(header_actions)
                            } else {
                                this.children(header_actions)
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

fn should_measure_all_message_list(auto_scroll_new_messages: bool) -> bool {
    auto_scroll_new_messages
}

const SCROLL_END_EPSILON_PX: f32 = 1.;

fn scroll_to_message_end(message_list: &ListState, alignment: ListAlignment, item_count: usize) {
    if item_count == 0 {
        return;
    }

    match alignment {
        ListAlignment::Bottom => message_list.scroll_to_reveal_item(item_count - 1),
        ListAlignment::Top => message_list.scroll_to(ListOffset {
            item_ix: item_count,
            offset_in_item: px(0.),
        }),
    }
}
fn list_is_at_end(message_list: &ListState, alignment: ListAlignment) -> bool {
    match alignment {
        ListAlignment::Bottom => {
            let scroll_top = message_list.logical_scroll_top();
            scroll_top.item_ix >= message_list.item_count() && scroll_top.offset_in_item == px(0.)
        }
        ListAlignment::Top => {
            let max_offset = message_list.max_offset_for_scrollbar().y;
            if max_offset == px(0.) {
                true
            } else {
                let current_offset = (-message_list.scroll_px_offset_for_scrollbar().y).max(px(0.));
                current_offset + px(SCROLL_END_EPSILON_PX) >= max_offset
            }
        }
    }
}

fn latest_revision_changed<T: PartialEq>(
    previous_revision: Option<&T>,
    next_revision: Option<&T>,
) -> bool {
    match (previous_revision, next_revision) {
        (Some(previous), Some(next)) => previous != next,
        _ => false,
    }
}

fn preserved_tail_item_scroll_offset(
    message_list: &ListState,
    previous_item_count: usize,
    next_item_count: usize,
    first_diff: usize,
) -> Option<PreservedScrollOffset> {
    if previous_item_count == 0
        || previous_item_count != next_item_count
        || first_diff + 1 != previous_item_count
    {
        return None;
    }

    let scroll_top = message_list.logical_scroll_top();
    if scroll_top.item_ix != first_diff || scroll_top.offset_in_item == px(0.) {
        return None;
    }

    Some(PreservedScrollOffset {
        item_ix: scroll_top.item_ix,
        offset_in_item: scroll_top.offset_in_item,
    })
}

fn message_list_change<T: PartialEq>(
    previous_revisions: &[T],
    next_revisions: &[T],
) -> MessageListChange {
    MessageListChange {
        item_count_increased: next_revisions.len() > previous_revisions.len(),
        latest_revision_changed: latest_revision_changed(
            previous_revisions.last(),
            next_revisions.last(),
        ),
    }
}

fn should_reveal_latest_message(
    auto_scroll_new_messages: bool,
    was_at_end: bool,
    change: MessageListChange,
    next_item_count: usize,
) -> bool {
    if !auto_scroll_new_messages || next_item_count == 0 {
        return false;
    }

    if change.item_count_increased {
        return true;
    }

    was_at_end && change.latest_revision_changed
}

#[cfg(test)]
mod tests {
    use super::{
        MessageListChange, PreservedScrollOffset, RunningTask, latest_revision_changed,
        list_is_at_end, message_list_change, preserved_tail_item_scroll_offset,
        should_measure_all_message_list, should_reveal_latest_message,
    };
    use gpui::{ListAlignment, ListState, Task, px};

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
    fn auto_scrolling_lists_use_full_measurement() {
        assert!(should_measure_all_message_list(true));
        assert!(!should_measure_all_message_list(false));
    }

    #[test]
    fn bottom_aligned_list_is_at_end_only_at_bottom_offset() {
        let state = ListState::new(3, ListAlignment::Bottom, px(100.));
        assert!(list_is_at_end(&state, ListAlignment::Bottom));

        state.scroll_to_reveal_item(0);
        assert!(!list_is_at_end(&state, ListAlignment::Bottom));
        assert!(list_is_at_end(&state, ListAlignment::Top));
    }

    #[test]
    fn latest_revision_change_only_tracks_last_message() {
        assert!(latest_revision_changed(Some(&2), Some(&3)));
        assert!(!latest_revision_changed(Some(&2), Some(&2)));
        assert!(!latest_revision_changed::<i32>(None, None));
    }

    #[test]
    fn message_list_change_tracks_new_items_and_tail_updates() {
        assert_eq!(
            message_list_change(&[1], &[1, 2]),
            MessageListChange {
                item_count_increased: true,
                latest_revision_changed: true,
            }
        );
        assert_eq!(
            message_list_change(&[1, 2], &[9, 2]),
            MessageListChange {
                item_count_increased: false,
                latest_revision_changed: false,
            }
        );
    }

    #[test]
    fn preserves_scroll_offset_for_last_item_only_when_viewport_is_inside_it() {
        let state = ListState::new(2, ListAlignment::Top, px(100.));
        state.scroll_to(gpui::ListOffset {
            item_ix: 1,
            offset_in_item: px(42.),
        });

        assert_eq!(
            preserved_tail_item_scroll_offset(&state, 2, 2, 1),
            Some(PreservedScrollOffset {
                item_ix: 1,
                offset_in_item: px(42.),
            })
        );
        assert_eq!(preserved_tail_item_scroll_offset(&state, 2, 2, 0), None);
        assert_eq!(preserved_tail_item_scroll_offset(&state, 2, 3, 1), None);

        state.scroll_to(gpui::ListOffset {
            item_ix: 1,
            offset_in_item: px(0.),
        });
        assert_eq!(preserved_tail_item_scroll_offset(&state, 2, 2, 1), None);
    }

    #[test]
    fn reveal_latest_message_for_new_message_or_tail_chunk_at_end() {
        assert!(should_reveal_latest_message(
            true,
            false,
            MessageListChange {
                item_count_increased: true,
                latest_revision_changed: true,
            },
            2,
        ));
        assert!(should_reveal_latest_message(
            true,
            true,
            MessageListChange {
                item_count_increased: false,
                latest_revision_changed: true,
            },
            2,
        ));
        assert!(!should_reveal_latest_message(
            true,
            false,
            MessageListChange {
                item_count_increased: false,
                latest_revision_changed: true,
            },
            2,
        ));
        assert!(!should_reveal_latest_message(
            true,
            true,
            MessageListChange::default(),
            2,
        ));
        assert!(!should_reveal_latest_message(
            false,
            true,
            MessageListChange {
                item_count_increased: true,
                latest_revision_changed: true,
            },
            2,
        ));
    }
}
