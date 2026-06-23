use ai_chat_core::PromptId;
use ai_chat_db::PromptRecord;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    v_flex,
};
use std::rc::Rc;

use crate::foundation::{I18n, assets::IconName, search::field_matches_query};

type PromptAction = Rc<dyn Fn(PromptId, &mut Window, &mut App) + 'static>;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PromptManagementRow {
    pub(super) id: PromptId,
    name: SharedString,
    preview: SharedString,
    search_text: String,
}

#[derive(IntoElement, Clone)]
pub(super) struct PromptManagementEntry {
    row: PromptManagementRow,
    on_view: PromptAction,
    on_edit: PromptAction,
    on_delete: PromptAction,
}

impl PromptManagementEntry {
    pub(super) fn new(row: PromptManagementRow) -> Self {
        Self {
            row,
            on_view: Rc::new(|_, _, _| {}),
            on_edit: Rc::new(|_, _, _| {}),
            on_delete: Rc::new(|_, _, _| {}),
        }
    }

    pub(super) fn on_view(
        mut self,
        handler: impl Fn(PromptId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_view = Rc::new(handler);
        self
    }

    pub(super) fn on_edit(
        mut self,
        handler: impl Fn(PromptId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_edit = Rc::new(handler);
        self
    }

    pub(super) fn on_delete(
        mut self,
        handler: impl Fn(PromptId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete = Rc::new(handler);
        self
    }
}

impl RenderOnce for PromptManagementEntry {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let view_label = cx.global::<I18n>().t("button-view");
        let edit_label = cx.global::<I18n>().t("button-edit");
        let delete_label = cx.global::<I18n>().t("button-delete");

        let row_id = self.row.id.clone();
        let view_id = self.row.id.clone();
        let edit_id = self.row.id.clone();
        let delete_id = self.row.id.clone();
        let on_row_view = self.on_view.clone();
        let on_view = self.on_view.clone();
        let on_edit = self.on_edit.clone();
        let on_delete = self.on_delete.clone();

        h_flex()
            .id(format!("prompt-settings-row-{}", self.row.id))
            .w_full()
            .min_w_0()
            .items_center()
            .gap_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .px_3()
            .py_2()
            .cursor_pointer()
            .hover(|this| this.bg(cx.theme().accent.opacity(0.45)))
            .on_click(move |_, window, cx| on_row_view(row_id.clone(), window, cx))
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().border.opacity(0.35))
                    .child(Icon::new(IconName::FilePen).text_color(cx.theme().muted_foreground)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        Label::new(self.row.name)
                            .w_full()
                            .min_w_0()
                            .text_sm()
                            .font_medium()
                            .truncate(),
                    )
                    .child(
                        Label::new(self.row.preview)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(format!("prompt-settings-view-{view_id}"))
                            .icon(IconName::Eye)
                            .ghost()
                            .tooltip(view_label)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_view(view_id.clone(), window, cx);
                            }),
                    )
                    .child(
                        Button::new(format!("prompt-settings-edit-{edit_id}"))
                            .icon(IconName::Pencil)
                            .ghost()
                            .tooltip(edit_label)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_edit(edit_id.clone(), window, cx);
                            }),
                    )
                    .child(
                        Button::new(format!("prompt-settings-delete-{delete_id}"))
                            .icon(IconName::Trash)
                            .danger()
                            .tooltip(delete_label)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_delete(delete_id.clone(), window, cx);
                            }),
                    ),
            )
    }
}

pub(super) fn prompt_management_entries(prompts: &[PromptRecord]) -> Vec<PromptManagementRow> {
    prompts
        .iter()
        .map(|prompt| PromptManagementRow {
            id: prompt.id.clone(),
            name: prompt.name.clone().into(),
            preview: prompt_preview(&prompt.content.text).into(),
            search_text: prompt_search_text(prompt),
        })
        .collect()
}

pub(super) fn filter_prompt_entries(
    entries: &[PromptManagementRow],
    query: &str,
) -> Vec<PromptManagementRow> {
    let query = query.trim();
    if query.is_empty() {
        return entries.to_vec();
    }

    entries
        .iter()
        .filter(|entry| field_matches_query(&entry.search_text, query))
        .cloned()
        .collect()
}

pub(super) fn prompt_preview(content: &str) -> String {
    let text = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if text.chars().count() <= 96 {
        return text.to_string();
    }

    let mut preview = text.chars().take(95).collect::<String>();
    preview.push_str("...");
    preview
}

fn prompt_search_text(prompt: &PromptRecord) -> String {
    format!(
        "{} {} prompts prompt system developer instruction text 提示词 系统 开发者 指令 文本",
        prompt.name, prompt.content.text
    )
    .to_lowercase()
}

pub(super) fn prompt_updated_label(updated_at: time::OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        updated_at.year(),
        u8::from(updated_at.month()),
        updated_at.day(),
        updated_at.hour(),
        updated_at.minute()
    )
}

#[cfg(test)]
mod tests {
    use super::{filter_prompt_entries, prompt_management_entries};
    use ai_chat_core::PromptContent;
    use ai_chat_db::PromptRecord;
    use time::OffsetDateTime;

    #[test]
    fn filter_prompt_entries_returns_all_for_blank_query() {
        let entries = prompt_management_entries(&[
            prompt_record("prompt-1", "小说", "写奇幻冒险故事"),
            prompt_record("prompt-2", "命名助手", "生成更好的变量名"),
        ]);

        let filtered = filter_prompt_entries(&entries, "   ");

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, "prompt-1");
        assert_eq!(filtered[1].id, "prompt-2");
    }

    #[test]
    fn filter_prompt_entries_matches_name() {
        let entries = prompt_management_entries(&[
            prompt_record("prompt-1", "小说", "写奇幻冒险故事"),
            prompt_record("prompt-2", "命名助手", "生成更好的变量名"),
        ]);

        let filtered = filter_prompt_entries(&entries, "命名");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "prompt-2");
    }

    #[test]
    fn filter_prompt_entries_matches_content() {
        let entries = prompt_management_entries(&[
            prompt_record("prompt-1", "小说", "写奇幻冒险故事"),
            prompt_record("prompt-2", "命名助手", "生成更好的变量名"),
        ]);

        let filtered = filter_prompt_entries(&entries, "变量");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "prompt-2");
    }

    #[test]
    fn filter_prompt_entries_matches_chinese_pinyin() {
        let entries =
            prompt_management_entries(&[prompt_record("prompt-1", "命名助手", "生成更好的变量名")]);

        let filtered = filter_prompt_entries(&entries, "mingming");
        assert_eq!(filtered.len(), 1);

        let filtered = filter_prompt_entries(&entries, "mmzs");
        assert_eq!(filtered.len(), 1);

        let filtered = filter_prompt_entries(&entries, "shengcheng");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_prompt_entries_returns_empty_for_miss() {
        let entries =
            prompt_management_entries(&[prompt_record("prompt-1", "命名助手", "生成更好的变量名")]);

        let filtered = filter_prompt_entries(&entries, "not-found");

        assert!(filtered.is_empty());
    }

    fn prompt_record(id: &str, name: &str, text: &str) -> PromptRecord {
        PromptRecord {
            id: id.to_string(),
            name: name.to_string(),
            content: PromptContent {
                text: text.to_string(),
            },
            enabled: true,
            sort_order: 10,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}
