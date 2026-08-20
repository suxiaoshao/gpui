use super::*;
use crate::{UsageAnalyticsAggregate, UsageAnalyticsFiniteRange, UsageAnalyticsRange};
use diesel::sql_types::{BigInt, Nullable, Text, TimestamptzSqlite};
use time::{Date, Month, OffsetDateTime, UtcOffset};

#[derive(Debug, Clone, Copy)]
struct TokenCounts {
    input: i64,
    output: i64,
    cached_input: i64,
    cache_write_input: i64,
    reasoning: i64,
    total: i64,
}

impl TokenCounts {
    const ZERO: Self = Self {
        input: 0,
        output: 0,
        cached_input: 0,
        cache_write_input: 0,
        reasoning: 0,
        total: 0,
    };
}

fn datetime(
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    offset: UtcOffset,
) -> OffsetDateTime {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day)
        .unwrap()
        .with_hms(hour, minute, 0)
        .unwrap()
        .assume_offset(offset)
}

fn finite_range(
    year: i32,
    month: u8,
    start_day: u8,
    end_day: u8,
    offset: UtcOffset,
) -> UsageAnalyticsRange {
    UsageAnalyticsRange::Finite(
        UsageAnalyticsFiniteRange::new(
            datetime(year, month, start_day, 0, 0, offset),
            datetime(year, month, end_day, 0, 0, offset),
            offset,
        )
        .unwrap(),
    )
}

fn fresh_store() -> (tempfile::TempDir, FreshStore) {
    let dir = tempdir().unwrap();
    let store = FreshStore::open_or_create_initial(dir.path().join(DATABASE_FILE)).unwrap();
    (dir, store)
}

fn insert_provider_catalog(
    store: &FreshStore,
    provider_id: &str,
    provider_label: &str,
    model_id: Option<&str>,
    model_label: Option<&str>,
) {
    let mut conn = store.pool().get().unwrap();
    let now = datetime(2026, 8, 20, 0, 0, UtcOffset::UTC);
    sql_query(
        "INSERT INTO providers (
            id, kind, display_name, enabled, settings_json, secret_refs_json, created_at, updated_at
         ) VALUES (?1, 'test', ?2, 1, '{}', '{}', ?3, ?3)
         ON CONFLICT(id) DO UPDATE SET display_name = excluded.display_name,
             updated_at = excluded.updated_at",
    )
    .bind::<Text, _>(provider_id)
    .bind::<Text, _>(provider_label)
    .bind::<TimestamptzSqlite, _>(now)
    .execute(&mut conn)
    .unwrap();

    if let Some(model_id) = model_id {
        sql_query(
            "INSERT OR REPLACE INTO provider_models (
                id, provider_id, model_id, display_name, enabled, capabilities_json,
                metadata_json, fetched_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 1, '{}', '{}', ?5, ?5, ?5)",
        )
        .bind::<Text, _>(format!("catalog-{provider_id}-{model_id}"))
        .bind::<Text, _>(provider_id)
        .bind::<Text, _>(model_id)
        .bind::<Nullable<Text>, _>(model_label)
        .bind::<TimestamptzSqlite, _>(now)
        .execute(&mut conn)
        .unwrap();
    }
}

fn insert_usage(
    store: &FreshStore,
    id: &str,
    provider_id: &str,
    model_id: &str,
    date_key: &str,
    created_at: OffsetDateTime,
    counts: TokenCounts,
) {
    let mut conn = store.pool().get().unwrap();
    conn.batch_execute("PRAGMA foreign_keys = OFF;").unwrap();
    sql_query(
        "INSERT INTO usage_events (
            id, provider_step_id, conversation_id, provider_id, model_id, date_key,
            input_tokens, output_tokens, cached_input_tokens, cache_write_input_tokens,
            reasoning_tokens, total_tokens, usage_json, created_at
         ) VALUES (
            ?1, ?2, 'conversation', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, '{}', ?12
         )",
    )
    .bind::<Text, _>(id)
    .bind::<Text, _>(format!("step-{id}"))
    .bind::<Text, _>(provider_id)
    .bind::<Text, _>(model_id)
    .bind::<Text, _>(date_key)
    .bind::<BigInt, _>(counts.input)
    .bind::<BigInt, _>(counts.output)
    .bind::<BigInt, _>(counts.cached_input)
    .bind::<BigInt, _>(counts.cache_write_input)
    .bind::<BigInt, _>(counts.reasoning)
    .bind::<BigInt, _>(counts.total)
    .bind::<TimestamptzSqlite, _>(created_at)
    .execute(&mut conn)
    .unwrap();
    conn.batch_execute("PRAGMA foreign_keys = ON;").unwrap();
}

