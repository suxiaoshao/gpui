use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRecord {
    pub id: ProviderId,
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    pub settings: ProviderSettingsPayload,
    pub secret_refs: ProviderSecretRefs,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProvider {
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    pub settings: ProviderSettingsPayload,
    pub secret_refs: ProviderSecretRefs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateProvider {
    pub display_name: String,
    pub enabled: bool,
    pub settings: ProviderSettingsPayload,
    pub secret_refs: ProviderSecretRefs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderModelRecord {
    pub id: ProviderModelId,
    pub provider_id: ProviderId,
    pub model_id: String,
    pub display_name: Option<String>,
    pub enabled: bool,
    pub capabilities: ModelCapabilitiesSnapshot,
    pub metadata: ProviderModelMetadata,
    pub fetched_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProviderModel {
    pub provider_id: ProviderId,
    pub model_id: String,
    pub display_name: Option<String>,
    pub enabled: bool,
    pub capabilities: ModelCapabilitiesSnapshot,
    pub metadata: ProviderModelMetadata,
}
