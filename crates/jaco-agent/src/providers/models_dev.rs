use std::{collections::BTreeMap, time::Duration};

use jaco_core::{
    ProviderModelPricingSnapshot, ProviderPricingRouteKey, ProviderTokenPriceSnapshot,
    ProviderTokenPriceTierSnapshot, UsdNanoPerMillionTokens, official_provider_pricing_route,
};
use jaco_db::{NewProviderModel, ProviderRecord};
use serde_json::{Map, Number, Value};
use time::OffsetDateTime;

use super::ProviderModelFetchError;

const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

pub(super) async fn attach_pricing(
    provider: &ProviderRecord,
    mut models: Vec<NewProviderModel>,
) -> Result<Vec<NewProviderModel>, ProviderModelFetchError> {
    let Some((models_dev_provider_id, route)) = eligible_pricing_route(provider) else {
        return Ok(models);
    };

    let catalog = fetch_catalog(&provider.kind).await?;
    merge_pricing(
        &mut models,
        &catalog,
        models_dev_provider_id,
        route,
        OffsetDateTime::now_utc(),
    )
    .map_err(|message| catalog_error(&provider.kind, message))?;
    Ok(models)
}

fn eligible_pricing_route(
    provider: &ProviderRecord,
) -> Option<(&'static str, ProviderPricingRouteKey)> {
    if provider.settings.provider_kind != provider.kind {
        return None;
    }
    Some((
        models_dev_provider_id(&provider.kind)?,
        official_provider_pricing_route(&provider.settings)?,
    ))
}

async fn fetch_catalog(provider_kind: &str) -> Result<Value, ProviderModelFetchError> {
    fetch_catalog_from(provider_kind, MODELS_DEV_URL, REQUEST_TIMEOUT).await
}

async fn fetch_catalog_from(
    provider_kind: &str,
    url: &str,
    timeout: Duration,
) -> Result<Value, ProviderModelFetchError> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|error| catalog_error(provider_kind, format!("client setup failed: {error}")))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| catalog_error(provider_kind, format!("request failed: {error}")))?;
    let mut response = response
        .error_for_status()
        .map_err(|error| catalog_error(provider_kind, format!("request failed: {error}")))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(catalog_error(
            provider_kind,
            "response exceeds the 32 MiB limit",
        ));
    }

    let mut body = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| catalog_error(provider_kind, format!("response read failed: {error}")))?
    {
        if body.len() > MAX_RESPONSE_BYTES.saturating_sub(chunk.len()) {
            return Err(catalog_error(
                provider_kind,
                "response exceeds the 32 MiB limit",
            ));
        }
        body.extend_from_slice(&chunk);
    }

    serde_json::from_slice(&body)
        .map_err(|error| catalog_error(provider_kind, format!("decode failed: {error}")))
}

fn merge_pricing(
    models: &mut [NewProviderModel],
    catalog: &Value,
    models_dev_provider_id: &str,
    route: ProviderPricingRouteKey,
    fetched_at: OffsetDateTime,
) -> Result<(), String> {
    let catalog = catalog
        .as_object()
        .ok_or_else(|| "catalog root must be an object".to_string())?;
    let Some(provider) = catalog
        .get(models_dev_provider_id)
        .and_then(Value::as_object)
    else {
        return Ok(());
    };
    if provider.get("id").and_then(Value::as_str) != Some(models_dev_provider_id) {
        return Ok(());
    }
    let provider_models = provider
        .get("models")
        .and_then(Value::as_object)
        .ok_or_else(|| "matched catalog provider has invalid models".to_string())?;

    let mut parsed = BTreeMap::new();
    for model in models.iter() {
        let Some(catalog_model) = provider_models
            .get(&model.model_id)
            .and_then(Value::as_object)
        else {
            continue;
        };
        if catalog_model.get("id").and_then(Value::as_str) != Some(model.model_id.as_str()) {
            continue;
        }
        let Some(cost) = catalog_model.get("cost") else {
            continue;
        };
        if cost.is_null() {
            continue;
        }
        let cost = cost
            .as_object()
            .ok_or_else(|| "matched catalog model has invalid cost".to_string())?;
        let pricing = parse_pricing(
            models_dev_provider_id,
            &model.model_id,
            route.clone(),
            fetched_at,
            cost,
        )?;
        parsed.insert(model.model_id.clone(), pricing);
    }

    for model in models {
        model.pricing = parsed.remove(&model.model_id);
    }
    Ok(())
}