#[test]
fn usage_analytics_finite_range_requires_ordered_local_midnights() {
    let offset = UtcOffset::from_hms(5, 30, 0).unwrap();
    let start = datetime(2026, 8, 1, 0, 0, offset);
    let end = datetime(2026, 8, 2, 0, 0, offset);

    let range = UsageAnalyticsFiniteRange::new(start, end, offset).unwrap();
    assert_eq!(range.start_utc().offset(), UtcOffset::UTC);
    assert_eq!(range.end_utc().offset(), UtcOffset::UTC);
    assert_eq!(range.local_offset(), offset);
    assert!(UsageAnalyticsFiniteRange::new(start, start, offset).is_none());
    assert!(UsageAnalyticsFiniteRange::new(end, start, offset).is_none());
    assert!(
        UsageAnalyticsFiniteRange::new(start + time::Duration::hours(1), end, offset).is_none()
    );
    assert!(
        UsageAnalyticsFiniteRange::new(start, end + time::Duration::minutes(1), offset).is_none()
    );
    let extreme_positive = UtcOffset::from_hms(23, 59, 59).unwrap();
    let extreme_negative = UtcOffset::from_hms(-23, -59, -59).unwrap();
    let minimum = Date::MIN.midnight().assume_utc();
    let maximum = Date::MAX.midnight().assume_utc();
    assert!(
        UsageAnalyticsFiniteRange::new(
            minimum,
            minimum.checked_add(time::Duration::days(1)).unwrap(),
            extreme_negative,
        )
        .is_none()
    );
    assert!(
        UsageAnalyticsFiniteRange::new(
            maximum.checked_sub(time::Duration::days(1)).unwrap(),
            maximum,
            extreme_positive,
        )
        .is_none()
    );

    let invalid = UsageAnalyticsAggregate {
        reported_request_count: 1,
        total_covered_request_count: 2,
        ..Default::default()
    };
    assert_eq!(invalid.partial_request_count(), None);
}

#[test]
fn usage_analytics_uses_half_open_utc_bounds_and_ignores_date_key() {
    let (_dir, store) = fresh_store();
    insert_provider_catalog(
        &store,
        "provider-a",
        "Provider A",
        Some("model-a"),
        Some("Model A"),
    );
    let start = datetime(2026, 8, 2, 0, 0, UtcOffset::UTC);
    let end = datetime(2026, 8, 3, 0, 0, UtcOffset::UTC);
    for (id, at) in [
        ("before", start - time::Duration::seconds(1)),
        ("start", start),
        ("inside", end - time::Duration::seconds(1)),
        ("end", end),
    ] {
        insert_usage(
            &store,
            id,
            "provider-a",
            "model-a",
            "1900-01-01",
            at,
            TokenCounts {
                total: 1,
                ..TokenCounts::ZERO
            },
        );
    }

    let snapshot = store
        .repository()
        .usage_analytics(finite_range(2026, 8, 2, 3, UtcOffset::UTC))
        .unwrap();
    assert_eq!(snapshot.summary.request_count, 2);
    assert_eq!(snapshot.summary.total_tokens, 2);
    assert_eq!(snapshot.daily.len(), 1);
    assert_eq!(snapshot.daily[0].local_date, start.date());
}

#[test]
fn usage_analytics_groups_daily_by_positive_negative_and_sub_hour_offsets() {
    for (index, offset) in [
        UtcOffset::from_hms(8, 0, 0).unwrap(),
        UtcOffset::from_hms(-7, 0, 0).unwrap(),
        UtcOffset::from_hms(5, 45, 0).unwrap(),
    ]
    .into_iter()
    .enumerate()
    {
        let (_dir, store) = fresh_store();
        insert_usage(
            &store,
            &format!("offset-{index}"),
            "missing-provider",
            "model",
            "wrong-date",
            datetime(2026, 8, 2, 0, 15, offset),
            TokenCounts {
                total: 9,
                ..TokenCounts::ZERO
            },
        );
        let snapshot = store
            .repository()
            .usage_analytics(finite_range(2026, 8, 1, 4, offset))
            .unwrap();
        assert_eq!(snapshot.daily.len(), 3);
        assert_eq!(snapshot.daily[0].aggregate.request_count, 0);
        assert_eq!(snapshot.daily[1].aggregate.request_count, 1);
        assert_eq!(snapshot.daily[2].aggregate.request_count, 0);
        assert_eq!(snapshot.daily[1].local_date.day(), 2);
    }
}

