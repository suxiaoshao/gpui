use crate::{
    components::delete_confirm::{DestructiveAction, open_async_destructive_confirm_dialog},
    foundation::{I18n, assets::IconName},
};
use fluent_bundle::FluentArgs;
use gpui::*;
use gpui_component::{
    Disableable, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogAction, DialogClose, DialogFooter},
    input::{Input, InputState},
    menu::{PopupMenu, PopupMenuItem},
    notification::{Notification, NotificationType},
    v_flex,
};
use std::rc::Rc;

use super::{
    super::workspace::{HomeWorkspace, SidebarConversationNode, SidebarProjectHeader},
    actions::{
        ConversationSidebarAction, ConversationSidebarActions, ProjectSidebarAction,
        ProjectSidebarActions, SidebarActionGuardError,
    },
};

type AsyncRenameOnSubmit = dyn Fn(String, &mut Window, &mut App) -> Task<bool>;

struct SidebarRenameDialogConfig {
    title: SharedString,
    initial_value: String,
    placeholder: SharedString,
    cancel_button_id: &'static str,
    submit_button_id: &'static str,
}

struct SidebarRenameDialogState {
    on_submit: Rc<AsyncRenameOnSubmit>,
    task: Option<Task<()>>,
}

impl SidebarRenameDialogState {
    fn submit(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.task.is_some() || value.is_empty() {
            return;
        }

        let action = (self.on_submit)(value, window, cx);
        let state = cx.entity().downgrade();
        self.task = Some(window.spawn(cx, async move |cx| {
            let should_close = action.await;
            let _ = state.update_in(cx, |state, window, cx| {
                state.task = None;
                if should_close {
                    window.close_dialog(cx);
                } else {
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    fn is_pending(&self) -> bool {
        self.task.is_some()
    }
}

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

fn open_sidebar_rename_dialog(
    config: SidebarRenameDialogConfig,
    on_submit: impl Fn(String, &mut Window, &mut App) -> Task<bool> + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let SidebarRenameDialogConfig {
        title,
        initial_value,
        placeholder,
        cancel_button_id,
        submit_button_id,
    } = config;
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(initial_value)
            .placeholder(placeholder)
    });
    let input_to_focus = input.clone();
    let state = cx.new(|_| SidebarRenameDialogState {
        on_submit: Rc::new(on_submit),
        task: None,
    });
    let cancel_label = cx.global::<I18n>().t("button-cancel");
    let save_label = cx.global::<I18n>().t("provider-action-save");

    window.open_dialog(cx, move |dialog, _window, cx| {
        let pending = state.read(cx).is_pending();
        let cancel_state = state.clone();
        let confirm_state = state.clone();
        let confirm_input = input.clone();
        dialog
            .title(title.clone())
            .w(px(420.))
            .close_button(false)
            .on_cancel(move |_, _, cx| !cancel_state.read(cx).is_pending())
            .on_ok(move |_, window, cx| {
                let value = confirm_input.read(cx).value().trim().to_string();
                confirm_state.update(cx, |state, cx| state.submit(value, window, cx));
                false
            })
            .child(
                v_flex()
                    .w_full()
                    .min_w_0()
                    .child(Input::new(&input).w_full().disabled(pending)),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().child(
                            Button::new(cancel_button_id)
                                .label(cancel_label.clone())
                                .disabled(pending),
                        ),
                    )
                    .child(
                        DialogAction::new().child(
                            Button::new(submit_button_id)
                                .primary()
                                .label(save_label.clone())
                                .loading(pending),
                        ),
                    ),
            )
    });

    window.defer(cx, move |window, cx| {
        input_to_focus.update(cx, |input, cx| input.focus(window, cx));
    });
}

pub(super) fn open_rename_project_dialog(
    project: SidebarProjectHeader,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let title = cx.global::<I18n>().t("sidebar-rename-project-title");
    let placeholder = cx.global::<I18n>().t("sidebar-rename-project-placeholder");
    let project_id = project.id;

    open_sidebar_rename_dialog(
        SidebarRenameDialogConfig {
            title: title.into(),
            initial_value: project.display_name.to_string(),
            placeholder: placeholder.into(),
            cancel_button_id: "rename-project-cancel",
            submit_button_id: "rename-project-submit",
        },
        move |display_name, window, cx| {
            let target_id = project_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.rename_project(target_id.clone(), display_name, cx)
            });
            window.spawn(cx, async move |cx| match task.await {
                Ok(_) => true,
                Err(error) => {
                    tracing::error!(
                        action = "project-rename",
                        target_kind = "project",
                        target_id = %target_id,
                        error = ?error,
                        "sidebar action failed"
                    );
                    let _ = cx.update(|window, cx| {
                        show_sidebar_safe_error(window, cx, "sidebar-rename-project-failed");
                    });
                    false
                }
            })
        },
        window,
        cx,
    );
}

pub(super) fn open_rename_conversation_dialog(
    conversation: SidebarConversationNode,
    workspace: Entity<HomeWorkspace>,
    window: &mut Window,
    cx: &mut App,
) {
    let title = cx.global::<I18n>().t("sidebar-rename-conversation-title");
    let placeholder = cx
        .global::<I18n>()
        .t("sidebar-rename-conversation-placeholder");
    let conversation_id = conversation.id;

    open_sidebar_rename_dialog(
        SidebarRenameDialogConfig {
            title: title.into(),
            initial_value: conversation.title.to_string(),
            placeholder: placeholder.into(),
            cancel_button_id: "rename-conversation-cancel",
            submit_button_id: "rename-conversation-submit",
        },
        move |value, window, cx| {
            let target_id = conversation_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.rename_conversation(target_id.clone(), value, cx)
            });
            window.spawn(cx, async move |cx| match task.await {
                Ok(_) => true,
                Err(error) => {
                    tracing::error!(
                        action = "conversation-rename",
                        target_kind = "conversation",
                        target_id = %target_id,
                        error = ?error,
                        "sidebar action failed"
                    );
                    let _ = cx.update(|window, cx| {
                        show_sidebar_safe_error(window, cx, "sidebar-rename-conversation-failed");
                    });
                    false
                }
            })
        },
        window,
        cx,
    );
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

