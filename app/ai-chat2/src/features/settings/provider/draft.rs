#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use ai_chat_core::{
    ModelCapabilitiesSnapshot, ProviderId, ProviderModelId, ProviderModelMetadata,
    ProviderSecretRefs,
};
use gpui::{Entity, SharedString, Subscription};
use gpui_component::input::InputState;

use super::{capabilities::CapabilityDraft, catalog::ProviderKindKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProviderSelection {
    Builtin {
        kind: ProviderKindKey,
        provider_id: Option<ProviderId>,
    },
    Custom {
        provider_id: ProviderId,
    },
    NewCustom,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum ProviderDraftValue {
    String(String),
    Bool(bool),
    Number(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProviderDraft {
    pub(super) provider_id: Option<ProviderId>,
    pub(super) kind: ProviderKindKey,
    pub(super) display_name: String,
    pub(super) enabled: bool,
    pub(super) fields: BTreeMap<String, ProviderDraftValue>,
    pub(super) existing_secret_refs: ProviderSecretRefs,
    pub(super) dirty: bool,
}

impl ProviderDraft {
    pub(super) fn field_string(&self, key: &str) -> String {
        match self.fields.get(key) {
            Some(ProviderDraftValue::String(value)) => value.clone(),
            Some(ProviderDraftValue::Bool(value)) => value.to_string(),
            Some(ProviderDraftValue::Number(value)) => value.to_string(),
            None => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProviderDraftSnapshot {
    pub(super) provider_id: Option<ProviderId>,
    pub(super) kind: ProviderKindKey,
    pub(super) display_name: String,
    pub(super) enabled: bool,
    pub(super) fields: BTreeMap<String, ProviderDraftValue>,
    pub(super) secret_refs: ProviderSecretRefs,
    pub(super) dirty_secret_keys: BTreeSet<String>,
}

impl ProviderDraftSnapshot {
    pub(super) fn from_draft(draft: &ProviderDraft) -> Self {
        Self {
            provider_id: draft.provider_id.clone(),
            kind: draft.kind.clone(),
            display_name: draft.display_name.clone(),
            enabled: draft.enabled,
            fields: draft.fields.clone(),
            secret_refs: draft.existing_secret_refs.clone(),
            dirty_secret_keys: BTreeSet::new(),
        }
    }

    pub(super) fn is_dirty_against(&self, saved: Option<&Self>) -> bool {
        saved != Some(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ProviderValidationState {
    Idle,
    Valid,
    Invalid(SharedString),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ProviderModelDraft {
    pub(super) row_id: Option<ProviderModelId>,
    pub(super) provider_id: ProviderId,
    pub(super) model_id: String,
    pub(super) display_name: Option<String>,
    pub(super) enabled: bool,
    pub(super) capabilities: ModelCapabilitiesSnapshot,
    pub(super) metadata: ProviderModelMetadata,
    pub(super) fetched_at: Option<String>,
    pub(super) dirty: bool,
}

pub(super) struct ManualModelEditor {
    pub(super) mode: ManualModelEditorMode,
    pub(super) model_id_input: Entity<InputState>,
    pub(super) display_name_input: Entity<InputState>,
    pub(super) context_window_input: Entity<InputState>,
    pub(super) capabilities: CapabilityDraft,
    pub(super) error: Option<SharedString>,
    pub(super) _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ManualModelEditorMode {
    Add,
    Edit,
}