#[test]
fn usage_analytics_counts_each_event_and_applies_coverage_predicates() {
    let (_dir, store) = fresh_store();
    for (id, counts) in [
        ("unreported", TokenCounts::ZERO),
        (
            "partial-input",
            TokenCounts {
                input: 7,
                ..TokenCounts::ZERO
            },
        ),
        (
            "partial-cache-write",
            TokenCounts {
                cache_write_input: 11,
                ..TokenCounts::ZERO
            },
        ),
        (
            "covered-total-only",
            TokenCounts {
                total: 13,
                ..TokenCounts::ZERO
            },
        ),
    ] {
        insert_usage(
            &store,
            id,
            "provider",
            "model",
            "ignored",
            datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
            counts,
        );
    }

    let snapshot = store
        .repository()
        .usage_analytics(UsageAnalyticsRange::AllTime)
        .unwrap();
    assert_eq!(snapshot.summary.request_count, 4);
    assert_eq!(snapshot.summary.reported_request_count, 3);
    assert_eq!(snapshot.summary.unreported_request_count, 1);
    assert_eq!(snapshot.summary.total_covered_request_count, 1);
    assert_eq!(snapshot.summary.partial_request_count(), Some(2));
    assert_eq!(snapshot.summary.input_tokens, 7);
    assert_eq!(snapshot.summary.cache_write_input_tokens, 11);
    assert_eq!(snapshot.summary.total_tokens, 13);
    assert!(snapshot.daily.is_empty());
}

#[test]
fn usage_analytics_counts_completed_steps_independently_after_their_run_fails() {
    let (_dir, store) = fresh_store();
    let repo = store.repository();
    let project = repo
        .insert_project(project("analytics-failed-run"))
        .unwrap();
    let conversation = repo.insert_conversation(conversation(&project)).unwrap();
    let provider = repo.insert_provider(provider()).unwrap();
    let model = repo
        .upsert_provider_model(provider_model(&provider.id, "gpt-5.6", "GPT-5.6"))
        .unwrap();
    let trigger = repo
        .append_conversation_entry(message_item(&conversation.id, "two provider steps"))
        .unwrap();
    let run = repo
        .insert_agent_run(NewAgentRun {
            conversation_id: conversation.id.clone(),
            trigger_entry_id: trigger.id.clone(),
            trigger_kind: AgentRunTriggerKind::User,
            input: agent_run_input(&trigger.id, &provider.id, &model.model_id),
        })
        .unwrap();

    let mut step_ids = Vec::new();
    for seq in [1, 2] {
        let step = repo
            .insert_provider_step(NewProviderStep {
                agent_run_id: run.id.clone(),
                seq,
                status: ProviderStepStatus::Running,
                request_snapshot: provider_step_request(&provider.id, &model.model_id, &trigger.id),
                response_snapshot: None,
                state_snapshot: None,
                settings_snapshot: run_settings(&provider.id, &model.model_id),
                error: None,
            })
            .unwrap();
        let completed = repo
            .complete_provider_step_with_usage(
                &step.id,
                crate::CompleteProviderStep {
                    response_snapshot: provider_step_response(),
                    state_snapshot: provider_run_state(&provider.id),
                    continuation: None,
                    usage: usage_snapshot(),
                },
            )
            .unwrap();
        step_ids.push(completed.step.id);
    }
    assert_ne!(step_ids[0], step_ids[1]);

    let error = run_error();
    repo.finish_agent_run(
        &run.id,
        FinishAgentRun {
            status: AgentRunStatus::Failed,
            stopped_reason: AgentStoppedReason::Failed,
            error: Some(error.clone()),
            final_entry: AgentRunFinalEntry::Append(Box::new(NewConversationEntry {
                conversation_id: conversation.id,
                status: ConversationEntryStatus::Failed,
                agent_run_id: Some(run.id.clone()),
                provider_step_id: None,
                tool_invocation_id: None,
                provider_item_id: None,
                payload: ConversationEntryPayload::Error(error),
            })),
        },
    )
    .unwrap();

    assert_eq!(
        repo.get_agent_run(&run.id).unwrap().unwrap().status,
        AgentRunStatus::Failed
    );
    let snapshot = repo.usage_analytics(UsageAnalyticsRange::AllTime).unwrap();
    assert_eq!(snapshot.summary.request_count, 2);
    assert_eq!(snapshot.summary.reported_request_count, 2);
    assert_eq!(snapshot.summary.total_covered_request_count, 2);
    assert_eq!(snapshot.summary.input_tokens, 20);
    assert_eq!(snapshot.summary.total_tokens, 78);
    assert_eq!(snapshot.provider_models.len(), 1);
    assert_eq!(snapshot.provider_models[0].aggregate.request_count, 2);
}

