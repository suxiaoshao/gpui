use crate::{
    foundation::{I18n, assets::IconName, search::field_matches_query},
    state::skills::GlobalSkillEntry,
};
use ai_chat_core::SkillSourceKind;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    h_flex,
    label::Label,
    scroll::ScrollableElement,
    tag::Tag,
    v_flex,
};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

type ToggleSkillContentHandler = Rc<dyn Fn(PathBuf, &mut Window, &mut App) + 'static>;
type ChainSkillContentScrollHandler = Rc<dyn Fn(Pixels, &mut Window, &mut App) + 'static>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SkillContentPanelState {
    Loading,
    Loaded {
        content: SharedString,
        content_sha256: SharedString,
    },
    Failed {
        message: SharedString,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillCatalogRow {
    pub(super) key: PathBuf,
    pub(super) entry: GlobalSkillEntry,
    name: SharedString,
    description: Option<SharedString>,
    source_label: SharedString,
    skill_file_path: SharedString,
    search_text: String,
}

#[derive(IntoElement, Clone)]
pub(super) struct SkillCatalogEntryView {
    row: SkillCatalogRow,
    content: Option<SkillContentPanelState>,
    on_toggle_content: ToggleSkillContentHandler,
    on_chain_content_scroll: ChainSkillContentScrollHandler,
}

impl SkillCatalogEntryView {
    pub(super) fn new(row: SkillCatalogRow, content: Option<SkillContentPanelState>) -> Self {
        Self {
            row,
            content,
            on_toggle_content: Rc::new(|_, _, _| {}),
            on_chain_content_scroll: Rc::new(|_, _, _| {}),
        }
    }

    pub(super) fn on_toggle_content(
        mut self,
        handler: impl Fn(PathBuf, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle_content = Rc::new(handler);
        self
    }

    pub(super) fn on_chain_content_scroll(
        mut self,
        handler: impl Fn(Pixels, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_chain_content_scroll = Rc::new(handler);
        self
    }
}

impl RenderOnce for SkillCatalogEntryView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            row,
            content,
            on_toggle_content,
            on_chain_content_scroll,
        } = self;
        let expanded = content.is_some();
        let toggle_label = if expanded {
            cx.global::<I18n>().t("button-hide-skill-content")
        } else {
            cx.global::<I18n>().t("button-view-skill-content")
        };
        let toggle_icon = if expanded {
            IconName::ChevronUp
        } else {
            IconName::ChevronDown
        };
        let row_key = row.key.clone();
        let stable_id = skill_row_id(&row.key);
        let description = row
            .description
            .unwrap_or_else(|| cx.global::<I18n>().t("skill-description-empty").into());

        let mut body =
            Collapsible::new()
                .w_full()
                .gap_3()
                .open(expanded)
                .child(
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(
                                    div().flex_1().min_w_0().child(
                                        Label::new(row.name)
                                            .w_full()
                                            .min_w_0()
                                            .text_sm()
                                            .font_medium()
                                            .truncate(),
                                    ),
                                )
                                .child(div().flex_none().child(
                                    Tag::secondary().small().outline().child(row.source_label),
                                )),
                        )
                        .child(
                            Label::new(description)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .line_height(relative(1.35)),
                        )
                        .child(path_chip(row.skill_file_path, cx)),
                )
                .child(
                    h_flex().w_full().justify_start().child(
                        Button::new(format!("skill-content-toggle-{stable_id}"))
                            .icon(toggle_icon)
                            .label(toggle_label)
                            .small()
                            .ghost()
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_toggle_content(row_key.clone(), window, cx);
                            }),
                    ),
                );

        if let Some(content) = content {
            body = body.content(render_content_state(
                content,
                &stable_id,
                on_chain_content_scroll,
                window,
                cx,
            ));
        }

        div().w_full().py_1().child(
            v_flex()
                .w_full()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().background)
                .p_3()
                .child(body),
        )
    }
}

pub(super) fn skill_catalog_rows(
    entries: &[GlobalSkillEntry],
    i18n: &I18n,
) -> Vec<SkillCatalogRow> {
    entries
        .iter()
        .map(|entry| SkillCatalogRow {
            key: entry.skill_file_path.clone(),
            entry: entry.clone(),
            name: entry.name.clone().into(),
            description: entry.description.clone().map(Into::into),
            source_label: skill_source_label(entry.source_kind, i18n),
            skill_file_path: entry.skill_file_path.to_string_lossy().to_string().into(),
            search_text: entry.search_text.clone(),
        })
        .collect()
}

pub(super) fn filter_skill_catalog_rows(
    rows: &[SkillCatalogRow],
    query: &str,
) -> Vec<SkillCatalogRow> {
    if query.trim().is_empty() {
        return rows.to_vec();
    }

    rows.iter()
        .filter(|row| field_matches_query(&row.search_text, query))
        .cloned()
        .collect()
}

pub(super) fn skill_catalog_list_items(rows: &[SkillCatalogRow]) -> Vec<PathBuf> {
    rows.iter().map(|row| row.key.clone()).collect()
}

