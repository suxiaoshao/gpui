use gpui_store::Select;
use jaco_core::{
    AppLanguage, AppThemeSettings, ConversationId, ConversationStatus, ProjectId, ProjectKind,
    ShortcutId,
};
use jaco_db::{ProjectRecord, PromptRecord, ProviderModelRecord, ProviderRecord, ShortcutRecord};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{
    config::{ConfigOperation, McpServerTomlConfig},
    conversations::index::ConversationIndexOperation,
    projects::ProjectOperation,
    prompts::PromptOperation,
    providers::{ProviderModelChoice, ProviderOperation},
    shortcuts::ShortcutOperation,
};
use crate::database::DatabaseTarget;

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectLanguage;

impl Select<ConfigOperation> for SelectLanguage {
    type Output = AppLanguage;

    fn select(&self, operation: &ConfigOperation) -> Self::Output {
        operation
            .data()
            .map(|config| config.app_settings_payload().language)
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectThemeSettings;

impl Select<ConfigOperation> for SelectThemeSettings {
    type Output = AppThemeSettings;

    fn select(&self, operation: &ConfigOperation) -> Self::Output {
        operation
            .data()
            .map(|config| config.app_settings_payload().theme)
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectDatabaseTarget;

impl Select<ConfigOperation> for SelectDatabaseTarget {
    type Output = Option<DatabaseTarget>;

    fn select(&self, operation: &ConfigOperation) -> Self::Output {
        operation.data().map(DatabaseTarget::from_config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfigGateStatus {
    phase: gpui_operation::repair::Phase,
    problem: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectConfigGateStatus;

impl Select<ConfigOperation> for SelectConfigGateStatus {
    type Output = ConfigGateStatus;

    fn select(&self, operation: &ConfigOperation) -> Self::Output {
        ConfigGateStatus {
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectMcpConfig;

impl Select<ConfigOperation> for SelectMcpConfig {
    type Output = Option<BTreeMap<String, McpServerTomlConfig>>;

    fn select(&self, operation: &ConfigOperation) -> Self::Output {
        operation.data().map(|config| config.mcp_servers.clone())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectTemporaryHotkey;

impl Select<ConfigOperation> for SelectTemporaryHotkey {
    type Output = Option<String>;

    fn select(&self, operation: &ConfigOperation) -> Self::Output {
        operation
            .data()
            .and_then(|config| config.app_settings.temporary_hotkey.clone())
    }
}

pub(crate) type ProviderWithModels = (ProviderRecord, Vec<ProviderModelRecord>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RefreshResourceStatus {
    phase: gpui_operation::refresh::Phase,
    problem: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProviderRecordsWithModels;

impl Select<ProviderOperation> for SelectProviderRecordsWithModels {
    type Output = Option<Vec<ProviderWithModels>>;

    fn select(&self, operation: &ProviderOperation) -> Self::Output {
        operation.data().map(|data| data.providers.clone())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProviderStatus;

impl Select<ProviderOperation> for SelectProviderStatus {
    type Output = RefreshResourceStatus;

    fn select(&self, operation: &ProviderOperation) -> Self::Output {
        RefreshResourceStatus {
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ProviderModelCatalogSnapshot {
    models: Option<Vec<ProviderModelChoice>>,
    phase: gpui_operation::refresh::Phase,
    problem: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProviderModelCatalog;

impl Select<ProviderOperation> for SelectProviderModelCatalog {
    type Output = ProviderModelCatalogSnapshot;

    fn select(&self, operation: &ProviderOperation) -> Self::Output {
        ProviderModelCatalogSnapshot {
            models: operation.data().map(|data| data.enabled_models.clone()),
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectNormalProjects;

impl Select<ProjectOperation> for SelectNormalProjects {
    type Output = Option<Vec<ProjectRecord>>;

    fn select(&self, operation: &ProjectOperation) -> Self::Output {
        operation.data().map(|data| {
            data.projects()
                .iter()
                .filter(|project| project.kind == ProjectKind::Normal && !project.removed)
                .cloned()
                .collect()
        })
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectProjectStatus;

impl Select<ProjectOperation> for SelectProjectStatus {
    type Output = RefreshResourceStatus;

    fn select(&self, operation: &ProjectOperation) -> Self::Output {
        RefreshResourceStatus {
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NormalProjectCatalogSnapshot {
    projects: Option<Vec<ProjectRecord>>,
    phase: gpui_operation::refresh::Phase,
    problem: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectNormalProjectCatalog;

impl Select<ProjectOperation> for SelectNormalProjectCatalog {
    type Output = NormalProjectCatalogSnapshot;

    fn select(&self, operation: &ProjectOperation) -> Self::Output {
        NormalProjectCatalogSnapshot {
            projects: operation.data().map(|data| {
                data.projects()
                    .iter()
                    .filter(|project| project.kind == ProjectKind::Normal && !project.removed)
                    .cloned()
                    .collect()
            }),
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectPromptRecords;

impl Select<PromptOperation> for SelectPromptRecords {
    type Output = Option<Vec<PromptRecord>>;

    fn select(&self, operation: &PromptOperation) -> Self::Output {
        operation.data().map(|data| data.prompts().to_vec())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectPromptStatus;

impl Select<PromptOperation> for SelectPromptStatus {
    type Output = RefreshResourceStatus;

    fn select(&self, operation: &PromptOperation) -> Self::Output {
        RefreshResourceStatus {
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectShortcutRecords;

impl Select<ShortcutOperation> for SelectShortcutRecords {
    type Output = Option<Vec<ShortcutRecord>>;

    fn select(&self, operation: &ShortcutOperation) -> Self::Output {
        operation.data().map(|data| data.shortcuts().to_vec())
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectShortcutStatus;

impl Select<ShortcutOperation> for SelectShortcutStatus {
    type Output = RefreshResourceStatus;

    fn select(&self, operation: &ShortcutOperation) -> Self::Output {
        RefreshResourceStatus {
            phase: operation.phase(),
            problem: operation.problem().map(ToString::to_string),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutRegistration {
    pub(crate) id: ShortcutId,
    pub(crate) hotkey: String,
    pub(crate) enabled: bool,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectShortcutRegistrations;

impl Select<ShortcutOperation> for SelectShortcutRegistrations {
    type Output = Option<Vec<ShortcutRegistration>>;

    fn select(&self, operation: &ShortcutOperation) -> Self::Output {
        operation.data().map(|data| {
            data.shortcuts()
                .iter()
                .map(|shortcut| ShortcutRegistration {
                    id: shortcut.id.clone(),
                    hotkey: shortcut.hotkey.clone(),
                    enabled: shortcut.enabled,
                })
                .collect()
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceProjectInput {
    pub(crate) id: ProjectId,
    pub(crate) kind: ProjectKind,
    pub(crate) path: PathBuf,
    pub(crate) display_name: String,
    pub(crate) pinned: bool,
    pub(crate) updated_at: i128,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectWorkspaceProjects;

impl Select<ProjectOperation> for SelectWorkspaceProjects {
    type Output = Option<Vec<WorkspaceProjectInput>>;

    fn select(&self, operation: &ProjectOperation) -> Self::Output {
        operation.data().map(|data| {
            data.projects()
                .iter()
                .filter(|project| !project.removed)
                .map(|project| WorkspaceProjectInput {
                    id: project.id.clone(),
                    kind: project.kind,
                    path: PathBuf::from(&project.path),
                    display_name: project.display_name.clone(),
                    pinned: project.pinned,
                    updated_at: project.updated_at.unix_timestamp_nanos(),
                })
                .collect()
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceConversationInput {
    pub(crate) id: ConversationId,
    pub(crate) project_id: ProjectId,
    pub(crate) title: String,
    pub(crate) pinned: bool,
    pub(crate) status: ConversationStatus,
    pub(crate) updated_at: i128,
    pub(crate) deleted_at: Option<i128>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SelectWorkspaceConversations;

impl Select<ConversationIndexOperation> for SelectWorkspaceConversations {
    type Output = Option<Vec<WorkspaceConversationInput>>;

    fn select(&self, operation: &ConversationIndexOperation) -> Self::Output {
        operation.data().map(|data| {
            data.conversations()
                .iter()
                .map(|conversation| WorkspaceConversationInput {
                    id: conversation.id.clone(),
                    project_id: conversation.project_id.clone(),
                    title: conversation.title.clone(),
                    pinned: conversation.pinned,
                    status: conversation.status,
                    updated_at: conversation.updated_at.unix_timestamp_nanos(),
                    deleted_at: conversation
                        .deleted_at
                        .map(|deleted_at| deleted_at.unix_timestamp_nanos()),
                })
                .collect()
        })
    }
}
