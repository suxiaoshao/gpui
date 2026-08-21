use std::fmt;

use time::OffsetDateTime;

use super::{ProviderSettingValue, ProviderSettingsPayload, ProviderUsageSnapshot};

const TOKENS_PER_MILLION: u128 = 1_000_000;
const USD_NANOS: u32 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PricingValidationError {
    InvalidDecimal,
    DecimalPrecision,
    ValueOutOfRange,
    InvalidAmount,
    InvalidRoute,
    InvalidIdentifier,
    InvalidTier,
}

impl fmt::Display for PricingValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidDecimal => "invalid decimal price",
            Self::DecimalPrecision => "decimal price exceeds nano-USD precision",
            Self::ValueOutOfRange => "pricing value is out of range",
            Self::InvalidAmount => "amount exceeds the supported SQLite INTEGER range",
            Self::InvalidRoute => "invalid official provider pricing route",
            Self::InvalidIdentifier => "pricing identifier must be non-empty and trimmed",
            Self::InvalidTier => "pricing tiers must have unique, increasing positive thresholds",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PricingValidationError {}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct UsdNanoPerMillionTokens(u64);

impl UsdNanoPerMillionTokens {
    pub const fn new(nano_usd_per_million_tokens: u64) -> Self {
        Self(nano_usd_per_million_tokens)
    }

    pub fn from_usd_per_million_decimal(decimal: &str) -> Result<Self, PricingValidationError> {
        parse_usd_decimal_to_nanos(decimal).map(Self)
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "u64", into = "u64")]
pub struct UsdNanoAmount(u64);

