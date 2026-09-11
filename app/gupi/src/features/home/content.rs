use super::*;
use crate::state::conversation::content::LoadStage;
use gpui_kit::component::{skeleton::Skeleton, spinner::Spinner};

fn loading_label(stage: LoadStage) -> &'static str {
    match stage {
        LoadStage::CheckingFile => "conversation-checking-file",
        LoadStage::Connecting => "conversation-connecting",
        LoadStage::History => "conversation-history-loading",
    }
}

pub(super) fn skeleton(stage: LoadStage, cx: &App) -> AnyElement {
    v_flex()
        .flex_1()
        .min_h_0()
        .w_full()
        .overflow_hidden()
        .p_5()
        .gap_6()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t(cx, loading_label(stage))),
        )
        .children([0.7, 1.0, 0.85].into_iter().map(|width| {
            v_flex()
                .w_full()
                .max_w(px(820.))
                .gap_3()
                .child(Skeleton::new().w(px(96.)).h_4())
                .child(Skeleton::new().w(relative(width)).h_4())
                .child(Skeleton::new().w(relative(width * 0.8)).h_4())
                .child(Skeleton::new().w(relative(width * 0.6)).h_4())
        }))
        .into_any_element()
}

pub(super) fn refreshing(stage: LoadStage, cx: &App) -> AnyElement {
    let label = match stage {
        LoadStage::History => "conversation-history-refreshing",
        LoadStage::CheckingFile | LoadStage::Connecting => loading_label(stage),
    };
    h_flex()
        .px_4()
        .py_2()
        .gap_2()
        .child(Spinner::new().small())
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(t(cx, label)),
        )
        .into_any_element()
}

impl HomeView {
    pub(super) fn render_content_error(
        &self,
        id: &'static str,
        error: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w_full()
            .min_w_0()
            .items_start()
            .p_4()
            .gap_2()
            .child(
                div()
                    .w_full()
                    .whitespace_normal()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.to_owned()),
            )
            .child(
                Button::new(id)
                    .small()
                    .label(t(cx, "conversation-reconnect"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(key) = this.shown_key.clone() {
                            this.state.update(cx, |s, cx| {
                                s.connect(&key, cx);
                                s.refresh(&key, cx);
                            });
                        }
                    })),
            )
            .into_any_element()
    }
}
