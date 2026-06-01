use ai_chat_core::{ProviderSecretRef, ProviderSecretRefs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderSecretWrite {
    pub(super) key: String,
    pub(super) value: String,
}

#[derive(Debug, Default, Clone)]
pub(super) struct ProviderSecretStore;

impl ProviderSecretStore {
    pub(super) fn refs_for(
        provider_id: &str,
        writes: &[ProviderSecretWrite],
    ) -> ProviderSecretRefs {
        ProviderSecretRefs {
            refs: writes
                .iter()
                .filter(|write| !write.value.is_empty())
                .map(|write| ProviderSecretRef {
                    key: write.key.clone(),
                    storage: "keychain".to_string(),
                    ref_id: format!("{provider_id}:{}", write.key),
                })
                .collect(),
        }
    }
}