fn parse_pricing(
    models_dev_provider_id: &str,
    models_dev_model_id: &str,
    route: ProviderPricingRouteKey,
    fetched_at: OffsetDateTime,
    cost: &Map<String, Value>,
) -> Result<ProviderModelPricingSnapshot, String> {
    let base = parse_rates(cost)?;
    let mut tiers = match cost.get("tiers") {
        None => Vec::new(),
        Some(Value::Array(tiers)) => tiers
            .iter()
            .map(parse_tier)
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err("matched catalog model has invalid cost tiers".to_string()),
    };
    tiers.sort_by_key(ProviderTokenPriceTierSnapshot::input_token_threshold);

    ProviderModelPricingSnapshot::new(
        models_dev_provider_id,
        models_dev_model_id,
        route,
        fetched_at,
        base,
        tiers,
    )
    .map_err(|error| format!("matched catalog pricing is invalid: {error}"))
}

fn parse_tier(tier: &Value) -> Result<Option<ProviderTokenPriceTierSnapshot>, String> {
    let tier = tier
        .as_object()
        .ok_or_else(|| "matched catalog price tier must be an object".to_string())?;
    let condition = tier
        .get("tier")
        .and_then(Value::as_object)
        .ok_or_else(|| "matched catalog price tier has invalid condition".to_string())?;
    if condition.get("type").and_then(Value::as_str) != Some("context") {
        return Ok(None);
    }
    let threshold = condition
        .get("size")
        .and_then(Value::as_u64)
        .ok_or_else(|| "matched catalog price tier has invalid threshold".to_string())?;
    ProviderTokenPriceTierSnapshot::new(threshold, parse_rates(tier)?)
        .map(Some)
        .map_err(|error| format!("matched catalog price tier is invalid: {error}"))
}

fn parse_rates(cost: &Map<String, Value>) -> Result<ProviderTokenPriceSnapshot, String> {
    Ok(ProviderTokenPriceSnapshot::new(
        parse_required_rate(cost, "input")?,
        parse_required_rate(cost, "output")?,
        parse_optional_rate(cost, "cache_read")?,
        parse_optional_rate(cost, "cache_write")?,
    ))
}

fn parse_required_rate(
    cost: &Map<String, Value>,
    field: &str,
) -> Result<UsdNanoPerMillionTokens, String> {
    let number = cost
        .get(field)
        .and_then(Value::as_number)
        .ok_or_else(|| format!("matched catalog price has invalid {field}"))?;
    parse_rate(number, field)
}

fn parse_optional_rate(
    cost: &Map<String, Value>,
    field: &str,
) -> Result<Option<UsdNanoPerMillionTokens>, String> {
    cost.get(field)
        .map(|value| {
            value
                .as_number()
                .ok_or_else(|| format!("matched catalog price has invalid {field}"))
                .and_then(|number| parse_rate(number, field))
        })
        .transpose()
}

fn parse_rate(number: &Number, field: &str) -> Result<UsdNanoPerMillionTokens, String> {
    UsdNanoPerMillionTokens::from_usd_per_million_decimal(&number.to_string())
        .map_err(|error| format!("matched catalog price has invalid {field}: {error}"))
}

fn models_dev_provider_id(provider_kind: &str) -> Option<&'static str> {
    match provider_kind {
        "openai" => Some("openai"),
        "anthropic" => Some("anthropic"),
        "gemini" => Some("google"),
        "openrouter" => Some("openrouter"),
        "deepseek" => Some("deepseek"),
        "mistral" => Some("mistral"),
        _ => None,
    }
}

