use std::path::PathBuf;

use gpui::{
    App, AppContext as _, ClipboardItem, Context, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, ScrollHandle, SharedString, StatefulInteractiveElement as _,
    Styled as _, Task, Window, div, px,
};
use gpui_component::{
    ActiveTheme, Sizable, StyledExt, WindowExt as _,
    button::Button,
    dialog::{DialogClose, DialogFooter},
    h_flex,
    label::Label,
    scroll::ScrollableElement as _,
    tag::Tag,
    text::TextView,
    v_flex,
};

use crate::{
    foundation::{I18n, assets::IconName},
    state::{self, skills::GlobalSkillEntry},
};

use super::{completion::skill_source_label, token::ComposerSkill};

enum SkillDetailContent {
    Loading,
    Loaded {
        content: SharedString,
        content_sha256: SharedString,
    },
    Failed {
        message: SharedString,
    },
}

pub(super) struct SkillDetailDialog {
    skill: ComposerSkill,
    content: SkillDetailContent,
    content_scroll_handle: ScrollHandle,
    _load_task: Task<()>,
}

impl SkillDetailDialog {
    fn new(skill: ComposerSkill, cx: &mut Context<Self>) -> Self {
        let entry = skill_to_entry(&skill);
        let load = cx.background_spawn(async move { state::skills::load_skill_content(entry) });
        let _load_task = cx.spawn(async move |this, cx| {
            let result = load.await;
            this.update(cx, |this, cx| {
                this.content = match result {
                    Ok(content) => SkillDetailContent::Loaded {
                        content: content.content.into(),
                        content_sha256: content.content_sha256.into(),
                    },
                    Err(err) => SkillDetailContent::Failed {
                        message: err.to_string().into(),
                    },
                };
                cx.notify();
            })
            .ok();
        });

        Self {
            skill,
            content: SkillDetailContent::Loading,
            content_scroll_handle: ScrollHandle::default(),
            _load_task,
        }
    }
}

impl Render for SkillDetailDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<I18n>();
        let source_label = skill_source_label(self.skill.source_kind, i18n);
        let description = self
            .skill
            .description
            .clone()
            .unwrap_or_else(|| i18n.t("skill-description-empty").to_string());
        let path = self.skill.skill_file_path.clone();

        v_flex()
            .w_full()
            .gap_3()
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Label::new(self.skill.name.clone())
                                    .text_lg()
                                    .font_semibold(),
                            )
                            .child(Tag::secondary().small().outline().child(source_label)),
                    )
                    .child(
                        Label::new(description)
                            .text_sm()
                            .line_height(gpui::relative(1.4))
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(path_chip(path.into(), cx)),
            )
            .child(self.render_content(cx))
    }
}

impl SkillDetailDialog {
    fn render_content(&mut self, cx: &mut App) -> gpui::AnyElement {
        match &self.content {
            SkillDetailContent::Loading => h_flex()
                .w_full()
                .h(px(280.))
                .items_center()
                .justify_center()
                .gap_2()
                .text_color(cx.theme().muted_foreground)
                .child(gpui_component::Icon::new(IconName::RefreshCcw).with_size(px(16.)))
                .child(Label::new(cx.global::<I18n>().t("skill-content-loading")).text_sm())
                .into_any_element(),
            SkillDetailContent::Failed { message } => h_flex()
                .w_full()
                .items_start()
                .gap_2()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().danger.opacity(0.35))
                .bg(cx.theme().tokens.danger.background.opacity(0.08))
                .p_3()
                .text_color(cx.theme().danger)
                .child(gpui_component::Icon::new(IconName::CircleAlert).with_size(px(16.)))
                .child(
                    Label::new(message.clone())
                        .text_sm()
                        .line_height(gpui::relative(1.4)),
                )
                .into_any_element(),
            SkillDetailContent::Loaded {
                content,
                content_sha256,
            } => {
                let hash_label = format!("sha256: {}", short_hash(content_sha256));
                let scroll_handle = self.content_scroll_handle.clone();
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        Label::new(hash_label)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(420.))
                            .relative()
                            .occlude()
                            .overflow_hidden()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().tokens.background.background)
                            .child(
                                div()
                                    .id("skill-detail-content-scroll")
                                    .size_full()
                                    .track_scroll(&scroll_handle)
                                    .overflow_y_scroll()
                                    .p_3()
                                    .child(
                                        TextView::markdown(
                                            "skill-detail-content",
                                            content.to_string(),
                                        )
                                        .selectable(true),
                                    ),
                            )
                            .vertical_scrollbar(&scroll_handle),
                    )
                    .into_any_element()
            }
        }
    }
}

pub(super) fn open_skill_detail_dialog(skill: ComposerSkill, window: &mut Window, cx: &mut App) {
    let title = cx.global::<I18n>().t("skill-detail-dialog-title");
    let close_label = cx.global::<I18n>().t("button-close");
    let copy_path_label = cx.global::<I18n>().t("button-copy-path");
    let path = skill.skill_file_path.clone();
    let detail = cx.new(|cx| SkillDetailDialog::new(skill, cx));

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(title.clone())
            .w(px(720.))
            .child(detail.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("skill-detail-copy-path")
                            .icon(IconName::Copy)
                            .label(copy_path_label.clone())
                            .on_click({
                                let path = path.clone();
                                move |_, _window, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                                }
                            }),
                    )
                    .child(
                        DialogClose::new()
                            .child(Button::new("skill-detail-close").label(close_label.clone())),
                    ),
            )
    });
}

fn skill_to_entry(skill: &ComposerSkill) -> GlobalSkillEntry {
    GlobalSkillEntry {
        name: skill.name.clone(),
        description: skill.description.clone(),
        source_kind: skill.source_kind,
        skill_file_path: PathBuf::from(&skill.skill_file_path),
        directory_path: PathBuf::from(&skill.directory_path),
        search_text: String::new(),
    }
}

fn path_chip(path: SharedString, cx: &mut App) -> gpui::AnyElement {
    h_flex()
        .max_w_full()
        .min_w_0()
        .rounded(cx.theme().radius)
        .bg(cx.theme().tokens.muted.background)
        .px_2()
        .py_1()
        .child(
            Label::new(path)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .truncate(),
        )
        .into_any_element()
}

fn short_hash(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

#[cfg(test)]
mod tests {
    use super::short_hash;

    #[test]
    fn short_hash_clips_long_hashes() {
        assert_eq!(short_hash("1234567890abcdef"), "1234567890ab");
        assert_eq!(short_hash("short"), "short");
    }
}
