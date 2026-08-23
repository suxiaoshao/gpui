use std::path::Path;

use gpui::{App, ClipboardItem, Entity, Task, Window};
use jaco_core::{ConversationId, ProjectId};

use super::{
    super::workspace::{HomeWorkspace, SidebarConversationNode, SidebarProjectHeader},
    menu,
};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectSidebarAction {
    NewConversation = 0,
    TogglePinned = 1,
    Rename = 2,
    RevealInFileManager = 3,
    ArchiveConversations = 4,
    Remove = 5,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConversationSidebarAction {
    TogglePinned = 0,
    Rename = 1,
    Archive = 2,
    CopyWorkingDirectory = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProjectActionAvailability {
    pub(super) project_mutations: bool,
    pub(super) new_conversation: bool,
    pub(super) archive_conversations: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ConversationActionAvailability {
    pub(super) conversation_mutations: bool,
    pub(super) copy_working_directory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(super) enum SidebarActionGuardError {
    #[error("sidebar resource is not ready: {resource:?}")]
    ResourceNotReady { resource: SidebarResource },
    #[error("sidebar target disappeared: {target:?}")]
    TargetDisappeared { target: SidebarTarget },
    #[error("clipboard verification failed")]
    ClipboardVerificationFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SidebarResource {
    Projects,
    Conversations,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SidebarTarget {
    Project(ProjectId),
    Conversation(ConversationId),
}

#[derive(Clone)]
pub(super) struct ProjectSidebarActions {
    project: SidebarProjectHeader,
    workspace: Entity<HomeWorkspace>,
    availability: ProjectActionAvailability,
}

impl ProjectSidebarActions {
    pub(super) fn new(
        project: SidebarProjectHeader,
        workspace: Entity<HomeWorkspace>,
        cx: &App,
    ) -> Self {
        let workspace_state = workspace.read(cx);
        let project_mutations = workspace_state.project_mutations_ready()
            && workspace_state.contains_project(&project.id);
        let conversations_ready = workspace_state.conversation_mutations_ready(cx);
        let active_count = workspace_state.active_conversation_count(&project.id, cx);

        Self {
            project,
            workspace,
            availability: ProjectActionAvailability {
                project_mutations,
                new_conversation: project_mutations && conversations_ready,
                archive_conversations: project_mutations && conversations_ready && active_count > 0,
            },
        }
    }

    pub(super) fn availability(&self, action: ProjectSidebarAction) -> bool {
        project_action_available(self.availability, action)
    }

    pub(super) fn project(&self) -> &SidebarProjectHeader {
        &self.project
    }

    pub(super) fn invoke(&self, action: ProjectSidebarAction, window: &mut Window, cx: &mut App) {
        if !self.availability(action) {
            return;
        }
        if let Err(error) = self.guard(action, cx) {
            menu::show_sidebar_guard_error(error, window, cx);
            return;
        }

        match action {
            ProjectSidebarAction::NewConversation => {
                let project_id = self.project.id.clone();
                self.workspace.update(cx, |workspace, cx| {
                    workspace.new_conversation_in_project(&project_id, cx);
                });
            }
            ProjectSidebarAction::TogglePinned => {
                let project_id = self.project.id.clone();
                let pinned = !self.project.pinned;
                let task = self.workspace.update(cx, |workspace, cx| {
                    workspace.pin_project(project_id.clone(), pinned, cx)
                });
                retain_project_task(
                    task,
                    project_id,
                    "project-toggle-pinned",
                    "sidebar-project-pin-failed",
                    window,
                    cx,
                );
            }
            ProjectSidebarAction::Rename => {
                menu::open_rename_project_dialog(
                    self.project.clone(),
                    self.workspace.clone(),
                    window,
                    cx,
                );
            }
            ProjectSidebarAction::RevealInFileManager => {
                cx.open_with_system(&self.project.path);
            }
            ProjectSidebarAction::ArchiveConversations => {
                menu::open_archive_project_conversations_confirm(
                    self.project.clone(),
                    self.workspace.clone(),
                    window,
                    cx,
                );
            }
            ProjectSidebarAction::Remove => {
                menu::open_remove_project_confirm(
                    self.project.clone(),
                    self.workspace.clone(),
                    window,
                    cx,
                );
            }
        }
    }

    fn guard(&self, action: ProjectSidebarAction, cx: &App) -> Result<(), SidebarActionGuardError> {
        if action == ProjectSidebarAction::RevealInFileManager {
            return Ok(());
        }
        let workspace = self.workspace.read(cx);
        if !workspace.project_mutations_ready() {
            return Err(SidebarActionGuardError::ResourceNotReady {
                resource: SidebarResource::Projects,
            });
        }
        if !workspace.contains_project(&self.project.id) {
            return Err(SidebarActionGuardError::TargetDisappeared {
                target: SidebarTarget::Project(self.project.id.clone()),
            });
        }
        if matches!(
            action,
            ProjectSidebarAction::NewConversation | ProjectSidebarAction::ArchiveConversations
        ) && !workspace.conversation_mutations_ready(cx)
        {
            return Err(SidebarActionGuardError::ResourceNotReady {
                resource: SidebarResource::Conversations,
            });
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct ConversationSidebarActions {
    conversation: SidebarConversationNode,
    workspace: Entity<HomeWorkspace>,
    availability: ConversationActionAvailability,
}

impl ConversationSidebarActions {
    pub(super) fn new(
        conversation: SidebarConversationNode,
        workspace: Entity<HomeWorkspace>,
        cx: &App,
    ) -> Self {
        let workspace_state = workspace.read(cx);
        let conversation_exists = workspace_state.contains_conversation(&conversation.id, cx);
        let conversation_mutations =
            workspace_state.conversation_mutations_ready(cx) && conversation_exists;
        let projects_ready = workspace_state.project_mutations_ready();
        let working_directory = workspace_state.project_path(&conversation.project_id);

        Self {
            conversation,
            workspace,
            availability: ConversationActionAvailability {
                conversation_mutations,
                copy_working_directory: conversation_exists
                    && projects_ready
                    && working_directory.is_some(),
            },
        }
    }

    pub(super) fn availability(&self, action: ConversationSidebarAction) -> bool {
        conversation_action_available(self.availability, action)
    }

    pub(super) fn conversation(&self) -> &SidebarConversationNode {
        &self.conversation
    }

    pub(super) fn invoke(
        &self,
        action: ConversationSidebarAction,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !self.availability(action) {
            return;
        }
        if let Err(error) = self.guard(action, cx) {
            menu::show_sidebar_guard_error(error, window, cx);
            return;
        }

        match action {
            ConversationSidebarAction::TogglePinned => {
                let conversation_id = self.conversation.id.clone();
                let pinned = !self.conversation.pinned;
                let task = self.workspace.update(cx, |workspace, cx| {
                    workspace.pin_conversation(conversation_id.clone(), pinned, cx)
                });
                retain_conversation_task(
                    task,
                    conversation_id,
                    "conversation-toggle-pinned",
                    "sidebar-conversation-pin-failed",
                    window,
                    cx,
                );
            }
            ConversationSidebarAction::Rename => {
                menu::open_rename_conversation_dialog(
                    self.conversation.clone(),
                    self.workspace.clone(),
                    window,
                    cx,
                );
            }
            ConversationSidebarAction::Archive => {
                menu::open_archive_conversation_confirm(
                    self.conversation.clone(),
                    self.workspace.clone(),
                    window,
                    cx,
                );
            }
            ConversationSidebarAction::CopyWorkingDirectory => {
                let Some(path) = self
                    .workspace
                    .read(cx)
                    .project_path(&self.conversation.project_id)
                else {
                    menu::show_sidebar_guard_error(
                        SidebarActionGuardError::TargetDisappeared {
                            target: SidebarTarget::Project(self.conversation.project_id.clone()),
                        },
                        window,
                        cx,
                    );
                    return;
                };

                if let Err(error) = copy_working_directory(&path, cx) {
                    tracing::error!(
                        action = "conversation-copy-working-directory",
                        target_kind = "conversation",
                        target_id = %self.conversation.id,
                        error = ?error,
                        "sidebar action failed"
                    );
                    menu::show_sidebar_guard_error(error, window, cx);
                } else {
                    menu::show_sidebar_copy_success(window, cx);
                }
            }
        }
    }

    fn guard(
        &self,
        action: ConversationSidebarAction,
        cx: &App,
    ) -> Result<(), SidebarActionGuardError> {
        let workspace = self.workspace.read(cx);
        if action != ConversationSidebarAction::CopyWorkingDirectory
            && !workspace.conversation_mutations_ready(cx)
        {
            return Err(SidebarActionGuardError::ResourceNotReady {
                resource: SidebarResource::Conversations,
            });
        }
        if !workspace.contains_conversation(&self.conversation.id, cx) {
            return Err(SidebarActionGuardError::TargetDisappeared {
                target: SidebarTarget::Conversation(self.conversation.id.clone()),
            });
        }
        if action == ConversationSidebarAction::CopyWorkingDirectory {
            if !workspace.project_mutations_ready() {
                return Err(SidebarActionGuardError::ResourceNotReady {
                    resource: SidebarResource::Projects,
                });
            }
            if workspace
                .project_path(&self.conversation.project_id)
                .is_none()
            {
                return Err(SidebarActionGuardError::TargetDisappeared {
                    target: SidebarTarget::Project(self.conversation.project_id.clone()),
                });
            }
        }
        Ok(())
    }
}

fn project_action_available(
    availability: ProjectActionAvailability,
    action: ProjectSidebarAction,
) -> bool {
    match action {
        ProjectSidebarAction::NewConversation => availability.new_conversation,
        ProjectSidebarAction::TogglePinned
        | ProjectSidebarAction::Rename
        | ProjectSidebarAction::Remove => availability.project_mutations,
        ProjectSidebarAction::RevealInFileManager => true,
        ProjectSidebarAction::ArchiveConversations => availability.archive_conversations,
    }
}

fn conversation_action_available(
    availability: ConversationActionAvailability,
    action: ConversationSidebarAction,
) -> bool {
    match action {
        ConversationSidebarAction::TogglePinned
        | ConversationSidebarAction::Rename
        | ConversationSidebarAction::Archive => availability.conversation_mutations,
        ConversationSidebarAction::CopyWorkingDirectory => availability.copy_working_directory,
    }
}

fn copy_working_directory(path: &Path, cx: &mut App) -> Result<(), SidebarActionGuardError> {
    let text = path.display().to_string();
    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
    let copied = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .is_some_and(|copied| copied == text);
    copied
        .then_some(())
        .ok_or(SidebarActionGuardError::ClipboardVerificationFailed)
}

fn retain_project_task<T: Send + 'static>(
    task: Task<jaco_db::Result<T>>,
    project_id: ProjectId,
    action: &'static str,
    error_key: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    let completion = window.spawn(cx, async move |cx| {
        if let Err(error) = task.await {
            tracing::error!(
                action,
                target_kind = "project",
                target_id = %project_id,
                error = ?error,
                "sidebar action failed"
            );
            let _ = cx.update(|window, cx| {
                menu::show_sidebar_safe_error(window, cx, error_key);
            });
        }
    });
    crate::app::tasks::retain_window(window, completion, cx);
}

fn retain_conversation_task<T: Send + 'static>(
    task: Task<jaco_db::Result<T>>,
    conversation_id: ConversationId,
    action: &'static str,
    error_key: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    let completion = window.spawn(cx, async move |cx| {
        if let Err(error) = task.await {
            tracing::error!(
                action,
                target_kind = "conversation",
                target_id = %conversation_id,
                error = ?error,
                "sidebar action failed"
            );
            let _ = cx.update(|window, cx| {
                menu::show_sidebar_safe_error(window, cx, error_key);
            });
        }
    });
    crate::app::tasks::retain_window(window, completion, cx);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gpui::TestAppContext;

    use super::{
        ConversationActionAvailability, ConversationSidebarAction, ProjectActionAvailability,
        ProjectSidebarAction, conversation_action_available, project_action_available,
    };

    #[test]
    fn project_actions_keep_product_order() {
        assert_eq!(
            [
                ProjectSidebarAction::NewConversation as u8,
                ProjectSidebarAction::TogglePinned as u8,
                ProjectSidebarAction::Rename as u8,
                ProjectSidebarAction::RevealInFileManager as u8,
                ProjectSidebarAction::ArchiveConversations as u8,
                ProjectSidebarAction::Remove as u8,
            ],
            [0, 1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn conversation_actions_keep_product_order() {
        assert_eq!(
            [
                ConversationSidebarAction::TogglePinned as u8,
                ConversationSidebarAction::Rename as u8,
                ConversationSidebarAction::Archive as u8,
                ConversationSidebarAction::CopyWorkingDirectory as u8,
            ],
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn project_action_availability_keeps_reveal_independent() {
        let availability = ProjectActionAvailability {
            project_mutations: false,
            new_conversation: false,
            archive_conversations: false,
        };

        assert!(project_action_available(
            availability,
            ProjectSidebarAction::RevealInFileManager
        ));
        assert!(!project_action_available(
            availability,
            ProjectSidebarAction::NewConversation
        ));
        assert!(!project_action_available(
            availability,
            ProjectSidebarAction::ArchiveConversations
        ));
    }

    #[test]
    fn conversation_copy_availability_is_independent_of_mutations() {
        let availability = ConversationActionAvailability {
            conversation_mutations: false,
            copy_working_directory: true,
        };

        assert!(conversation_action_available(
            availability,
            ConversationSidebarAction::CopyWorkingDirectory
        ));
        assert!(!conversation_action_available(
            availability,
            ConversationSidebarAction::TogglePinned
        ));
    }

    #[gpui::test]
    fn copy_working_directory_writes_and_verifies_exact_project_path(cx: &mut TestAppContext) {
        let path = Path::new("/tmp/jaco-issue-188-project");
        let result = cx.update(|cx| super::copy_working_directory(path, cx));
        assert_eq!(result, Ok(()));
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some(path.to_str().unwrap())
        );
    }
}
