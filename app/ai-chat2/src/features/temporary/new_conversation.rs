use crate::components::chat_form::{
    ChatForm, ChatFormEvent, ChatFormSkillCompletionPlacement, ChatFormSubmit,
};
use gpui::*;
use gpui_component::v_flex;

#[allow(clippy::enum_variant_names)]
#[derive(Clone)]
pub(super) enum TemporaryNewConversationPaneEvent {
    SendRequested(Box<ChatFormSubmit>),
}

pub(super) struct TemporaryNewConversationPane {
    chat_form: Entity<ChatForm>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<TemporaryNewConversationPaneEvent> for TemporaryNewConversationPane {}

impl TemporaryNewConversationPane {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_form = cx.new(|cx| {
            let mut chat_form = ChatForm::new(window, cx);
            chat_form.set_skill_completion_placement(ChatFormSkillCompletionPlacement::BelowForm);
            chat_form
        });
        chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_skill_catalog(None, cx);
        });
        let chat_form_subscription = cx.subscribe_in(
            &chat_form,
            window,
            |_pane, _chat_form, event: &ChatFormEvent, _window, cx| match event {
                ChatFormEvent::SendRequested(submit) => {
                    cx.emit(TemporaryNewConversationPaneEvent::SendRequested(
                        submit.clone(),
                    ));
                }
                ChatFormEvent::StopRequested | ChatFormEvent::AddRequested => {}
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

    pub(super) fn clear_after_submit(&mut self, cx: &mut Context<Self>) {
        self.chat_form
            .update(cx, |chat_form, cx| chat_form.clear_after_submit(cx));
    }
}

impl Render for TemporaryNewConversationPane {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("ai-chat2-temporary-new-conversation")
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
