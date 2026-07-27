use std::collections::BTreeMap;

use gpui::AsyncWindowContext;
use jaco_agent::ProviderSecretValues;
use jaco_core::{ProviderSecretRef, ProviderSecretRefs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProviderSecretWrite {
    pub(crate) key: String,
    pub(crate) value: String,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ProviderSecretStore;

impl ProviderSecretStore {
    pub(crate) fn refs_for(
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

    pub(crate) async fn write_values(
        cx: &mut AsyncWindowContext,
        refs: &ProviderSecretRefs,
        writes: &[ProviderSecretWrite],
    ) -> Result<(), String> {
        for write in writes {
            let Some(secret_ref) = refs.refs.iter().find(|secret| secret.key == write.key) else {
                continue;
            };
            let ref_id = secret_ref.ref_id.clone();
            let key = write.key.clone();
            let value = write.value.clone();
            let task = cx
                .update(move |_, cx| cx.write_credentials(&ref_id, &key, value.as_bytes()))
                .map_err(|err| err.to_string())?;
            task.await.map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub(crate) async fn read_values(
        cx: &mut AsyncWindowContext,
        refs: &ProviderSecretRefs,
    ) -> Result<ProviderSecretValues, String> {
        let mut values = BTreeMap::new();
        for secret_ref in &refs.refs {
            let ref_id = secret_ref.ref_id.clone();
            let key = secret_ref.key.clone();
            let task = cx
                .update(move |_, cx| cx.read_credentials(&ref_id))
                .map_err(|err| err.to_string())?;
            let Some((_, password)) = task.await.map_err(|err| err.to_string())? else {
                continue;
            };
            let value = String::from_utf8(password).map_err(|err| err.to_string())?;
            values.insert(key, value);
        }
        Ok(ProviderSecretValues { values })
    }
}
