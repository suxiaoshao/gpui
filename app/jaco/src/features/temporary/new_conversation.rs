use crate::components::chat_input::{
    ChatFormSkillCompletionPlacement, ChatInputController, ChatInputEvent, ChatInputSubmit,
};
use gpui::*;
use gpui_component::v_flex;

#[allow(clippy::enum_variant_names)]
#[derive(Clone)]
pub(super) enum TemporaryNewConversationPaneEvent {
    SendRequested(Box<ChatInputSubmit>),
}

pub(super) struct TemporaryNewConversationPane {
    chat_form: Entity<ChatInputController>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TemporaryNewConversationPaneEvent> for TemporaryNewConversationPane {}

impl TemporaryNewConversationPane {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_form = cx.new(|cx| {
            // The temporary window owns keyboard focus through its search input;
            // route changes must not focus the composer as a side effect.
            let mut chat_form = ChatInputController::new_without_focus(window, cx);
            chat_form
                .set_skill_completion_placement(ChatFormSkillCompletionPlacement::BelowForm, cx);
            chat_form
        });
        chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_skill_catalog(None, cx);
        });
        let chat_form_subscription = cx.subscribe_in(
            &chat_form,
            window,
            |_pane, _chat_form, event: &ChatInputEvent, _window, cx| match event {
                ChatInputEvent::SendRequested(submit) => {
                    cx.emit(TemporaryNewConversationPaneEvent::SendRequested(
                        submit.clone(),
                    ));
                }
                ChatInputEvent::StopRequested
                | ChatInputEvent::AddRequested
                | ChatInputEvent::AddProjectRequested => {}
            },
        );

        Self {
            chat_form,
            _subscriptions: vec![chat_form_subscription],
        }
    }

    pub(super) fn focus_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.focus_composer(window, cx));
    }

    pub(super) fn clear_after_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.clear_after_submit(window, cx));
    }
}

impl Render for TemporaryNewConversationPane {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("jaco-temporary-new-conversation")
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .items_center()
            .justify_center()
            .px_8()
            .py_12()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(780.))
                    .items_center()
                    .child(self.chat_form.clone()),
            )
    }
}