#[test]
fn usage_analytics_sums_six_dimensions_exactly_without_cache_double_counting() {
    let (_dir, store) = fresh_store();
    let large = 9_007_199_254_740_993_i64;
    insert_usage(
        &store,
        "six-dimensions",
        "provider",
        "model",
        "ignored",
        datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
        TokenCounts {
            input: large,
            output: 2,
            cached_input: 3,
            cache_write_input: 5,
            reasoning: 7,
            total: large + 17,
        },
    );

    let snapshot = store
        .repository()
        .usage_analytics(UsageAnalyticsRange::AllTime)
        .unwrap();
    assert_eq!(snapshot.summary.input_tokens, large as u64);
    assert_eq!(snapshot.summary.output_tokens, 2);
    assert_eq!(snapshot.summary.cached_input_tokens, 3);
    assert_eq!(snapshot.summary.cache_write_input_tokens, 5);
    assert_eq!(snapshot.summary.reasoning_tokens, 7);
    assert_eq!(snapshot.summary.total_tokens, (large + 17) as u64);
}

#[test]
fn usage_analytics_rejects_negative_values_and_sqlite_sum_overflow() {
    let (_dir, negative_store) = fresh_store();
    insert_usage(
        &negative_store,
        "negative",
        "provider",
        "model",
        "ignored",
        datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
        TokenCounts {
            reasoning: -1,
            ..TokenCounts::ZERO
        },
    );
    assert!(matches!(
        negative_store
            .repository()
            .usage_analytics(UsageAnalyticsRange::AllTime),
        Err(crate::DbError::Invariant(message)) if message.contains("negative token")
    ));

    let (_dir, overflow_store) = fresh_store();
    for id in ["overflow-a", "overflow-b"] {
        insert_usage(
            &overflow_store,
            id,
            "provider",
            "model",
            "ignored",
            datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
            TokenCounts {
                input: i64::MAX,
                ..TokenCounts::ZERO
            },
        );
    }
    assert!(matches!(
        overflow_store
            .repository()
            .usage_analytics(UsageAnalyticsRange::AllTime),
        Err(crate::DbError::Diesel(_))
    ));
}

