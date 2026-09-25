use super::*;
use crate::{foundation::i18n::t_with_args, state::conversation::Session};
use fluent_bundle::FluentArgs;
use gpui_kit::component::{Icon, label::Label};

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

        // Keep the sentence's word order in Fluent while substituting a native
        // button for the project argument. Remove only this slot's bidi isolation.
        let mut args = FluentArgs::new();
        args.set("project", "\u{fffc}");
        let sentence = t_with_args(cx, "conversation-welcome-project", &args)
            .replace("\u{2068}\u{fffc}\u{2069}", "\u{fffc}");
        let (before, after) = sentence.split_once('\u{fffc}').unwrap_or((&sentence, ""));
        let path = &session.info.cwd;
        let name = path
            .file_name()
            .filter(|name| !name.is_empty())
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        welcome
            .child(
                h_flex()
                    .max_w_full()
                    .flex_wrap()
                    .justify_center()
                    .items_center()
                    .gap_0()
                    .text_2xl()
                    .font_weight(FontWeight::NORMAL)
                    .line_height(relative(1.25))
                    .child(before.to_owned())
                    .child(
                        Button::new("choose-project")
                            .ghost()
                            .large()
                            .px_0()
                            .min_w_0()
                            .max_w(rems(16.))
                            .tooltip(path.to_string_lossy().into_owned())
                            .accessibility_label(t(cx, "conversation-project"))
                            .disabled(session.busy() || session.command.running())
                            .child(
                                h_flex()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        Label::new(name)
                                            .text_2xl()
                                            .line_height(relative(1.25))
                                            .min_w_0()
                                            .truncate(),
                                    )
                                    .child(Icon::new(IconName::ChevronDown).size_4().flex_none()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.pick_directory(cx))),
                    )
                    .child(after.to_owned()),
            )
            .into_any_element()
    }
}