    open_async_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Archive,
        move |window, cx| {
            let conversation_id = conversation_id.clone();
            let project_id = project_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.archive_conversation(conversation_id.clone(), project_id, cx)
            });
            window.spawn(cx, async move |cx| match task.await {
                Ok(_) => true,
                Err(error) => {
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
                    false
                }
            })
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

    open_async_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Archive,
        move |window, cx| {
            let project_id = project_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.archive_project_conversations(project_id.clone(), cx)
            });
            window.spawn(cx, async move |cx| match task.await {
                Ok(_) => true,
                Err(error) => {
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
                    false
                }
            })
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

    open_async_destructive_confirm_dialog(
        title,
        message,
        DestructiveAction::Delete,
        move |window, cx| {
            let project_id = project_id.clone();
            let task = workspace.update(cx, |workspace, cx| {
                workspace.remove_project(project_id.clone(), cx)
            });
            window.spawn(cx, async move |cx| match task.await {
                Ok(_) => true,
                Err(error) => {
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
                    false
                }
            })
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
    use gpui::{
        AppContext as _, IntoElement, Render, SharedString, TestAppContext, Window, WindowHandle,
        div,
    };
    use gpui_component::IconNamed;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    use tokio::sync::oneshot;

    use super::SidebarRenameDialogState;

    struct TestView;

    impl Render for TestView {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl IntoElement {
            div()
        }
    }

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

    #[gpui::test]
    fn rename_dialog_owns_submission_and_blocks_repeated_submit(cx: &mut TestAppContext) {
        let window = open_test_window(cx);
        let invocations = Rc::new(Cell::new(0));
        let (sender, receiver) = oneshot::channel();
        let receiver = Rc::new(RefCell::new(Some(receiver)));
        let state = window
            .update(cx, |_view, _window, cx| {
                let invocations = invocations.clone();
                let receiver = receiver.clone();
                cx.new(|_| SidebarRenameDialogState {
                    on_submit: Rc::new(move |_value, window, cx| {
                        invocations.set(invocations.get() + 1);
                        let receiver = receiver
                            .borrow_mut()
                            .take()
                            .expect("rename submission starts once");
                        window.spawn(cx, async move |_| receiver.await.unwrap_or(false))
                    }),
                    task: None,
                })
            })
            .expect("create rename dialog state");

        window
            .update(cx, |_view, window, cx| {
                state.update(cx, |state, cx| state.submit("first".into(), window, cx));
                state.update(cx, |state, cx| state.submit("second".into(), window, cx));
                assert!(state.read(cx).is_pending());
            })
            .expect("start rename submission");
        assert_eq!(invocations.get(), 1);

        sender.send(false).expect("finish rename submission");
        cx.run_until_parked();
        window
            .update(cx, |_view, _window, cx| {
                assert!(!state.read(cx).is_pending());
            })
            .expect("inspect completed rename submission");
    }

    fn open_test_window(cx: &mut TestAppContext) -> WindowHandle<TestView> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |_window, cx| cx.new(|_| TestView))
                .expect("open rename dialog test window")
        })
    }
}
