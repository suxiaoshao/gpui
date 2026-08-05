use fluent_bundle::FluentArgs;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogButtonProps,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};

use crate::{
    foundation::I18n,
    store::database::{self, DatabasePhase},
};

#[derive(IntoElement)]
pub(super) struct DatabaseResourcePage;

impl DatabaseResourcePage {
    pub(super) fn new() -> Self {
        Self
    }
}

impl RenderOnce for DatabaseResourcePage {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (phase, problem) = database::store(cx).read(cx, |resource| {
            (
                resource.phase(),
                resource.problem().map(ToString::to_string),
            )
        });
        let i18n = cx.global::<I18n>();
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(Label::new(match phase {
                DatabasePhase::Loading => i18n.t("database-state-loading"),
                DatabasePhase::Repairing => i18n.t("database-state-repairing"),
                DatabasePhase::Unavailable => i18n.t("database-state-unavailable"),
                DatabasePhase::Ready => i18n.t("database-state-ready"),
            }))
            .when_some(problem, |this, problem| {
                this.child(Label::new(problem).text_color(cx.theme().danger))
            })
            .when(phase == DatabasePhase::Unavailable, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("database-reopen")
                                .label(i18n.t("database-action-reopen"))
                                .on_click(|_, _, cx| database::request_reopen(cx)),
                        )
                        .child(
                            Button::new("database-backup-rebuild")
                                .danger()
                                .label(i18n.t("database-action-backup-rebuild"))
                                .on_click(|_, window, cx| confirm_backup_and_rebuild(window, cx)),
                        ),
                )
            })
    }
}

fn confirm_backup_and_rebuild(window: &mut Window, cx: &mut App) {
    let i18n = cx.global::<I18n>();
    let title = i18n.t("database-rebuild-title");
    let description = i18n.t("database-rebuild-description");
    let cancel = i18n.t("database-rebuild-cancel");
    let continue_label = i18n.t("database-rebuild-continue");
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(title.clone())
            .child(description.clone())
            .button_props(
                DialogButtonProps::default()
                    .show_cancel(true)
                    .cancel_text(cancel.clone())
                    .ok_text(continue_label.clone())
                    .on_ok(|_, window, cx| {
                        let initial = crate::store::get_data_url()
                            .ok()
                            .and_then(|path| path.parent().map(ToOwned::to_owned))
                            .unwrap_or_else(std::env::temp_dir);
                        let suggested = format!(
                            "feiwen-database-backup-{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                        );
                        let prompt = cx.prompt_for_new_path(&initial, Some(&suggested));
                        let handle = window.window_handle();
                        window
                            .spawn(cx, async move |cx| {
                                let Ok(Ok(Some(path))) = prompt.await else {
                                    return;
                                };
                                let _ = handle.update(cx, |_, _, cx| {
                                    database::request_backup_and_rebuild(path, cx);
                                });
                            })
                            .detach();
                        true
                    }),
            )
    });
}

pub(super) fn notify_backup_completed(path: &std::path::Path, window: &mut Window, cx: &mut App) {
    let i18n = cx.global::<I18n>();
    let mut args = FluentArgs::new();
    args.set("path", path.display().to_string());
    window.push_notification(
        Notification::new()
            .title(i18n.t("database-rebuild-success-title"))
            .message(i18n.t_with_args("database-rebuild-success-message", &args))
            .with_type(NotificationType::Success),
        cx,
    );
}