#[test]
fn usage_analytics_returns_dense_days_and_all_time_omits_daily() {
    let (_dir, store) = fresh_store();
    insert_usage(
        &store,
        "dense",
        "provider",
        "model",
        "ignored",
        datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
        TokenCounts {
            total: 4,
            ..TokenCounts::ZERO
        },
    );
    let finite = store
        .repository()
        .usage_analytics(finite_range(2026, 8, 1, 5, UtcOffset::UTC))
        .unwrap();
    assert_eq!(finite.daily.len(), 4);
    assert_eq!(
        finite
            .daily
            .iter()
            .map(|bucket| bucket.local_date.day())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(finite.daily[1].aggregate.total_tokens, 4);

    let all_time = store
        .repository()
        .usage_analytics(UsageAnalyticsRange::AllTime)
        .unwrap();
    assert!(all_time.daily.is_empty());
    assert_eq!(all_time.summary, finite.summary);
}

#[test]
fn usage_analytics_groups_by_stable_ids_with_current_optional_labels_and_sorting() {
    let (_dir, store) = fresh_store();
    insert_provider_catalog(
        &store,
        "provider-b",
        "Provider B",
        Some("model-b"),
        Some("Old Model"),
    );
    insert_provider_catalog(&store, "provider-a", "Provider A", None, None);
    for (id, provider_id, model_id, total) in [
        ("b", "provider-b", "model-b", 20),
        ("a", "provider-a", "missing-model", 20),
        ("missing", "missing-provider", "model", 10),
    ] {
        insert_usage(
            &store,
            id,
            provider_id,
            model_id,
            "ignored",
            datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
            TokenCounts {
                total,
                ..TokenCounts::ZERO
            },
        );
    }
    insert_provider_catalog(
        &store,
        "provider-b",
        "Provider B Renamed",
        Some("model-b"),
        Some("New Model"),
    );

    let snapshot = store
        .repository()
        .usage_analytics(UsageAnalyticsRange::AllTime)
        .unwrap();
    assert_eq!(
        snapshot
            .provider_models
            .iter()
            .map(|bucket| (bucket.provider_id.as_str(), bucket.model_id.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("provider-a", "missing-model"),
            ("provider-b", "model-b"),
            ("missing-provider", "model"),
        ]
    );
    assert_eq!(
        snapshot.provider_models[0].provider_label.as_deref(),
        Some("Provider A")
    );
    assert_eq!(snapshot.provider_models[0].model_label, None);
    assert_eq!(
        snapshot.provider_models[1].provider_label.as_deref(),
        Some("Provider B Renamed")
    );
    assert_eq!(
        snapshot.provider_models[1].model_label.as_deref(),
        Some("New Model")
    );
    assert_eq!(snapshot.provider_models[2].provider_label, None);
}

#[test]
fn usage_analytics_empty_snapshot_preserves_requested_shape() {
    let (_dir, store) = fresh_store();
    let range = finite_range(2026, 8, 1, 4, UtcOffset::UTC);
    let snapshot = store.repository().usage_analytics(range).unwrap();
    assert_eq!(snapshot.range, range);
    assert!(snapshot.summary.is_empty());
    assert_eq!(snapshot.summary.partial_request_count(), Some(0));
    assert_eq!(snapshot.daily.len(), 3);
    assert!(
        snapshot
            .daily
            .iter()
            .all(|bucket| bucket.aggregate == UsageAnalyticsAggregate::default())
    );
    assert!(snapshot.provider_models.is_empty());
}

#[derive(diesel::QueryableByName)]
struct QueryPlanRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn usage_analytics_fresh_schema_and_production_query_shapes_use_created_at_index() {
    let (_dir, store) = fresh_store();
    assert_eq!(crate::migrations::SCHEMA_VERSION, 1);
    assert_eq!(crate::migrations::MIGRATIONS.len(), 1);
    assert_eq!(
        crate::migrations::MIGRATIONS[0].name,
        "0001_create_fresh_schema"
    );

    let mut conn = store.pool().get().unwrap();
    let index_count = sql_query(
        "SELECT COUNT(*) AS value FROM sqlite_master
         WHERE type = 'index' AND name = 'idx_usage_events_created_at'",
    )
    .load::<CountRow>(&mut conn)
    .unwrap()[0]
        .value;
    assert_eq!(index_count, 1);

    let start = datetime(2026, 8, 1, 0, 0, UtcOffset::UTC);
    let end = datetime(2026, 9, 1, 0, 0, UtcOffset::UTC);
    let summary_plan = sql_query(format!(
        "EXPLAIN QUERY PLAN {}",
        crate::repository::SUMMARY_FINITE_SQL
    ))
    .bind::<TimestamptzSqlite, _>(start)
    .bind::<TimestamptzSqlite, _>(end)
    .load::<QueryPlanRow>(&mut conn)
    .unwrap();
    assert_plan_uses_created_at_index(&summary_plan, "summary");

    let daily_plan = sql_query(format!(
        "EXPLAIN QUERY PLAN {}",
        crate::repository::DAILY_FINITE_SQL
    ))
    .bind::<TimestamptzSqlite, _>(start)
    .bind::<TimestamptzSqlite, _>(end)
    .bind::<diesel::sql_types::Integer, _>(0)
    .load::<QueryPlanRow>(&mut conn)
    .unwrap();
    assert_plan_uses_created_at_index(&daily_plan, "daily");

    let provider_model_plan = sql_query(format!(
        "EXPLAIN QUERY PLAN {}",
        crate::repository::PROVIDER_MODELS_FINITE_SQL
    ))
    .bind::<TimestamptzSqlite, _>(start)
    .bind::<TimestamptzSqlite, _>(end)
    .load::<QueryPlanRow>(&mut conn)
    .unwrap();
    assert_plan_uses_created_at_index(&provider_model_plan, "provider/model");
}

fn assert_plan_uses_created_at_index(plan: &[QueryPlanRow], query_name: &str) {
    assert!(
        plan.iter().any(|row| {
            row.detail.contains("SEARCH usage_events USING")
                && row.detail.contains("idx_usage_events_created_at")
        }),
        "{query_name} query plan did not use created_at index: {}",
        plan.iter()
            .map(|row| row.detail.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    );
}
