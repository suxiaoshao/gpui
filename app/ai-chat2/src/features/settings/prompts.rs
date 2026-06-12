use crate::{
    foundation::{I18n, assets::IconName},
    state,
};
use ai_chat_core::PromptId;
use ai_chat_db::PromptRecord;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    v_flex,
};

use super::{layout::settings_empty_message, push_settings_error};
use dialog::{PromptEditMode, open_prompt_delete_confirm, open_prompt_edit_dialog};
use rows::{
    PromptManagementEntry, PromptManagementRow, filter_prompt_entries, prompt_management_entries,
};

mod dialog;
mod rows;

pub(super) struct PromptsSettingsPage {
    search_input: Entity<InputState>,
    prompts: Result<Vec<PromptRecord>, String>,
    _subscriptions: Vec<Subscription>,
}

impl PromptsSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("prompt-search-placeholder"))
        });
        let prompts = match Self::load_prompts(cx) {
            Ok(prompts) => Ok(prompts),
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-prompts-failed");
                let message = err.to_string();
                push_settings_error(window, cx, title, message.clone());
                Err(message)
            }
        };
        let search_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        let prompt_catalog = state::prompts::catalog(cx);
        let catalog_subscription =
            cx.subscribe_in(&prompt_catalog, window, Self::on_prompt_catalog_event);

        Self {
            search_input,
            prompts,
            _subscriptions: vec![search_subscription, catalog_subscription],
        }
    }

    fn load_prompts(cx: &App) -> ai_chat_db::Result<Vec<PromptRecord>> {
        state::prompts::list_prompts(cx)
    }

    fn on_search_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
        }
    }

    fn on_prompt_catalog_event(
        &mut self,
        _: &Entity<state::prompts::PromptCatalogStore>,
        _: &state::prompts::PromptCatalogEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reload_prompts(window, cx);
    }

    fn reload_prompts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match Self::load_prompts(cx) {
            Ok(prompts) => {
                self.prompts = Ok(prompts);
                cx.notify();
                true
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-prompts-failed");
                let message = err.to_string();
                self.prompts = Err(message.clone());
                push_settings_error(window, cx, title, message);
                cx.notify();
                false
            }
        }
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn prompt_by_id(&self, prompt_id: &PromptId) -> Option<&PromptRecord> {
        self.prompts
            .as_ref()
            .ok()?
            .iter()
            .find(|prompt| &prompt.id == prompt_id)
    }

    fn open_add_prompt_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        open_prompt_edit_dialog(PromptEditMode::Create, None, window, cx);
    }

    fn open_view_prompt_dialog(
        &mut self,
        prompt_id: PromptId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.prompt_by_id(&prompt_id).cloned() else {
            let title = cx.global::<I18n>().t("notify-load-prompts-failed");
            push_settings_error(window, cx, title, prompt_id);
            return;
        };
        dialog::open_prompt_preview_dialog(prompt, window, cx);
    }

    fn open_edit_prompt_dialog(
        &mut self,
        prompt_id: PromptId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.prompt_by_id(&prompt_id).cloned() else {
            let title = cx.global::<I18n>().t("notify-load-prompts-failed");
            push_settings_error(window, cx, title, prompt_id);
            return;
        };
        open_prompt_edit_dialog(PromptEditMode::Edit, Some(prompt), window, cx);
    }

    fn open_delete_prompt_dialog(
        &mut self,
        prompt_id: PromptId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.prompt_by_id(&prompt_id).cloned() else {
            let title = cx.global::<I18n>().t("notify-load-prompts-failed");
            push_settings_error(window, cx, title, prompt_id);
            return;
        };
        open_prompt_delete_confirm(prompt, window, cx);
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                Input::new(&self.search_input)
                    .flex_1()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
                    .cleanable(true),
            )
            .child(
                Button::new("prompt-settings-add")
                    .icon(IconName::Plus)
                    .label(cx.global::<I18n>().t("button-add-prompt"))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_add_prompt_dialog(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_prompt_entry(&self, row: PromptManagementRow, cx: &mut Context<Self>) -> AnyElement {
        let view_page = cx.entity().downgrade();
        let edit_page = view_page.clone();
        let delete_page = view_page.clone();

        PromptManagementEntry::new(row)
            .on_view(move |prompt_id, window, cx| {
                let _ = view_page.update(cx, |page, cx| {
                    page.open_view_prompt_dialog(prompt_id, window, cx);
                });
            })
            .on_edit(move |prompt_id, window, cx| {
                let _ = edit_page.update(cx, |page, cx| {
                    page.open_edit_prompt_dialog(prompt_id, window, cx);
                });
            })
            .on_delete(move |prompt_id, window, cx| {
                let _ = delete_page.update(cx, |page, cx| {
                    page.open_delete_prompt_dialog(prompt_id, window, cx);
                });
            })
            .into_any_element()
    }

    fn render_prompt_rows(
        &self,
        prompts: &[PromptRecord],
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entries = prompt_management_entries(prompts);
        let query = self.current_query(cx);
        let filtered = filter_prompt_entries(&entries, &query);

        if filtered.is_empty() {
            return v_flex()
                .w_full()
                .min_h(px(220.))
                .items_center()
                .justify_center()
                .child(
                    Label::new(cx.global::<I18n>().t("prompt-search-empty"))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element();
        }

        v_flex()
            .w_full()
            .gap_2()
            .children(
                filtered
                    .into_iter()
                    .map(|row| self.render_prompt_entry(row, cx))
                    .collect::<Vec<_>>(),
            )
            .into_any_element()
    }

    fn render_empty_prompts(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .min_h(px(260.))
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                Label::new(cx.global::<I18n>().t("prompt-empty"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Button::new("prompt-settings-empty-add")
                    .icon(IconName::Plus)
                    .label(cx.global::<I18n>().t("button-add-prompt"))
                    .small()
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_add_prompt_dialog(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match &self.prompts {
            Err(err) => {
                let load_failed = cx.global::<I18n>().t("notify-load-prompts-failed");
                settings_empty_message(format!("{load_failed}: {err}"))
            }
            Ok(prompts) if prompts.is_empty() => self.render_empty_prompts(cx),
            Ok(prompts) => self.render_prompt_rows(prompts, window, cx),
        }
    }
}

impl Render for PromptsSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_3()
            .child(self.render_toolbar(window, cx))
            .child(self.render_body(window, cx))
    }
}