fn render_content_state(
    content: SkillContentPanelState,
    stable_id: &str,
    on_chain_content_scroll: ChainSkillContentScrollHandler,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    match content {
        SkillContentPanelState::Loading => h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .text_color(cx.theme().muted_foreground)
            .child(Icon::new(IconName::RefreshCcw).with_size(px(16.)))
            .child(Label::new(cx.global::<I18n>().t("skill-content-loading")).text_sm())
            .into_any_element(),
        SkillContentPanelState::Failed { message } => h_flex()
            .w_full()
            .items_start()
            .gap_2()
            .text_color(cx.theme().danger)
            .child(Icon::new(IconName::CircleAlert).with_size(px(16.)))
            .child(Label::new(message).text_sm().line_height(relative(1.4)))
            .into_any_element(),
        SkillContentPanelState::Loaded {
            content,
            content_sha256,
        } => {
            let hash_label = format!("sha256: {}", short_hash(&content_sha256));
            let scroll_handle = window
                .use_keyed_state(format!("skill-content-scroll-{stable_id}"), cx, |_, _| {
                    ScrollHandle::default()
                })
                .read(cx)
                .clone();
            let content_scroll_handle = scroll_handle.clone();
            let chain_scroll = on_chain_content_scroll.clone();
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
                        .max_h(px(420.))
                        .relative()
                        .occlude()
                        .overflow_hidden()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .child(
                            div()
                                .id(format!("skill-content-scroll-area-{stable_id}"))
                                .max_h(px(420.))
                                .track_scroll(&scroll_handle)
                                .on_scroll_wheel(move |event, window, cx| {
                                    let delta = event.delta.pixel_delta(window.line_height());
                                    let current_offset = content_scroll_handle.offset();
                                    let max_offset = content_scroll_handle.max_offset();
                                    let requested_y = current_offset.y + delta.y;
                                    let next_offset = point(
                                        current_offset.x,
                                        requested_y.clamp(-max_offset.y, px(0.)),
                                    );
                                    let residual_y = requested_y - next_offset.y;

                                    if next_offset != current_offset {
                                        content_scroll_handle.set_offset(next_offset);
                                        window.refresh();
                                    }
                                    if residual_y != px(0.) {
                                        chain_scroll(-residual_y, window, cx);
                                    }
                                    cx.stop_propagation();
                                })
                                .p_3()
                                .child(
                                    div()
                                        .text_xs()
                                        .line_height(relative(1.45))
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .child(content),
                                ),
                        )
                        .vertical_scrollbar(&scroll_handle),
                )
                .into_any_element()
        }
    }
}

fn path_chip(path: SharedString, cx: &App) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted.opacity(0.32))
        .px_2()
        .py_1()
        .child(
            Label::new(path)
                .text_xs()
                .font_family(cx.theme().mono_font_family.clone())
                .text_color(cx.theme().muted_foreground)
                .truncate(),
        )
}

fn skill_source_label(source_kind: SkillSourceKind, i18n: &I18n) -> SharedString {
    i18n.t(match source_kind {
        SkillSourceKind::BuiltIn => "skill-source-builtin",
        SkillSourceKind::User => "skill-source-user",
        SkillSourceKind::Project => "skill-source-project",
        SkillSourceKind::Plugin => "skill-source-plugin",
    })
    .into()
}

fn skill_row_id(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        filter_skill_catalog_rows, short_hash, skill_catalog_list_items, skill_catalog_rows,
    };
    use crate::{foundation::I18n, state::skills::GlobalSkillEntry};
    use ai_chat_core::SkillSourceKind;
    use std::path::PathBuf;

    #[test]
    fn rows_filter_by_metadata() {
        let i18n = I18n::for_locale_tag("en-US");
        let entries = vec![entry(
            "browser",
            Some("Browser automation"),
            "/tmp/skills/browser/SKILL.md",
        )];
        let rows = skill_catalog_rows(&entries, &i18n);

        assert_eq!(filter_skill_catalog_rows(&rows, "browser").len(), 1);
        assert_eq!(filter_skill_catalog_rows(&rows, "automation").len(), 1);
        assert_eq!(filter_skill_catalog_rows(&rows, "skill.md").len(), 1);
        assert!(filter_skill_catalog_rows(&rows, "missing").is_empty());
    }

    #[test]
    fn list_items_stay_one_item_per_row_when_expanded() {
        let i18n = I18n::for_locale_tag("en-US");
        let entries = vec![
            entry("browser", None, "/tmp/skills/browser/SKILL.md"),
            entry("rust", None, "/tmp/skills/rust/SKILL.md"),
        ];
        let rows = skill_catalog_rows(&entries, &i18n);
        let expanded_path = PathBuf::from("/tmp/skills/browser/SKILL.md");
        assert_eq!(
            skill_catalog_list_items(&rows),
            vec![expanded_path, PathBuf::from("/tmp/skills/rust/SKILL.md"),]
        );
    }

    #[test]
    fn short_hash_uses_first_twelve_chars() {
        assert_eq!(short_hash("1234567890abcdef"), "1234567890ab");
    }

    fn entry(name: &str, description: Option<&str>, skill_file_path: &str) -> GlobalSkillEntry {
        let skill_file_path = PathBuf::from(skill_file_path);
        let directory_path = skill_file_path.parent().unwrap().to_path_buf();
        GlobalSkillEntry {
            name: name.to_string(),
            description: description.map(ToOwned::to_owned),
            source_kind: SkillSourceKind::User,
            skill_file_path,
            directory_path,
            search_text: format!(
                "{} {} {}",
                name,
                description.unwrap_or_default(),
                "user global skill.md"
            )
            .to_lowercase(),
        }
    }
}
