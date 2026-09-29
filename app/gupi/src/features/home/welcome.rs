use super::*;
use crate::{foundation::i18n::t_with_args, state::conversation::Session};
use fluent_bundle::FluentArgs;

impl HomeView {
    pub(super) fn render_welcome(
        &self,
        session: Option<&Session>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let welcome = v_flex()
            .id("conversation-welcome")
            .test_support()
            .flex_1()
            .w_full()
            .px_6()
            .justify_center()
            .items_center();
        let Some(session) = session else {
            return welcome
                .gap_4()
                .child(div().text_2xl().child(t(cx, "conversation-welcome")))
                .child(
                    Button::new("empty-new-conversation")
                        .outline()
                        .icon(IconName::Plus)
                        .label(t(cx, "conversation-new"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.new_conversation(window, cx);
                        })),
                )
                .into_any_element();
        };
        if self.state.read(cx).temporary
            || !session.info.path.as_os_str().is_empty()
            || !session.pending_ui.is_empty()
        {
            return welcome
                .child(div().text_2xl().child(t(cx, "conversation-welcome")))
                .into_any_element();
        }

        let path = &session.info.cwd;
        let name = path
            .file_name()
            .filter(|name| !name.is_empty())
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let mut args = FluentArgs::new();
        args.set("project", name);
        welcome
            .child(
                div()
                    .max_w_full()
                    .text_2xl()
                    .font_weight(FontWeight::NORMAL)
                    .line_height(relative(1.25))
                    .truncate()
                    .child(t_with_args(cx, "conversation-welcome-project", &args)),
            )
            .into_any_element()
    }
}
