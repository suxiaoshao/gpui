use std::rc::Rc;

use crate::{
    foundation::{I18n, assets::IconName},
    state,
};
use ai_chat_core::ShortcutId;
use ai_chat_db::{PromptRecord, ProviderModelRecord, ProviderRecord, ShortcutRecord};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, WindowExt as NotificationWindowExt,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};

use super::{layout::settings_empty_message, push_settings_error};
use choices::PromptChoice;
use dialog::{
    ShortcutDialogChoices, ShortcutEditMode, input_source_choices, open_shortcut_delete_confirm,
    open_shortcut_edit_dialog, open_shortcut_preview_dialog,
};
use rows::{
    ShortcutManagementEntry, ShortcutManagementRow, filter_shortcut_rows, shortcut_management_rows,
};

mod choices;
mod dialog;
mod rows;
mod validation;

pub(super) struct ShortcutsSettingsPage {
    search_input: Entity<InputState>,
    snapshot: Result<ShortcutSettingsSnapshot, String>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
struct ShortcutSettingsSnapshot {
    shortcuts: Vec<ShortcutRecord>,
    prompts: Vec<PromptRecord>,
    providers: Vec<(ProviderRecord, Vec<ProviderModelRecord>)>,
    model_choices: Vec<state::providers::ProviderModelChoice>,
    diagnostics: state::hotkey::ShortcutRuntimeDiagnostics,
}

impl ShortcutsSettingsPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(cx.global::<I18n>().t("shortcut-search-placeholder"))
        });
        let snapshot = match Self::load_snapshot(cx) {
            Ok(snapshot) => Ok(snapshot),
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-shortcuts-failed");
                let message = err.to_string();
                push_settings_error(window, cx, title, message.clone());
                Err(message)
            }
        };
        let search_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        let shortcut_catalog = state::shortcuts::catalog(cx);
        let shortcut_subscription =
            cx.subscribe_in(&shortcut_catalog, window, Self::on_shortcut_catalog_event);
        let prompt_catalog = state::prompts::catalog(cx);
        let prompt_subscription = prompt_catalog.observe_select_in(
            cx,
            window,
            |state| state.prompts().to_vec(),
            |page, _catalog, window, cx| {
                page.reload_snapshot(window, cx);
            },
        );
        let provider_catalog = state::providers::catalog(cx);
        let provider_subscription =
            cx.subscribe_in(&provider_catalog, window, Self::on_provider_catalog_event);

        Self {
            search_input,
            snapshot,
            _subscriptions: vec![
                search_subscription,
                shortcut_subscription,
                prompt_subscription,
                provider_subscription,
            ],
        }
    }

    fn load_snapshot(cx: &App) -> ai_chat_db::Result<ShortcutSettingsSnapshot> {
        Ok(ShortcutSettingsSnapshot {
            shortcuts: state::shortcuts::list_shortcuts(cx)?,
            prompts: state::prompts::list_prompts(cx)?,
            providers: state::providers::providers_with_models(cx)?,
            model_choices: state::providers::enabled_provider_models(cx)?,
            diagnostics: state::GlobalHotkeyState::diagnostics_snapshot(cx),
        })
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

    fn on_shortcut_catalog_event(
        &mut self,
        _: &Entity<state::shortcuts::ShortcutCatalogStore>,
        _: &state::shortcuts::ShortcutCatalogEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reload_snapshot(window, cx);
    }

    fn on_provider_catalog_event(
        &mut self,
        _: &Entity<state::providers::ProviderCatalogStore>,
        _: &state::providers::ProviderCatalogEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reload_snapshot(window, cx);
    }

    fn reload_snapshot(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match Self::load_snapshot(cx) {
            Ok(snapshot) => {
                self.snapshot = Ok(snapshot);
                cx.notify();
                true
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-shortcuts-failed");
                let message = err.to_string();
                self.snapshot = Err(message.clone());
                push_settings_error(window, cx, title, message);
                cx.notify();
                false
            }
        }
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn shortcut_by_id(&self, shortcut_id: &ShortcutId) -> Option<&ShortcutRecord> {
        self.snapshot
            .as_ref()
            .ok()?
            .shortcuts
            .iter()
            .find(|shortcut| &shortcut.id == shortcut_id)
    }

    fn row_by_id(&self, shortcut_id: &ShortcutId, cx: &App) -> Option<ShortcutManagementRow> {
        let snapshot = self.snapshot.as_ref().ok()?;
        shortcut_management_rows(
            &snapshot.shortcuts,
            &snapshot.prompts,
            &snapshot.providers,
            &snapshot.diagnostics,
            cx.global::<I18n>(),
        )
        .into_iter()
        .find(|row| &row.id == shortcut_id)
    }

    fn dialog_choices(&self, cx: &App) -> ShortcutDialogChoices {
        let Some(snapshot) = self.snapshot.as_ref().ok() else {
            return ShortcutDialogChoices {
                prompts: Vec::new(),
                models: Vec::new(),
                input_sources: input_source_choices(cx),
            };
        };
        let mut prompts = Vec::new();
        prompts.push(PromptChoice::none(
            cx.global::<I18n>().t("shortcut-prompt-none"),
        ));
        prompts.extend(
            snapshot
                .prompts
                .iter()
                .filter(|prompt| prompt.enabled)
                .map(PromptChoice::from_prompt),
        );
        ShortcutDialogChoices {
            prompts,
            models: snapshot.model_choices.clone(),
            input_sources: input_source_choices(cx),
        }
    }

    fn open_add_shortcut_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let existing_shortcuts = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.shortcuts.clone())
            .unwrap_or_default();
        let temporary_hotkey = self
            .snapshot
            .as_ref()
            .ok()
            .and_then(|snapshot| snapshot.diagnostics.temporary_hotkey.clone());
        open_shortcut_edit_dialog(
            ShortcutEditMode::Create,
            None,
            self.dialog_choices(cx),
            existing_shortcuts,
            temporary_hotkey,
            window,
            cx,
        );
    }

    fn open_view_shortcut_dialog(
        &mut self,
        shortcut_id: ShortcutId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(shortcut) = self.shortcut_by_id(&shortcut_id).cloned() else {
            let title = cx.global::<I18n>().t("notify-load-shortcuts-failed");
            push_settings_error(window, cx, title, shortcut_id);
            return;
        };
        let Some(row) = self.row_by_id(&shortcut.id, cx) else {
            let title = cx.global::<I18n>().t("notify-load-shortcuts-failed");
            push_settings_error(window, cx, title, shortcut.id);
            return;
        };
        let page_for_edit = cx.entity().downgrade();
        open_shortcut_preview_dialog(
            shortcut,
            row,
            window,
            cx,
            Rc::new(move |shortcut, window, cx| {
                let _ = page_for_edit.update(cx, |page, cx| {
                    page.open_edit_shortcut_record(shortcut, window, cx);
                });
            }),
            Rc::new(move |shortcut, window, cx| {
                open_shortcut_delete_confirm(shortcut, window, cx);
            }),
        );
    }

    fn open_edit_shortcut_dialog(
        &mut self,
        shortcut_id: ShortcutId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(shortcut) = self.shortcut_by_id(&shortcut_id).cloned() else {
            let title = cx.global::<I18n>().t("notify-load-shortcuts-failed");
            push_settings_error(window, cx, title, shortcut_id);
            return;
        };
        self.open_edit_shortcut_record(shortcut, window, cx);
    }

    fn open_edit_shortcut_record(
        &mut self,
        shortcut: ShortcutRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let existing_shortcuts = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.shortcuts.clone())
            .unwrap_or_default();
        let temporary_hotkey = self
            .snapshot
            .as_ref()
            .ok()
            .and_then(|snapshot| snapshot.diagnostics.temporary_hotkey.clone());
        open_shortcut_edit_dialog(
            ShortcutEditMode::Edit,
            Some(shortcut),
            self.dialog_choices(cx),
            existing_shortcuts,
            temporary_hotkey,
            window,
            cx,
        );
    }

    fn open_delete_shortcut_dialog(
        &mut self,
        shortcut_id: ShortcutId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(shortcut) = self.shortcut_by_id(&shortcut_id).cloned() else {
            let title = cx.global::<I18n>().t("notify-load-shortcuts-failed");
            push_settings_error(window, cx, title, shortcut_id);
            return;
        };
        open_shortcut_delete_confirm(shortcut, window, cx);
    }

    fn toggle_shortcut_enabled(
        &mut self,
        shortcut_id: ShortcutId,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(err) = state::shortcuts::set_shortcut_enabled(cx, &shortcut_id, enabled) {
            let title = cx.global::<I18n>().t("notify-save-shortcut-failed");
            push_settings_error(window, cx, title, err);
        }
    }

    fn reregister_shortcut(
        &mut self,
        shortcut_id: ShortcutId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match state::shortcuts::reregister_shortcut(cx, &shortcut_id) {
            Ok(_) => {
                window.push_notification(
                    Notification::new()
                        .title(cx.global::<I18n>().t("notify-shortcut-reregistered"))
                        .with_type(NotificationType::Success),
                    cx,
                );
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-shortcut-register-failed");
                push_settings_error(window, cx, title, err);
            }
        }
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
                Button::new("shortcut-settings-add")
                    .icon(IconName::Plus)
                    .label(cx.global::<I18n>().t("button-add-shortcut"))
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_add_shortcut_dialog(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_shortcut_entry(
        &self,
        row: ShortcutManagementRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view_page = cx.entity().downgrade();
        let edit_page = view_page.clone();
        let reregister_page = view_page.clone();
        let delete_page = view_page.clone();
        let toggle_page = view_page.clone();

        ShortcutManagementEntry::new(row)
            .on_view(move |shortcut_id, window, cx| {
                let _ = view_page.update(cx, |page, cx| {
                    page.open_view_shortcut_dialog(shortcut_id, window, cx);
                });
            })
            .on_edit(move |shortcut_id, window, cx| {
                let _ = edit_page.update(cx, |page, cx| {
                    page.open_edit_shortcut_dialog(shortcut_id, window, cx);
                });
            })
            .on_reregister(move |shortcut_id, window, cx| {
                let _ = reregister_page.update(cx, |page, cx| {
                    page.reregister_shortcut(shortcut_id, window, cx);
                });
            })
            .on_delete(move |shortcut_id, window, cx| {
                let _ = delete_page.update(cx, |page, cx| {
                    page.open_delete_shortcut_dialog(shortcut_id, window, cx);
                });
            })
            .on_toggle(move |shortcut_id, enabled, window, cx| {
                let _ = toggle_page.update(cx, |page, cx| {
                    page.toggle_shortcut_enabled(shortcut_id, enabled, window, cx);
                });
            })
            .into_any_element()
    }

    fn render_shortcut_rows(
        &self,
        snapshot: &ShortcutSettingsSnapshot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entries = shortcut_management_rows(
            &snapshot.shortcuts,
            &snapshot.prompts,
            &snapshot.providers,
            &snapshot.diagnostics,
            cx.global::<I18n>(),
        );
        let query = self.current_query(cx);
        let filtered = filter_shortcut_rows(&entries, &query);

        if filtered.is_empty() {
            return v_flex()
                .w_full()
                .min_h(px(220.))
                .items_center()
                .justify_center()
                .child(
                    Label::new(cx.global::<I18n>().t("shortcut-search-empty"))
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
                    .map(|row| self.render_shortcut_entry(row, cx))
                    .collect::<Vec<_>>(),
            )
            .into_any_element()
    }

    fn render_empty_shortcuts(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .w_full()
            .min_h(px(260.))
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                Label::new(cx.global::<I18n>().t("shortcut-empty"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Button::new("shortcut-settings-empty-add")
                    .icon(IconName::Plus)
                    .label(cx.global::<I18n>().t("button-add-shortcut"))
                    .small()
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_add_shortcut_dialog(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match &self.snapshot {
            Err(err) => {
                let load_failed = cx.global::<I18n>().t("notify-load-shortcuts-failed");
                settings_empty_message(format!("{load_failed}: {err}"))
            }
            Ok(snapshot) if snapshot.shortcuts.is_empty() => self.render_empty_shortcuts(cx),
            Ok(snapshot) => self.render_shortcut_rows(snapshot, window, cx),
        }
    }
}

impl Render for ShortcutsSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_3()
            .child(self.render_toolbar(window, cx))
            .child(self.render_body(window, cx))
    }
}
