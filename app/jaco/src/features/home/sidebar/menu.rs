use crate::{
    components::delete_confirm::{DestructiveAction, open_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
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

use super::{
    super::workspace::{HomeWorkspace, SidebarConversationNode, SidebarProjectHeader},
    actions::{
        ConversationSidebarAction, ConversationSidebarActions, ProjectSidebarAction,
        ProjectSidebarActions, SidebarActionGuardError,
    },
};

pub(super) fn project_popup_menu(
    menu: PopupMenu,
    actions: ProjectSidebarActions,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let project = actions.project();
    let i18n = cx.global::<I18n>();
    let pin_label = if project.pinned {
        i18n.t("sidebar-project-unpin")
    } else {
        i18n.t("sidebar-project-pin")
    };
    let new_actions = actions.clone();
    let pin_actions = actions.clone();
    let rename_actions = actions.clone();
    let reveal_actions = actions.clone();
    let archive_actions = actions.clone();
    let remove_actions = actions.clone();

    menu.item(
        PopupMenuItem::new(i18n.t("sidebar-project-new-conversation"))
            .disabled(!actions.availability(ProjectSidebarAction::NewConversation))
            .icon(IconName::SquarePen)
            .on_click(move |_, window, cx| {
                new_actions.invoke(ProjectSidebarAction::NewConversation, window, cx);
            }),
    )
    .item(
        PopupMenuItem::new(pin_label)
            .disabled(!actions.availability(ProjectSidebarAction::TogglePinned))
            .icon(if project.pinned {
                IconName::PinOff
            } else {
                IconName::Pin
            })
            .on_click(move |_, window, cx| {
                pin_actions.invoke(ProjectSidebarAction::TogglePinned, window, cx);
            }),
    )
    .item(
        PopupMenuItem::new(i18n.t("sidebar-project-rename"))
            .disabled(!actions.availability(ProjectSidebarAction::Rename))
            .icon(IconName::Pencil)
            .on_click(move |_, window, cx| {
                rename_actions.invoke(ProjectSidebarAction::Rename, window, cx);
            }),
    )
    .item(
        PopupMenuItem::new(i18n.t(show_project_label_key()))
            .disabled(!actions.availability(ProjectSidebarAction::RevealInFileManager))
            .icon(IconName::FolderOpen)
            .on_click(move |_, window, cx| {
                reveal_actions.invoke(ProjectSidebarAction::RevealInFileManager, window, cx);
            }),
    )
    .item(PopupMenuItem::separator())
    .item(
        PopupMenuItem::new(i18n.t("sidebar-project-archive-conversations"))
            .disabled(!actions.availability(ProjectSidebarAction::ArchiveConversations))
            .icon(IconName::Archive)
            .on_click(move |_, window, cx| {
                archive_actions.invoke(ProjectSidebarAction::ArchiveConversations, window, cx);
            }),
    )
    .item(PopupMenuItem::separator())
    .item(
        PopupMenuItem::new(i18n.t("sidebar-project-remove"))
            .disabled(!actions.availability(ProjectSidebarAction::Remove))
            .icon(IconName::FolderMinus)
            .on_click(move |_, window, cx| {
                remove_actions.invoke(ProjectSidebarAction::Remove, window, cx);
            }),
    )
}

pub(super) fn conversation_popup_menu(
    menu: PopupMenu,
    actions: ConversationSidebarActions,
    _window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let conversation = actions.conversation();
    let i18n = cx.global::<I18n>();
    let pin_label = if conversation.pinned {
        i18n.t("sidebar-conversation-unpin")
    } else {
        i18n.t("sidebar-conversation-pin")
    };
    let pin_actions = actions.clone();
    let rename_actions = actions.clone();
    let archive_actions = actions.clone();
    let copy_actions = actions.clone();

    menu.item(
        PopupMenuItem::new(pin_label)
            .disabled(!actions.availability(ConversationSidebarAction::TogglePinned))
            .icon(if conversation.pinned {
                IconName::PinOff
            } else {
                IconName::Pin
            })
            .on_click(move |_, window, cx| {
                pin_actions.invoke(ConversationSidebarAction::TogglePinned, window, cx);
            }),
    )
    .item(
        PopupMenuItem::new(i18n.t("sidebar-conversation-rename"))
            .disabled(!actions.availability(ConversationSidebarAction::Rename))
            .icon(IconName::Pencil)
            .on_click(move |_, window, cx| {
                rename_actions.invoke(ConversationSidebarAction::Rename, window, cx);
            }),
    )
    .item(
        PopupMenuItem::new(i18n.t("sidebar-conversation-archive"))
            .disabled(!actions.availability(ConversationSidebarAction::Archive))
            .icon(IconName::Archive)
            .on_click(move |_, window, cx| {
                archive_actions.invoke(ConversationSidebarAction::Archive, window, cx);
            }),
    )
    .item(PopupMenuItem::separator())
    .item(
        PopupMenuItem::new(i18n.t("sidebar-conversation-copy-working-directory"))
            .disabled(!actions.availability(ConversationSidebarAction::CopyWorkingDirectory))
            .icon(IconName::Copy)
            .on_click(move |_, window, cx| {
                copy_actions.invoke(ConversationSidebarAction::CopyWorkingDirectory, window, cx);
            }),
    )
}

pub(super) fn open_rename_project_dialog(
    project: SidebarProjectHeader,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
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
                                    let workspace = workspace.clone();
                                    move |_, window, cx| {
                                        let display_name =
                                            input.read(cx).value().trim().to_string();
                                        if display_name.is_empty() {
                                            return;
                                        }
                                        let target_id = project_id.clone();
                                        let task = workspace.update(cx, |workspace, cx| {
                                            workspace.rename_project(
                                                target_id.clone(),
                                                display_name.clone(),
                                                cx,
                                            )
                                        });
                                        let completion = window.spawn(cx, async move |cx| {
                                            let result = task.await;
                                            let _ = cx.update(|window, cx| match result {
                                                Ok(_) => window.close_dialog(cx),
                                                Err(error) => {
                                                    tracing::error!(
                                                        action = "project-rename",
                                                        target_kind = "project",
                                                        target_id = %target_id,
                                                        error = ?error,
                                                        "sidebar action failed"
                                                    );
                                                    show_sidebar_safe_error(
                                                        window,
                                                        cx,
                                                        "sidebar-rename-project-failed",
                                                    )
                                                }
                                            });
                                        });
                                        crate::app::tasks::retain_window(window, completion, cx);
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

pub(super) fn open_rename_conversation_dialog(
    conversation: SidebarConversationNode,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(conversation.title.to_string())
            .placeholder(
                cx.global::<I18n>()
                    .t("sidebar-rename-conversation-placeholder"),
            )
    });
    let input_to_focus = input.clone();
    let conversation_id = conversation.id;
    let title = cx.global::<I18n>().t("sidebar-rename-conversation-title");

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
                            Button::new("rename-conversation-cancel")
                                .label(_cx.global::<I18n>().t("button-cancel")),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new("rename-conversation-submit")
                                .primary()
                                .label(_cx.global::<I18n>().t("provider-action-save"))
                                .on_click({
                                    let input = input.clone();
                                    let conversation_id = conversation_id.clone();
                                    let workspace = workspace.clone();
                                    move |_, window, cx| {
                                        let value = input.read(cx).value().trim().to_string();
                                        if value.is_empty() {
                                            return;
                                        }
                                        let target_id = conversation_id.clone();
                                        let task = workspace.update(cx, |workspace, cx| {
                                            workspace.rename_conversation(
                                                target_id.clone(),
                                                value,
                                                cx,
                                            )
                                        });
                                        let completion = window.spawn(cx, async move |cx| {
                                            let result = task.await;
                                            let _ = cx.update(|window, cx| match result {
                                                Ok(_) => window.close_dialog(cx),
                                                Err(error) => {
                                                    tracing::error!(
                                                        action = "conversation-rename",
                                                        target_kind = "conversation",
                                                        target_id = %target_id,
                                                        error = ?error,
                                                        "sidebar action failed"
                                                    );
                                                    show_sidebar_safe_error(
                                                        window,
                                                        cx,
                                                        "sidebar-rename-conversation-failed",
                                                    )
                                                }
                                            });
                                        });
                                        crate::app::tasks::retain_window(window, completion, cx);
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

pub(super) fn open_archive_conversation_confirm(
    conversation: SidebarConversationNode,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut args = FluentArgs::new();
    args.set("title", conversation.title.to_string());
    let title = cx.global::<I18n>().t("sidebar-archive-conversation-title");
    let message = cx
        .global::<I18n>()
        .t_with_args("sidebar-archive-conversation-message", &args);
    let conversation_id = conversation.id;
    let project_id = conversation.project_id;

    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Archive,
        move |window, cx| {
            let conversation_id = conversation_id.clone();
            let project_id = project_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.archive_conversation(conversation_id.clone(), project_id, cx)
            });
            let completion = window.spawn(cx, async move |cx| {
                if let Err(error) = task.await {
                    tracing::warn!(
                        action = "conversation-archive",
                        target_kind = "conversation",
                        target_id = %conversation_id,
                        error = ?error,
                        "sidebar action failed"
                    );
                    let _ = cx.update(|window, cx| match error {
                        jaco_db::DbError::ConversationHasActiveRun { .. } => {
                            push_sidebar_notification(
                                window,
                                cx,
                                cx.global::<I18n>()
                                    .t("sidebar-archive-conversation-running-title"),
                                cx.global::<I18n>()
                                    .t("sidebar-archive-conversation-running-message"),
                                NotificationType::Warning,
                            );
                        }
                        _ => show_sidebar_safe_error(
                            window,
                            cx,
                            "sidebar-archive-conversation-failed",
                        ),
                    });
                }
            });
            crate::app::tasks::retain_window(window, completion, cx);
        },
        window,
        cx,
    );
}

pub(super) fn open_archive_project_conversations_confirm(
    project: SidebarProjectHeader,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut args = FluentArgs::new();
    args.set("name", project.display_name.to_string());
    let title = cx
        .global::<I18n>()
        .t("sidebar-project-archive-conversations-title");
    let message = cx
        .global::<I18n>()
        .t_with_args("sidebar-project-archive-conversations-message", &args);
    let project_id = project.id;

    open_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Archive,
        move |window, cx| {
            let project_id = project_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.archive_project_conversations(project_id.clone(), cx)
            });
            let completion = window.spawn(cx, async move |cx| {
                if let Err(error) = task.await {
                    tracing::warn!(
                        action = "project-archive-conversations",
                        target_kind = "project",
                        target_id = %project_id,
                        error = ?error,
                        "sidebar action failed"
                    );
                    let _ = cx.update(|window, cx| match error {
                        jaco_db::DbError::ConversationHasActiveRun { .. } => {
                            push_sidebar_notification(
                                window,
                                cx,
                                cx.global::<I18n>()
                                    .t("sidebar-project-archive-conversations-running-title"),
                                cx.global::<I18n>()
                                    .t("sidebar-project-archive-conversations-running-message"),
                                NotificationType::Warning,
                            );
                        }
                        _ => show_sidebar_safe_error(
                            window,
                            cx,
                            "sidebar-project-archive-conversations-failed",
                        ),
                    });
                }
            });
            crate::app::tasks::retain_window(window, completion, cx);
        },
        window,
        cx,
    );
}