fn catalog_error(provider_kind: &str, message: impl Into<String>) -> ProviderModelFetchError {
    ProviderModelFetchError::ListingFailed {
        provider_kind: provider_kind.to_string(),
        message: format!("models.dev catalog {}", message.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
    };

    use jaco_core::{
        ProviderSecretRefs, ProviderSettingFieldValue, ProviderSettingValue,
        ProviderSettingsPayload,
    };
    use rig::model::Model;

    use super::*;

    #[test]
    fn pricing_eligibility_requires_supported_kind_and_official_route() {
        let openai = provider("openai", None);
        let (provider_id, route) = eligible_pricing_route(&openai).expect("official OpenAI");
        assert_eq!(provider_id, "openai");
        assert_eq!(route.provider_kind(), "openai");

        let gemini = provider("gemini", None);
        assert_eq!(
            eligible_pricing_route(&gemini).map(|(provider_id, _)| provider_id),
            Some("google")
        );
        assert!(
            eligible_pricing_route(&provider("openai", Some("https://compatible.example/v1")))
                .is_none()
        );
        assert!(eligible_pricing_route(&provider("ollama", None)).is_none());

        let mut mismatched = provider("openai", None);
        mismatched.settings.provider_kind = "anthropic".to_string();
        assert!(eligible_pricing_route(&mismatched).is_none());
    }

    #[test]
    fn pricing_merge_requires_exact_provider_and_model_identity() {
        let provider = provider("openai", None);
        let route = eligible_pricing_route(&provider).unwrap().1;
        let mut models = vec![
            model(&provider, "gpt-exact"),
            model(&provider, "gpt-family"),
            model(&provider, "gpt-missing-cost"),
        ];
        let catalog = serde_json::from_str(
            r#"{
                "openai": {
                    "id": "openai",
                    "models": {
                        "gpt-exact": {
                            "id": "gpt-exact",
                            "cost": {
                                "input": 1.234567891,
                                "output": 2e0,
                                "cache_read": 0.1,
                                "cache_write": 3,
                                "reasoning": 99,
                                "tiers": [{
                                    "tier": {"type": "context", "size": 200000},
                                    "input": 4,
                                    "output": 5,
                                    "cache_read": 0.4,
                                    "input_audio": 88
                                }]
                            },
                            "experimental": {"modes": {"fast": {"cost": {"input": 0}}}}
                        },
                        "gpt-family-v2": {
                            "id": "gpt-family-v2",
                            "cost": {"input": 1, "output": 2}
                        },
                        "gpt-missing-cost": {"id": "another-id"}
                    }
                },
                "anthropic": {
                    "id": "anthropic",
                    "models": {
                        "gpt-family": {
                            "id": "gpt-family",
                            "cost": {"input": 1, "output": 2}
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        merge_pricing(
            &mut models,
            &catalog,
            "openai",
            route,
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();

        let pricing = models[0].pricing.as_ref().expect("exact price");
        assert_eq!(pricing.models_dev_provider_id(), "openai");
        assert_eq!(pricing.models_dev_model_id(), "gpt-exact");
        assert_eq!(pricing.fetched_at(), OffsetDateTime::UNIX_EPOCH);
        assert_eq!(pricing.base().input().as_u64(), 1_234_567_891);
        assert_eq!(pricing.base().output().as_u64(), 2_000_000_000);
        assert_eq!(pricing.base().cache_read().unwrap().as_u64(), 100_000_000);
        assert_eq!(
            pricing.base().cache_write().unwrap().as_u64(),
            3_000_000_000
        );
        assert_eq!(pricing.tiers().len(), 1);
        assert_eq!(pricing.tiers()[0].input_token_threshold(), 200_000);
        assert_eq!(pricing.tiers()[0].rates().input().as_u64(), 4_000_000_000);
        assert!(
            models[1].pricing.is_none(),
            "no family or cross-provider fallback"
        );
        assert!(
            models[2].pricing.is_none(),
            "payload model id must also match"
        );
    }

    #[test]
    fn pricing_merge_treats_identity_mismatch_as_unpriced_and_invalid_price_as_failure() {
        let provider = provider("openai", None);
        let route = eligible_pricing_route(&provider).unwrap().1;
        let mut models = vec![model(&provider, "gpt-exact")];
        let wrong_provider: Value = serde_json::from_str(
            r#"{"openai":{"id":"not-openai","models":{"gpt-exact":{"id":"gpt-exact","cost":{"input":1,"output":2}}}}}"#,
        )
        .unwrap();
        merge_pricing(
            &mut models,
            &wrong_provider,
            "openai",
            route.clone(),
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap();
        assert!(models[0].pricing.is_none());

        let invalid_price: Value = serde_json::from_str(
            r#"{"openai":{"id":"openai","models":{"gpt-exact":{"id":"gpt-exact","cost":{"input":1.0000000001,"output":2}}}}}"#,
        )
        .unwrap();
        assert!(
            merge_pricing(
                &mut models,
                &invalid_price,
                "openai",
                route,
                OffsetDateTime::UNIX_EPOCH,
            )
            .is_err()
        );
        assert!(
            models[0].pricing.is_none(),
            "failed merge is not partially applied"
        );
    }

    #[tokio::test]
    async fn pricing_catalog_transport_rejects_status_decode_size_and_timeout() {
        let status = serve_once(
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            Duration::ZERO,
        );
        assert!(
            fetch_catalog_from("openai", &status.url, Duration::from_secs(1))
                .await
                .is_err()
        );
        status.finish();

        let invalid_json = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\nnot-json",
            Duration::ZERO,
        );
        assert!(
            fetch_catalog_from("openai", &invalid_json.url, Duration::from_secs(1))
                .await
                .is_err()
        );
        invalid_json.finish();

        let oversized = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 33554433\r\nConnection: close\r\n\r\n",
            Duration::ZERO,
        );
        assert!(
            fetch_catalog_from("openai", &oversized.url, Duration::from_secs(1))
                .await
                .is_err()
        );
        oversized.finish();

        let delayed = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            Duration::from_millis(100),
        );
        assert!(
            fetch_catalog_from("openai", &delayed.url, Duration::from_millis(10))
                .await
                .is_err()
        );
        delayed.finish();
    }

    #[tokio::test]
    async fn pricing_catalog_request_has_fixed_shape_and_no_credentials() {
        let server = serve_once(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
            Duration::ZERO,
        );
        fetch_catalog_from("openai", &server.url, Duration::from_secs(1))
            .await
            .unwrap();
        let request = server.finish();
        let request = request.to_ascii_lowercase();
        assert!(request.starts_with("get /api.json http/1.1\r\n"));
        assert!(!request.contains("authorization:"));
        assert!(!request.contains("api_key"));
        assert!(!request.contains("sk-test"));
    }

    fn provider(kind: &str, base_url: Option<&str>) -> ProviderRecord {
        ProviderRecord {
            id: format!("{kind}-provider"),
            kind: kind.to_string(),
            display_name: kind.to_string(),
            enabled: true,
            settings: ProviderSettingsPayload {
                provider_kind: kind.to_string(),
                fields: base_url
                    .map(|base_url| ProviderSettingFieldValue {
                        key: "base_url".to_string(),
                        value: ProviderSettingValue::String {
                            value: base_url.to_string(),
                        },
                    })
                    .into_iter()
                    .collect(),
            },
            secret_refs: ProviderSecretRefs { refs: Vec::new() },
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    fn model(provider: &ProviderRecord, model_id: &str) -> NewProviderModel {
        super::super::provider_model_from_rig_model(
            provider,
            Model {
                id: model_id.to_string(),
                name: None,
                description: None,
                r#type: Some("chat".to_string()),
                created_at: None,
                owned_by: None,
                context_length: None,
                max_output_tokens: None,
            },
        )
    }

    struct TestServer {
        url: String,
        request: mpsc::Receiver<String>,
        thread: thread::JoinHandle<()>,
    }

    impl TestServer {
        fn finish(self) -> String {
            self.thread.join().unwrap();
            self.request.recv().unwrap()
        }
    }

    fn serve_once(response: &'static [u8], delay: Duration) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (request_sender, request) = mpsc::channel();
        let thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request_bytes = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request_bytes.extend_from_slice(&buffer[..read]);
                if request_bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            request_sender
                .send(String::from_utf8_lossy(&request_bytes).into_owned())
                .unwrap();
            thread::sleep(delay);
            let _ = stream.write_all(response);
        });
        TestServer {
            url: format!("http://{address}/api.json"),
            request,
            thread,
        }
    }
}
