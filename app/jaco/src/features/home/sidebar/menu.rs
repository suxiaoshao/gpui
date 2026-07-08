use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
    state,
};
use fluent_bundle::FluentArgs;
use gpui::*;
use gpui_component::{
    WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    input::{Input, InputState},
    menu::{PopupMenu, PopupMenuItem},
    notification::{Notification, NotificationType},
    v_flex,
};

use crate::state::workspace::{SidebarConversationNode, SidebarProjectHeader};

pub(super) fn project_popup_menu(
    menu: PopupMenu,
    project: SidebarProjectHeader,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let i18n = cx.global::<I18n>();
    let pin_label = if project.pinned {
        i18n.t("sidebar-project-unpin")
    } else {
        i18n.t("sidebar-project-pin")
    };
    let show_label = i18n.t(show_project_label_key());
    let rename_label = i18n.t("sidebar-project-rename");
    let remove_label = i18n.t("sidebar-project-remove");
    let project_for_pin = project.clone();
    let project_for_show = project.clone();
    let project_for_rename = project.clone();
    let project_for_remove = project;

    menu.item(
        PopupMenuItem::new(pin_label)
            .icon(if project_for_pin.pinned {
                IconName::PinOff
            } else {
                IconName::Pin
            })
            .on_click(move |_, _window, cx| {
                let workspace = state::workspace::workspace(cx);
                let project_id = project_for_pin.id.clone();
                let pinned = !project_for_pin.pinned;
                let _ = workspace.update(cx, |workspace, cx| {
                    workspace.pin_project(&project_id, pinned, cx)
                });
            }),
    )
    .item(
        PopupMenuItem::new(show_label)
            .icon(IconName::FolderOpen)
            .on_click(move |_, _window, cx| {
                cx.open_with_system(&project_for_show.path);
            }),
    )
    .item(
        PopupMenuItem::new(rename_label)
            .icon(IconName::Pencil)
            .on_click(move |_, window, cx| {
                open_rename_project_dialog(project_for_rename.clone(), window, cx);
            }),
    )
    .item(PopupMenuItem::separator())
    .item(
        PopupMenuItem::new(remove_label)
            .icon(IconName::FolderMinus)
            .on_click(move |_, window, cx| {
                open_remove_project_confirm(project_for_remove.clone(), window, cx);
            }),
    )
}

pub(super) fn open_delete_conversation_confirm(
    conversation: SidebarConversationNode,
    window: &mut Window,
    cx: &mut App,
) {
    let mut args = FluentArgs::new();
    args.set("title", conversation.title.as_ref().to_string());
    let title = cx.global::<I18n>().t("sidebar-delete-conversation-title");
    let message = cx
        .global::<I18n>()
        .t_with_args("sidebar-delete-conversation-message", &args);
    let conversation_id = conversation.id;

    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        move |window, cx| {
            let workspace = state::workspace::workspace(cx);
            if let Err(err) = workspace.update(cx, |workspace, cx| {
                workspace.delete_conversation(&conversation_id, cx)
            }) {
                push_sidebar_error(
                    window,
                    cx,
                    cx.global::<I18n>().t("sidebar-delete-conversation-failed"),
                    err.to_string(),
                );
            }
        },
        window,
        cx,
    );
}

fn open_rename_project_dialog(project: SidebarProjectHeader, window: &mut Window, cx: &mut App) {
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(project.display_name.to_string())
            .placeholder(cx.global::<I18n>().t("sidebar-rename-project-placeholder"))
    });
    let input_to_focus = input.clone();
    let project_id = project.id;
    let title = cx.global::<I18n>().t("sidebar-rename-project-title");

    window.open_dialog(cx, move |dialog, _window, _cx| {
        let input = input.clone();
        dialog
            .title(title.clone())
            .w(px(420.))
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .child(Input::new(&input).w_full()),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new("rename-project-cancel")
                                .label(_cx.global::<I18n>().t("button-cancel")),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("rename-project-submit")
                                .primary()
                                .label(_cx.global::<I18n>().t("provider-action-save"))
                                .on_click({
                                    let input = input.clone();
                                    let project_id = project_id.clone();
                                    move |_, window, cx| {
                                        let display_name =
                                            input.read(cx).value().trim().to_string();
                                        if display_name.is_empty() {
                                            return;
                                        }
                                        let workspace = state::workspace::workspace(cx);
                                        match workspace.update(cx, |workspace, cx| {
                                            workspace.rename_project(
                                                &project_id,
                                                display_name.clone(),
                                                cx,
                                            )
                                        }) {
                                            Ok(_) => window.close_dialog(cx),
                                            Err(err) => push_sidebar_error(
                                                window,
                                                cx,
                                                cx.global::<I18n>()
                                                    .t("sidebar-rename-project-failed"),
                                                err.to_string(),
                                            ),
                                        }
                                    }
                                }),
                        ),
                    ),
            )
    });

    window.defer(cx, move |window, cx| {
        input_to_focus.update(cx, |input, cx| input.focus(window, cx));
    });
}

fn open_remove_project_confirm(project: SidebarProjectHeader, window: &mut Window, cx: &mut App) {
    let mut args = FluentArgs::new();
    args.set("name", project.display_name.as_ref().to_string());
    let title = cx.global::<I18n>().t("sidebar-remove-project-title");
    let message = cx
        .global::<I18n>()
        .t_with_args("sidebar-remove-project-message", &args);
    let project_id = project.id;

    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        move |window, cx| {
            let workspace = state::workspace::workspace(cx);
            if let Err(err) = workspace.update(cx, |workspace, cx| {
                workspace.remove_project(&project_id, cx)
            }) {
                push_sidebar_error(
                    window,
                    cx,
                    cx.global::<I18n>().t("sidebar-remove-project-failed"),
                    err.to_string(),
                );
            }
        },
        window,
        cx,
    );
}

fn push_sidebar_error(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
) {
    window.push_notification(
        Notification::new()
            .title(title.into())
            .message(message.into())
            .with_type(NotificationType::Error),
        cx,
    );
}

fn show_project_label_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "sidebar-project-show-in-finder"
    } else if cfg!(target_os = "windows") {
        "sidebar-project-show-in-explorer"
    } else {
        "sidebar-project-show-in-file-manager"
    }
}