pub(super) fn open_remove_project_confirm(
    project: SidebarProjectHeader,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut args = FluentArgs::new();
    args.set("name", project.display_name.to_string());
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
            let project_id = project_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.remove_project(project_id.clone(), cx)
            });
            let completion = window.spawn(cx, async move |cx| {
                if let Err(error) = task.await {
                    tracing::error!(
                        action = "project-remove",
                        target_kind = "project",
                        target_id = %project_id,
                        error = ?error,
                        "sidebar action failed"
                    );
                    let _ = cx.update(|window, cx| {
                        show_sidebar_safe_error(window, cx, "sidebar-remove-project-failed");
                    });
                }
            });
            crate::app::tasks::retain_window(window, completion, cx);
        },
        window,
        cx,
    );
}

pub(super) fn show_sidebar_guard_error(
    error: SidebarActionGuardError,
    window: &mut Window,
    cx: &mut App,
) {
    match error {
        SidebarActionGuardError::ResourceNotReady { .. } => push_sidebar_notification(
            window,
            cx,
            cx.global::<I18n>()
                .t("sidebar-action-resource-unavailable-title"),
            cx.global::<I18n>()
                .t("sidebar-action-resource-unavailable-message"),
            NotificationType::Warning,
        ),
        SidebarActionGuardError::TargetDisappeared { .. } => push_sidebar_notification(
            window,
            cx,
            cx.global::<I18n>()
                .t("sidebar-action-target-unavailable-title"),
            cx.global::<I18n>()
                .t("sidebar-action-target-unavailable-message"),
            NotificationType::Warning,
        ),
        SidebarActionGuardError::ClipboardVerificationFailed => push_sidebar_error(
            window,
            cx,
            cx.global::<I18n>().t("conversation-copy-failed"),
            cx.global::<I18n>().t("conversation-copy-failed-message"),
        ),
    }
}