impl UsdNanoAmount {
    pub fn new(nano_usd: u64) -> Result<Self, PricingValidationError> {
        if nano_usd > i64::MAX as u64 {
            return Err(PricingValidationError::InvalidAmount);
        }
        Ok(Self(nano_usd))
    }

    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for UsdNanoAmount {
    type Error = PricingValidationError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<UsdNanoAmount> for u64 {
    fn from(value: UsdNanoAmount) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderTokenPriceSnapshot {
    input: UsdNanoPerMillionTokens,
    output: UsdNanoPerMillionTokens,
    cache_read: Option<UsdNanoPerMillionTokens>,
    cache_write: Option<UsdNanoPerMillionTokens>,
}

impl ProviderTokenPriceSnapshot {
    pub const fn new(
        input: UsdNanoPerMillionTokens,
        output: UsdNanoPerMillionTokens,
        cache_read: Option<UsdNanoPerMillionTokens>,
        cache_write: Option<UsdNanoPerMillionTokens>,
    ) -> Self {
        Self {
            input,
            output,
            cache_read,
            cache_write,
        }
    }

    pub const fn input(&self) -> UsdNanoPerMillionTokens {
        self.input
    }

    pub const fn output(&self) -> UsdNanoPerMillionTokens {
        self.output
    }

    pub const fn cache_read(&self) -> Option<UsdNanoPerMillionTokens> {
        self.cache_read
    }

    pub const fn cache_write(&self) -> Option<UsdNanoPerMillionTokens> {
        self.cache_write
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTokenPriceTierSnapshot {
    input_token_threshold: u64,
    rates: ProviderTokenPriceSnapshot,
}

impl ProviderTokenPriceTierSnapshot {
    pub fn new(
        input_token_threshold: u64,
        rates: ProviderTokenPriceSnapshot,
    ) -> Result<Self, PricingValidationError> {
        if input_token_threshold == 0 {
            return Err(PricingValidationError::InvalidTier);
        }
        Ok(Self {
            input_token_threshold,
            rates,
        })
    }

    pub const fn input_token_threshold(&self) -> u64 {
        self.input_token_threshold
    }

    pub const fn rates(&self) -> &ProviderTokenPriceSnapshot {
        &self.rates
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderTokenPriceTierSnapshotRepr {
    input_token_threshold: u64,
    rates: ProviderTokenPriceSnapshot,
}

impl<'de> serde::Deserialize<'de> for ProviderTokenPriceTierSnapshot {
    fn deserialize<Deserializer>(deserializer: Deserializer) -> Result<Self, Deserializer::Error>
    where
        Deserializer: serde::Deserializer<'de>,
    {
        let repr = ProviderTokenPriceTierSnapshotRepr::deserialize(deserializer)?;
        Self::new(repr.input_token_threshold, repr.rates).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPricingRouteKey {
    provider_kind: String,
    canonical_base_url: String,
}

impl ProviderPricingRouteKey {
    pub fn new(
        provider_kind: impl Into<String>,
        canonical_base_url: impl Into<String>,
    ) -> Result<Self, PricingValidationError> {
        let provider_kind = provider_kind.into();
        let canonical_base_url = canonical_base_url.into();
        let expected_base_url =
            official_base_url(&provider_kind).ok_or(PricingValidationError::InvalidRoute)?;
        if canonical_base_url != expected_base_url {
            return Err(PricingValidationError::InvalidRoute);
        }
        Ok(Self {
            provider_kind,
            canonical_base_url,
        })
    }

    pub fn provider_kind(&self) -> &str {
        &self.provider_kind
    }

    pub fn canonical_base_url(&self) -> &str {
        &self.canonical_base_url
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderPricingRouteKeyRepr {
    provider_kind: String,
    canonical_base_url: String,
}

impl<'de> serde::Deserialize<'de> for ProviderPricingRouteKey {
    fn deserialize<Deserializer>(deserializer: Deserializer) -> Result<Self, Deserializer::Error>
    where
        Deserializer: serde::Deserializer<'de>,
    {
        let repr = ProviderPricingRouteKeyRepr::deserialize(deserializer)?;
        Self::new(repr.provider_kind, repr.canonical_base_url).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelPricingSnapshot {
    models_dev_provider_id: String,
    models_dev_model_id: String,
    route: ProviderPricingRouteKey,
    fetched_at: OffsetDateTime,
    base: ProviderTokenPriceSnapshot,
    tiers: Vec<ProviderTokenPriceTierSnapshot>,
}

impl ProviderModelPricingSnapshot {
    pub fn new(
        models_dev_provider_id: impl Into<String>,
        models_dev_model_id: impl Into<String>,
        route: ProviderPricingRouteKey,
        fetched_at: OffsetDateTime,
        base: ProviderTokenPriceSnapshot,
        tiers: Vec<ProviderTokenPriceTierSnapshot>,
    ) -> Result<Self, PricingValidationError> {
        let models_dev_provider_id = models_dev_provider_id.into();
        let models_dev_model_id = models_dev_model_id.into();
        if !valid_identifier(&models_dev_provider_id) || !valid_identifier(&models_dev_model_id) {
            return Err(PricingValidationError::InvalidIdentifier);
        }
        if tiers
            .windows(2)
            .any(|tiers| tiers[0].input_token_threshold >= tiers[1].input_token_threshold)
        {
            return Err(PricingValidationError::InvalidTier);
        }
        Ok(Self {
            models_dev_provider_id,
            models_dev_model_id,
            route,
            fetched_at,
            base,
            tiers,
        })
    }

    pub fn models_dev_provider_id(&self) -> &str {
        &self.models_dev_provider_id
    }

    pub fn models_dev_model_id(&self) -> &str {
        &self.models_dev_model_id
    }

    pub const fn route(&self) -> &ProviderPricingRouteKey {
        &self.route
    }

    pub const fn fetched_at(&self) -> OffsetDateTime {
        self.fetched_at
    }

    pub const fn base(&self) -> &ProviderTokenPriceSnapshot {
        &self.base
    }

    pub fn tiers(&self) -> &[ProviderTokenPriceTierSnapshot] {
        &self.tiers
    }

    fn rates_for_input_tokens(&self, input_tokens: u64) -> &ProviderTokenPriceSnapshot {
        self.tiers
            .iter()
            .rev()
            .find(|tier| input_tokens >= tier.input_token_threshold)
            .map(|tier| &tier.rates)
            .unwrap_or(&self.base)
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderModelPricingSnapshotRepr {
    models_dev_provider_id: String,
    models_dev_model_id: String,
    route: ProviderPricingRouteKey,
    fetched_at: OffsetDateTime,
    base: ProviderTokenPriceSnapshot,
    tiers: Vec<ProviderTokenPriceTierSnapshot>,
}

impl<'de> serde::Deserialize<'de> for ProviderModelPricingSnapshot {
    fn deserialize<Deserializer>(deserializer: Deserializer) -> Result<Self, Deserializer::Error>
    where
        Deserializer: serde::Deserializer<'de>,
    {
        let repr = ProviderModelPricingSnapshotRepr::deserialize(deserializer)?;
        Self::new(
            repr.models_dev_provider_id,
            repr.models_dev_model_id,
            repr.route,
            repr.fetched_at,
            repr.base,
            repr.tiers,
        )
        .map_err(serde::de::Error::custom)
    }
}

pub fn official_provider_pricing_route(
    settings: &ProviderSettingsPayload,
) -> Option<ProviderPricingRouteKey> {
    let official_base_url = official_base_url(&settings.provider_kind)?;
    let mut configured_base_urls = settings
        .fields
        .iter()
        .filter(|field| field.key == "base_url");
    let configured_base_url = configured_base_urls.next();
    if configured_base_urls.next().is_some() {
        return None;
    }
    let configured_base_url = match configured_base_url {
        Some(field) => match &field.value {
            ProviderSettingValue::String { value } => value.trim(),
            _ => return None,
        },
        None => "",
    };
    if !configured_base_url.is_empty()
        && !official_url_matches(&settings.provider_kind, configured_base_url)
    {
        return None;
    }
    ProviderPricingRouteKey::new(&settings.provider_kind, official_base_url).ok()
}

pub fn estimate_request_cost(
    provider_kind: &str,
    usage: &ProviderUsageSnapshot,
    pricing: &ProviderModelPricingSnapshot,
) -> Option<UsdNanoAmount> {
    if usage.total_tokens == 0 || provider_kind != pricing.route.provider_kind {
        return None;
    }

    let uncached_input_tokens = match provider_kind {
        "anthropic" => usage.input_tokens,
        "openai" | "gemini" | "openrouter" | "deepseek" | "mistral" => usage
            .input_tokens
            .checked_sub(usage.cached_input_tokens)?
            .checked_sub(usage.cache_write_input_tokens)?,
        _ => return None,
    };
    let rates = pricing.rates_for_input_tokens(usage.input_tokens);
    let cache_read_rate = rates.cache_read.unwrap_or(rates.input);
    let cache_write_rate = rates.cache_write.unwrap_or(rates.input);
    let terms = [
        (uncached_input_tokens, rates.input),
        (usage.output_tokens, rates.output),
        (usage.cached_input_tokens, cache_read_rate),
        (usage.cache_write_input_tokens, cache_write_rate),
    ];
    let numerator = terms
        .into_iter()
        .try_fold(0_u128, |total, (tokens, rate)| {
            let term = u128::from(tokens).checked_mul(u128::from(rate.0))?;
            total.checked_add(term)
        })?;
    let rounded_nano_usd = numerator
        .checked_add(TOKENS_PER_MILLION / 2)?
        .checked_div(TOKENS_PER_MILLION)?;
    let rounded_nano_usd = u64::try_from(rounded_nano_usd).ok()?;
    UsdNanoAmount::new(rounded_nano_usd).ok()
}

fn official_base_url(provider_kind: &str) -> Option<&'static str> {
    match provider_kind {
        "openai" => Some("https://api.openai.com/v1"),
        "anthropic" => Some("https://api.anthropic.com"),
        "gemini" => Some("https://generativelanguage.googleapis.com"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        "deepseek" => Some("https://api.deepseek.com"),
        "mistral" => Some("https://api.mistral.ai"),
        _ => None,
    }
}

fn official_url_matches(provider_kind: &str, configured_base_url: &str) -> bool {
    let configured_base_url = configured_base_url.trim_end_matches('/');
    configured_base_url == official_base_url(provider_kind).unwrap_or_default()
        || provider_kind == "openai" && configured_base_url == "https://api.openai.com"
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn parse_usd_decimal_to_nanos(decimal: &str) -> Result<u64, PricingValidationError> {
    if decimal.is_empty() || decimal.starts_with(['-', '+']) {
        return Err(PricingValidationError::InvalidDecimal);
    }

    let (mantissa, exponent) = match decimal.find(['e', 'E']) {
        Some(index) => {
            let (mantissa, exponent) = decimal.split_at(index);
            if exponent[1..].contains(['e', 'E']) {
                return Err(PricingValidationError::InvalidDecimal);
            }
            (mantissa, parse_exponent(&exponent[1..])?)
        }
        None => (decimal, 0),
    };
    let (integer, fractional) = match mantissa.split_once('.') {
        Some((integer, fractional)) => {
            if fractional.contains('.') || fractional.is_empty() {
                return Err(PricingValidationError::InvalidDecimal);
            }
            (integer, fractional)
        }
        None => (mantissa, ""),
    };
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(PricingValidationError::InvalidDecimal);
    }

    let mut digits = format!("{integer}{fractional}");
    let first_nonzero = digits.find(|character| character != '0');
    let Some(first_nonzero) = first_nonzero else {
        return Ok(0);
    };
    digits.drain(..first_nonzero);
    let mut scale = i64::try_from(fractional.len())
        .map_err(|_| PricingValidationError::ValueOutOfRange)?
        .checked_sub(exponent)
        .ok_or(PricingValidationError::ValueOutOfRange)?;
    while digits.ends_with('0') {
        digits.pop();
        scale = scale
            .checked_sub(1)
            .ok_or(PricingValidationError::ValueOutOfRange)?;
    }
    if scale > i64::from(USD_NANOS) {
        return Err(PricingValidationError::DecimalPrecision);
    }

    let coefficient = digits
        .parse::<u128>()
        .map_err(|_| PricingValidationError::ValueOutOfRange)?;
    let power = i64::from(USD_NANOS)
        .checked_sub(scale)
        .ok_or(PricingValidationError::ValueOutOfRange)?;
    let power = u32::try_from(power).map_err(|_| PricingValidationError::ValueOutOfRange)?;
    let multiplier = 10_u128
        .checked_pow(power)
        .ok_or(PricingValidationError::ValueOutOfRange)?;
    let nanos = coefficient
        .checked_mul(multiplier)
        .ok_or(PricingValidationError::ValueOutOfRange)?;
    u64::try_from(nanos).map_err(|_| PricingValidationError::ValueOutOfRange)
}

fn parse_exponent(exponent: &str) -> Result<i64, PricingValidationError> {
    if exponent.is_empty() {
        return Err(PricingValidationError::InvalidDecimal);
    }
    let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PricingValidationError::InvalidDecimal);
    }
    exponent
        .parse()
        .map_err(|_| PricingValidationError::ValueOutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProviderRawPayload, ProviderSettingFieldValue};

    fn rate(value: u64) -> UsdNanoPerMillionTokens {
        UsdNanoPerMillionTokens::new(value)
    }

    fn rates(
        input: u64,
        output: u64,
        cache_read: Option<u64>,
        cache_write: Option<u64>,
    ) -> ProviderTokenPriceSnapshot {
        ProviderTokenPriceSnapshot::new(
            rate(input),
            rate(output),
            cache_read.map(rate),
            cache_write.map(rate),
        )
    }

    fn settings(
        provider_kind: &str,
        base_url: Option<ProviderSettingValue>,
    ) -> ProviderSettingsPayload {
        ProviderSettingsPayload {
            provider_kind: provider_kind.to_string(),
            fields: base_url
                .map(|value| ProviderSettingFieldValue {
                    key: "base_url".to_string(),
                    value,
                })
                .into_iter()
                .collect(),
        }
    }

    fn pricing(
        provider_kind: &str,
        base: ProviderTokenPriceSnapshot,
        tiers: Vec<ProviderTokenPriceTierSnapshot>,
    ) -> ProviderModelPricingSnapshot {
        ProviderModelPricingSnapshot::new(
            provider_kind,
            "model-exact",
            official_provider_pricing_route(&settings(provider_kind, None)).unwrap(),
            OffsetDateTime::UNIX_EPOCH,
            base,
            tiers,
        )
        .unwrap()
    }

    fn usage(
        input_tokens: u64,
        output_tokens: u64,
        cached_input_tokens: u64,
        cache_write_input_tokens: u64,
        reasoning_tokens: u64,
        total_tokens: u64,
    ) -> ProviderUsageSnapshot {
        ProviderUsageSnapshot {
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cache_write_input_tokens,
            reasoning_tokens,
            total_tokens,
            metadata: Some(ProviderRawPayload {
                provider_kind: "ignored".to_string(),
                value: serde_json::json!({"reportedCost": "ignored"}),
            }),
        }
    }

    #[test]
    fn pricing_decimal_parser_accepts_plain_and_scientific_exact_nanos() {
        let cases = [
            ("0", 0),
            ("1", 1_000_000_000),
            ("1.25", 1_250_000_000),
            ("1.25e-3", 1_250_000),
            ("1E+3", 1_000_000_000_000),
            ("0.000000001", 1),
            ("1.2300000000", 1_230_000_000),
        ];
        for (decimal, expected) in cases {
            assert_eq!(
                UsdNanoPerMillionTokens::from_usd_per_million_decimal(decimal)
                    .unwrap()
                    .as_u64(),
                expected,
                "{decimal}"
            );
        }
        assert_eq!(
            UsdNanoPerMillionTokens::from_usd_per_million_decimal("18446744073.709551615",)
                .unwrap()
                .as_u64(),
            u64::MAX
        );
    }

    #[test]
    fn pricing_decimal_parser_rejects_negative_overprecision_and_overflow() {
        for decimal in ["-1", "+1", "1.", ".1", "1e", "1e-10", "0.0000000001"] {
            assert!(
                UsdNanoPerMillionTokens::from_usd_per_million_decimal(decimal).is_err(),
                "{decimal}"
            );
        }
        assert!(
            UsdNanoPerMillionTokens::from_usd_per_million_decimal("18446744073.709551616").is_err()
        );
    }

    #[test]
    fn pricing_amount_rounds_once_half_up_and_enforces_sqlite_boundary() {
        let half_up = pricing("anthropic", rates(500_000, 0, None, None), vec![]);
        let half_down = pricing("anthropic", rates(499_999, 0, None, None), vec![]);
        assert_eq!(
            estimate_request_cost("anthropic", &usage(1, 0, 0, 0, 0, 1), &half_up)
                .unwrap()
                .as_u64(),
            1
        );
        assert_eq!(
            estimate_request_cost("anthropic", &usage(1, 0, 0, 0, 0, 1), &half_down)
                .unwrap()
                .as_u64(),
            0
        );

        let boundary = pricing("anthropic", rates(i64::MAX as u64, 0, None, None), vec![]);
        let overflow = pricing(
            "anthropic",
            rates(i64::MAX as u64 + 1, 0, None, None),
            vec![],
        );
        assert_eq!(
            estimate_request_cost(
                "anthropic",
                &usage(1_000_000, 0, 0, 0, 0, 1_000_000),
                &boundary,
            )
            .unwrap()
            .as_u64(),
            i64::MAX as u64
        );
        assert!(
            estimate_request_cost(
                "anthropic",
                &usage(1_000_000, 0, 0, 0, 0, 1_000_000),
                &overflow,
            )
            .is_none()
        );
        assert!(UsdNanoAmount::new(i64::MAX as u64 + 1).is_err());
        assert!(
            serde_json::from_value::<UsdNanoAmount>(serde_json::json!(i64::MAX as u64 + 1))
                .is_err()
        );
    }

    #[test]
    fn pricing_estimator_uses_separated_and_inclusive_input_formulas() {
        let base = rates(1_000_000, 2_000_000, Some(300_000), Some(400_000));
        let anthropic = pricing("anthropic", base.clone(), vec![]);
        let openai = pricing("openai", base, vec![]);
        let anthropic_usage = usage(100, 50, 20, 10, 40, 180);
        let inclusive_usage = usage(130, 50, 20, 10, 40, 180);

        assert_eq!(
            estimate_request_cost("anthropic", &anthropic_usage, &anthropic)
                .unwrap()
                .as_u64(),
            210
        );
        assert_eq!(
            estimate_request_cost("openai", &inclusive_usage, &openai)
                .unwrap()
                .as_u64(),
            210
        );
    }

    #[test]
    fn pricing_estimator_falls_back_cache_rates_and_does_not_double_count_reasoning() {
        let pricing = pricing("anthropic", rates(1_000_000, 2_000_000, None, None), vec![]);
        assert_eq!(
            estimate_request_cost(
                "anthropic",
                &usage(100, 50, 20, 10, u64::MAX, 180),
                &pricing,
            )
            .unwrap()
            .as_u64(),
            230
        );
    }

    #[test]
    fn pricing_estimator_selects_largest_matching_tier_at_boundary() {
        let pricing = pricing(
            "anthropic",
            rates(1_000_000, 0, None, None),
            vec![
                ProviderTokenPriceTierSnapshot::new(100, rates(2_000_000, 0, None, None)).unwrap(),
                ProviderTokenPriceTierSnapshot::new(200, rates(3_000_000, 0, None, None)).unwrap(),
            ],
        );
        for (tokens, expected) in [(99, 99), (100, 200), (200, 600), (250, 750)] {
            assert_eq!(
                estimate_request_cost("anthropic", &usage(tokens, 0, 0, 0, 0, tokens), &pricing,)
                    .unwrap()
                    .as_u64(),
                expected
            );
        }
    }

    #[test]
    fn pricing_estimator_rejects_unreported_underflow_unsupported_and_u128_overflow() {
        let openai = pricing("openai", rates(1, 1, Some(1), Some(1)), vec![]);
        assert!(estimate_request_cost("openai", &usage(1, 0, 0, 0, 0, 0), &openai).is_none());
        assert!(estimate_request_cost("openai", &usage(1, 0, 2, 0, 0, 3), &openai).is_none());
        assert!(estimate_request_cost("gemini", &usage(1, 0, 0, 0, 0, 1), &openai).is_none());

        let max = u64::MAX;
        let anthropic = pricing("anthropic", rates(max, max, Some(max), Some(max)), vec![]);
        assert!(
            estimate_request_cost(
                "anthropic",
                &usage(max, max, max, max, max, max),
                &anthropic,
            )
            .is_none()
        );
    }

    #[test]
    fn pricing_estimator_preserves_explicit_free_as_known_zero() {
        let free = pricing("anthropic", rates(0, 0, Some(0), Some(0)), vec![]);
        assert_eq!(
            estimate_request_cost("anthropic", &usage(1, 1, 1, 1, 1, 4), &free),
            Some(UsdNanoAmount::new(0).unwrap())
        );
    }

    #[test]
    fn pricing_snapshot_serde_roundtrips_provenance_and_rejects_invalid_tiers() {
        let snapshot = pricing(
            "openai",
            rates(1_000_000_000, 2_000_000_000, Some(100), None),
            vec![
                ProviderTokenPriceTierSnapshot::new(
                    200_000,
                    rates(2_000_000_000, 4_000_000_000, None, None),
                )
                .unwrap(),
            ],
        );
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["modelsDevProviderId"], "openai");
        assert_eq!(value["modelsDevModelId"], "model-exact");
        assert_eq!(value["route"]["providerKind"], "openai");
        assert_eq!(
            value["route"]["canonicalBaseUrl"],
            "https://api.openai.com/v1"
        );
        assert_eq!(
            serde_json::from_value::<ProviderModelPricingSnapshot>(value).unwrap(),
            snapshot
        );

        let invalid = serde_json::json!({
            "modelsDevProviderId": "openai",
            "modelsDevModelId": "model-exact",
            "route": {
                "providerKind": "openai",
                "canonicalBaseUrl": "https://api.openai.com/v1"
            },
            "fetchedAt": OffsetDateTime::UNIX_EPOCH,
            "base": { "input": 1, "output": 1, "cacheRead": null, "cacheWrite": null },
            "tiers": [
                { "inputTokenThreshold": 100, "rates": { "input": 1, "output": 1, "cacheRead": null, "cacheWrite": null } },
                { "inputTokenThreshold": 100, "rates": { "input": 1, "output": 1, "cacheRead": null, "cacheWrite": null } }
            ]
        });
        assert!(serde_json::from_value::<ProviderModelPricingSnapshot>(invalid).is_err());
    }

    #[test]
    fn official_pricing_route_only_accepts_builtin_official_settings() {
        let openai = settings(
            "openai",
            Some(ProviderSettingValue::String {
                value: " https://api.openai.com/ ".to_string(),
            }),
        );
        assert_eq!(
            official_provider_pricing_route(&openai)
                .unwrap()
                .canonical_base_url(),
            "https://api.openai.com/v1"
        );
        assert!(
            official_provider_pricing_route(&settings(
                "openai",
                Some(ProviderSettingValue::String {
                    value: "https://proxy.example/v1".to_string(),
                }),
            ))
            .is_none()
        );
        assert!(official_provider_pricing_route(&settings("ollama", None)).is_none());
        assert!(
            official_provider_pricing_route(&settings(
                "openai",
                Some(ProviderSettingValue::Number { value: 1.0 }),
            ))
            .is_none()
        );
    }
}