pub(super) fn show_sidebar_copy_success(window: &mut Window, cx: &mut App) {
    push_sidebar_notification(
        window,
        cx,
        cx.global::<I18n>().t("conversation-copy-success"),
        "",
        NotificationType::Success,
    );
}

pub(super) fn show_sidebar_safe_error(window: &mut Window, cx: &mut App, title_key: &'static str) {
    push_sidebar_error(
        window,
        cx,
        cx.global::<I18n>().t(title_key),
        cx.global::<I18n>().t("sidebar-action-failed-message"),
    );
}

fn push_sidebar_error(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
) {
    push_sidebar_notification(window, cx, title, message, NotificationType::Error);
}

fn push_sidebar_notification(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    notification_type: NotificationType,
) {
    window.push_notification(
        Notification::new()
            .title(title.into())
            .message(message.into())
            .with_type(notification_type),
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

#[cfg(test)]
mod tests {
    use crate::foundation::{I18n, assets::IconName};
    use gpui::SharedString;
    use gpui_component::IconNamed;

    #[test]
    fn project_reveal_label_follows_platform() {
        assert_eq!(
            super::show_project_label_key(),
            if cfg!(target_os = "macos") {
                "sidebar-project-show-in-finder"
            } else if cfg!(target_os = "windows") {
                "sidebar-project-show-in-explorer"
            } else {
                "sidebar-project-show-in-file-manager"
            }
        );
    }

    #[test]
    fn conversation_action_contract_has_localized_labels_and_archive_icon() {
        for locale in ["en-US", "zh-CN"] {
            let i18n = I18n::for_locale_tag(locale);
            for key in [
                "sidebar-project-archive-conversations",
                "sidebar-project-archive-conversations-title",
                "sidebar-conversation-pin",
                "sidebar-conversation-unpin",
                "sidebar-conversation-archive",
                "sidebar-archive-conversation-title",
            ] {
                assert_ne!(i18n.t(key), key, "missing {key} in {locale}");
            }
        }

        assert_eq!(
            IconName::Archive.path(),
            SharedString::from("icons/archive.svg")
        );
    }
}
