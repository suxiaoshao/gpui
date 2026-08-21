use super::*;
use crate::{
    UsageAnalyticsAggregate, UsageAnalyticsFiniteRange, UsageAnalyticsQuery, UsageAnalyticsRange,
};
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
) -> UsageAnalyticsFiniteRange {
    UsageAnalyticsFiniteRange::new(
        datetime(year, month, start_day, 0, 0, offset),
        datetime(year, month, end_day, 0, 0, offset),
        offset,
    )
    .unwrap()
}

fn finite_range_for_days(
    year: i32,
    month: u8,
    start_day: u8,
    day_count: i64,
    offset: UtcOffset,
) -> UsageAnalyticsFiniteRange {
    let start = datetime(year, month, start_day, 0, 0, offset);
    UsageAnalyticsFiniteRange::new(start, start + time::Duration::days(day_count), offset).unwrap()
}

fn same_range_query(range: UsageAnalyticsFiniteRange) -> UsageAnalyticsQuery {
    UsageAnalyticsQuery {
        selected_range: UsageAnalyticsRange::Finite(range),
        activity_range: range,
    }
}

fn all_time_query(activity_range: UsageAnalyticsFiniteRange) -> UsageAnalyticsQuery {
    UsageAnalyticsQuery {
        selected_range: UsageAnalyticsRange::AllTime,
        activity_range,
    }
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

fn set_usage_cost(
    store: &FreshStore,
    id: &str,
    cost_amount_nano_usd: i64,
) -> diesel::QueryResult<usize> {
    let mut conn = store.pool().get().unwrap();
    sql_query("UPDATE usage_events SET cost_amount_nano_usd = ?2 WHERE id = ?1")
        .bind::<Text, _>(id)
        .bind::<BigInt, _>(cost_amount_nano_usd)
        .execute(&mut conn)
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
        .usage_analytics(same_range_query(finite_range(
            2026,
            8,
            2,
            3,
            UtcOffset::UTC,
        )))
        .unwrap();
    assert_eq!(snapshot.selected_summary.request_count, 2);
    assert_eq!(snapshot.selected_summary.total_tokens, 2);
    assert_eq!(snapshot.provider_models.len(), 1);
    assert_eq!(
        snapshot.provider_models[0].aggregate,
        snapshot.selected_summary
    );
    assert_eq!(snapshot.activity.summary, snapshot.selected_summary);
    assert_eq!(snapshot.activity.daily.len(), 1);
    assert_eq!(snapshot.activity.daily[0].local_date, start.date());
}

#[test]
fn usage_analytics_groups_cross_year_leap_activity_by_fixed_offsets() {
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
            datetime(2024, 2, 29, 0, 15, offset),
            TokenCounts {
                total: 9,
                ..TokenCounts::ZERO
            },
        );
        let selected_range = finite_range_for_days(2024, 2, 28, 3, offset);
        let activity_range = finite_range_for_days(2023, 3, 2, 365, offset);
        let snapshot = store
            .repository()
            .usage_analytics(UsageAnalyticsQuery {
                selected_range: UsageAnalyticsRange::Finite(selected_range),
                activity_range,
            })
            .unwrap();
        assert_eq!(snapshot.provider_models.len(), 1);
        assert_eq!(snapshot.provider_models[0].aggregate.request_count, 1);
        assert_eq!(snapshot.activity.daily.len(), 365);
        assert_eq!(
            snapshot.activity.daily.first().unwrap().local_date,
            Date::from_calendar_date(2023, Month::March, 2).unwrap()
        );
        assert_eq!(
            snapshot.activity.daily.last().unwrap().local_date,
            Date::from_calendar_date(2024, Month::February, 29).unwrap()
        );
        assert_eq!(snapshot.activity.summary, snapshot.selected_summary);
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
        .usage_analytics(all_time_query(finite_range(2026, 8, 1, 4, UtcOffset::UTC)))
        .unwrap();
    assert_eq!(snapshot.selected_summary.request_count, 4);
    assert_eq!(snapshot.selected_summary.reported_request_count, 3);
    assert_eq!(snapshot.selected_summary.unreported_request_count, 1);
    assert_eq!(snapshot.selected_summary.total_covered_request_count, 1);
    assert_eq!(snapshot.selected_summary.partial_request_count(), Some(2));
    assert_eq!(snapshot.selected_summary.input_tokens, 7);
    assert_eq!(snapshot.selected_summary.cache_write_input_tokens, 11);
    assert_eq!(snapshot.selected_summary.total_tokens, 13);
    assert_eq!(snapshot.provider_models.len(), 1);
    assert_eq!(
        snapshot.provider_models[0].aggregate,
        snapshot.selected_summary
    );
    assert_eq!(snapshot.activity.summary, snapshot.selected_summary);
    assert_eq!(snapshot.activity.daily.len(), 3);
}

#[test]
fn usage_analytics_sums_estimated_cost_and_counts_known_zero_as_priced() {
    let (_dir, store) = fresh_store();
    for (id, provider_id, model_id, total) in [
        ("priced", "provider-a", "model-a", 3),
        ("unknown", "provider-a", "model-a", 2),
        ("free", "provider-b", "model-b", 1),
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
    set_usage_cost(&store, "priced", 125).unwrap();
    set_usage_cost(&store, "free", 0).unwrap();

    let snapshot = store
        .repository()
        .usage_analytics(all_time_query(finite_range(2026, 8, 1, 4, UtcOffset::UTC)))
        .unwrap();
    assert_eq!(snapshot.selected_summary.request_count, 3);
    assert_eq!(snapshot.selected_summary.priced_request_count, 2);
    assert_eq!(snapshot.selected_summary.estimated_cost_nano_usd, 125);
    assert_eq!(
        snapshot.selected_cost_daily,
        vec![crate::UsageAnalyticsCostDailyBucket {
            local_date: Date::from_calendar_date(2026, Month::August, 2).unwrap(),
            priced_request_count: 2,
            estimated_cost_nano_usd: 125,
        }]
    );
    assert_eq!(snapshot.activity.summary.priced_request_count, 0);
    assert_eq!(snapshot.activity.summary.estimated_cost_nano_usd, 0);
    assert!(snapshot.activity.daily.iter().all(|bucket| {
        bucket.aggregate.priced_request_count == 0 && bucket.aggregate.estimated_cost_nano_usd == 0
    }));

    let provider_a = snapshot
        .provider_models
        .iter()
        .find(|bucket| bucket.provider_id == "provider-a")
        .unwrap();
    assert_eq!(provider_a.aggregate.request_count, 2);
    assert_eq!(provider_a.aggregate.priced_request_count, 1);
    assert_eq!(provider_a.aggregate.estimated_cost_nano_usd, 125);
    let provider_b = snapshot
        .provider_models
        .iter()
        .find(|bucket| bucket.provider_id == "provider-b")
        .unwrap();
    assert_eq!(provider_b.aggregate.request_count, 1);
    assert_eq!(provider_b.aggregate.priced_request_count, 1);
    assert_eq!(provider_b.aggregate.estimated_cost_nano_usd, 0);

    assert!(set_usage_cost(&store, "unknown", -1).is_err());
}

#[test]
fn usage_analytics_cost_daily_is_sparse_and_uses_captured_fixed_offset() {
    let (_dir, store) = fresh_store();
    for (id, created_at, total) in [
        (
            "local-day-one",
            datetime(2026, 8, 1, 0, 0, UtcOffset::UTC),
            1,
        ),
        (
            "local-day-two-priced",
            datetime(2026, 8, 1, 23, 30, UtcOffset::UTC),
            2,
        ),
        (
            "local-day-two-free",
            datetime(2026, 8, 2, 15, 30, UtcOffset::UTC),
            3,
        ),
        (
            "local-day-three-unpriced",
            datetime(2026, 8, 2, 15, 45, UtcOffset::UTC),
            4,
        ),
        (
            "outside-selected",
            datetime(2026, 8, 3, 12, 0, UtcOffset::UTC),
            5,
        ),
    ] {
        insert_usage(
            &store,
            id,
            "provider",
            "model",
            "ignored",
            created_at,
            TokenCounts {
                total,
                ..TokenCounts::ZERO
            },
        );
    }
    set_usage_cost(&store, "local-day-one", 50).unwrap();
    set_usage_cost(&store, "local-day-two-priced", 100).unwrap();
    set_usage_cost(&store, "local-day-two-free", 0).unwrap();
    set_usage_cost(&store, "outside-selected", 400).unwrap();

    let captured_offset = UtcOffset::from_hms(8, 0, 0).unwrap();
    let selected_range = finite_range(2026, 8, 1, 3, captured_offset);
    let activity_range = finite_range(2026, 8, 1, 4, captured_offset);
    let finite = store
        .repository()
        .usage_analytics(UsageAnalyticsQuery {
            selected_range: UsageAnalyticsRange::Finite(selected_range),
            activity_range,
        })
        .unwrap();
    assert_eq!(finite.selected_summary.request_count, 4);
    assert_eq!(finite.selected_summary.priced_request_count, 3);
    assert_eq!(finite.selected_summary.estimated_cost_nano_usd, 150);
    assert_eq!(
        finite.selected_cost_daily,
        vec![
            crate::UsageAnalyticsCostDailyBucket {
                local_date: Date::from_calendar_date(2026, Month::August, 1).unwrap(),
                priced_request_count: 1,
                estimated_cost_nano_usd: 50,
            },
            crate::UsageAnalyticsCostDailyBucket {
                local_date: Date::from_calendar_date(2026, Month::August, 2).unwrap(),
                priced_request_count: 2,
                estimated_cost_nano_usd: 100,
            },
        ]
    );

    let all_time = store
        .repository()
        .usage_analytics(all_time_query(activity_range))
        .unwrap();
    assert_eq!(all_time.selected_summary.request_count, 5);
    assert_eq!(all_time.selected_summary.priced_request_count, 4);
    assert_eq!(all_time.selected_summary.estimated_cost_nano_usd, 550);
    assert_eq!(
        all_time
            .selected_cost_daily
            .iter()
            .map(|bucket| (
                bucket.local_date,
                bucket.priced_request_count,
                bucket.estimated_cost_nano_usd
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                Date::from_calendar_date(2026, Month::August, 1).unwrap(),
                1,
                50
            ),
            (
                Date::from_calendar_date(2026, Month::August, 2).unwrap(),
                2,
                100
            ),
            (
                Date::from_calendar_date(2026, Month::August, 3).unwrap(),
                1,
                400
            ),
        ]
    );
}

#[test]
fn usage_analytics_cost_daily_keeps_dst_transition_offsets_fixed() {
    let (_dir, store) = fresh_store();
    for (id, created_at, cost) in [
        (
            "dst-before-fallback",
            datetime(2026, 11, 1, 7, 30, UtcOffset::UTC),
            1,
        ),
        (
            "dst-after-fallback",
            datetime(2026, 11, 1, 8, 30, UtcOffset::UTC),
            2,
        ),
    ] {
        insert_usage(
            &store,
            id,
            "provider",
            "model",
            "ignored",
            created_at,
            TokenCounts {
                total: 1,
                ..TokenCounts::ZERO
            },
        );
        set_usage_cost(&store, id, cost).unwrap();
    }

    let summer_offset = UtcOffset::from_hms(-7, 0, 0).unwrap();
    let summer_selected_range = UsageAnalyticsFiniteRange::new(
        datetime(2026, 10, 30, 0, 0, summer_offset),
        datetime(2026, 11, 3, 0, 0, summer_offset),
        summer_offset,
    )
    .unwrap();
    let summer_activity_range = UsageAnalyticsFiniteRange::new(
        datetime(2026, 10, 31, 0, 0, summer_offset),
        datetime(2026, 11, 2, 0, 0, summer_offset),
        summer_offset,
    )
    .unwrap();
    let summer = store
        .repository()
        .usage_analytics(UsageAnalyticsQuery {
            selected_range: UsageAnalyticsRange::Finite(summer_selected_range),
            activity_range: summer_activity_range,
        })
        .unwrap();
    assert_eq!(
        summer.selected_cost_daily,
        vec![crate::UsageAnalyticsCostDailyBucket {
            local_date: Date::from_calendar_date(2026, Month::November, 1).unwrap(),
            priced_request_count: 2,
            estimated_cost_nano_usd: 3,
        }]
    );

    let winter_offset = UtcOffset::from_hms(-8, 0, 0).unwrap();
    let winter_selected_range = UsageAnalyticsFiniteRange::new(
        datetime(2026, 10, 30, 0, 0, winter_offset),
        datetime(2026, 11, 3, 0, 0, winter_offset),
        winter_offset,
    )
    .unwrap();
    let winter_activity_range = UsageAnalyticsFiniteRange::new(
        datetime(2026, 10, 31, 0, 0, winter_offset),
        datetime(2026, 11, 2, 0, 0, winter_offset),
        winter_offset,
    )
    .unwrap();
    let winter = store
        .repository()
        .usage_analytics(UsageAnalyticsQuery {
            selected_range: UsageAnalyticsRange::Finite(winter_selected_range),
            activity_range: winter_activity_range,
        })
        .unwrap();
    assert_eq!(
        winter.selected_cost_daily,
        vec![
            crate::UsageAnalyticsCostDailyBucket {
                local_date: Date::from_calendar_date(2026, Month::October, 31).unwrap(),
                priced_request_count: 1,
                estimated_cost_nano_usd: 1,
            },
            crate::UsageAnalyticsCostDailyBucket {
                local_date: Date::from_calendar_date(2026, Month::November, 1).unwrap(),
                priced_request_count: 1,
                estimated_cost_nano_usd: 2,
            },
        ]
    );
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
                    cost_amount: None,
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
    let snapshot = repo
        .usage_analytics(all_time_query(finite_range(2026, 8, 1, 4, UtcOffset::UTC)))
        .unwrap();
    assert_eq!(snapshot.selected_summary.request_count, 2);
    assert_eq!(snapshot.selected_summary.reported_request_count, 2);
    assert_eq!(snapshot.selected_summary.total_covered_request_count, 2);
    assert_eq!(snapshot.selected_summary.input_tokens, 20);
    assert_eq!(snapshot.selected_summary.total_tokens, 78);
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
        .usage_analytics(all_time_query(finite_range(2026, 8, 1, 4, UtcOffset::UTC)))
        .unwrap();
    assert_eq!(snapshot.selected_summary.input_tokens, large as u64);
    assert_eq!(snapshot.selected_summary.output_tokens, 2);
    assert_eq!(snapshot.selected_summary.cached_input_tokens, 3);
    assert_eq!(snapshot.selected_summary.cache_write_input_tokens, 5);
    assert_eq!(snapshot.selected_summary.reasoning_tokens, 7);
    assert_eq!(snapshot.selected_summary.total_tokens, (large + 17) as u64);
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
            .usage_analytics(UsageAnalyticsQuery {
                selected_range: UsageAnalyticsRange::Finite(finite_range(
                    2026,
                    7,
                    1,
                    2,
                    UtcOffset::UTC,
                )),
                activity_range: finite_range(2026, 8, 1, 4, UtcOffset::UTC),
            }),
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
            .usage_analytics(UsageAnalyticsQuery {
                selected_range: UsageAnalyticsRange::Finite(finite_range(
                    2026,
                    7,
                    1,
                    2,
                    UtcOffset::UTC,
                )),
                activity_range: finite_range(2026, 8, 1, 4, UtcOffset::UTC),
            }),
        Err(crate::DbError::Diesel(_))
    ));

    let (_dir, cost_overflow_store) = fresh_store();
    for id in ["cost-overflow-a", "cost-overflow-b"] {
        insert_usage(
            &cost_overflow_store,
            id,
            "provider",
            "model",
            "ignored",
            datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
            TokenCounts {
                total: 1,
                ..TokenCounts::ZERO
            },
        );
        set_usage_cost(&cost_overflow_store, id, i64::MAX).unwrap();
    }
    assert!(matches!(
        cost_overflow_store
            .repository()
            .usage_analytics(all_time_query(finite_range(2026, 8, 1, 4, UtcOffset::UTC,))),
        Err(crate::DbError::Diesel(_))
    ));
}

#[test]
fn usage_analytics_returns_selected_provider_models_and_dense_365_day_activity() {
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
    let selected_range = finite_range(2026, 8, 1, 5, UtcOffset::UTC);
    let activity_range = finite_range_for_days(2025, 8, 3, 365, UtcOffset::UTC);
    let finite = store
        .repository()
        .usage_analytics(UsageAnalyticsQuery {
            selected_range: UsageAnalyticsRange::Finite(selected_range),
            activity_range,
        })
        .unwrap();
    assert_eq!(
        finite.selected_range,
        UsageAnalyticsRange::Finite(selected_range)
    );
    assert_eq!(finite.provider_models.len(), 1);
    assert_eq!(finite.provider_models[0].aggregate.total_tokens, 4);
    assert_eq!(finite.activity.range, activity_range);
    assert_eq!(finite.activity.daily.len(), 365);
    assert_eq!(
        finite.activity.daily.first().unwrap().local_date,
        Date::from_calendar_date(2025, Month::August, 3).unwrap()
    );
    assert_eq!(
        finite.activity.daily.last().unwrap().local_date,
        Date::from_calendar_date(2026, Month::August, 2).unwrap()
    );
    assert_eq!(finite.activity.summary, finite.selected_summary);

    let all_time = store
        .repository()
        .usage_analytics(all_time_query(activity_range))
        .unwrap();
    assert_eq!(all_time.selected_summary, finite.selected_summary);
    assert_eq!(all_time.provider_models, finite.provider_models);
    assert_eq!(all_time.activity.daily.len(), 365);
    assert_eq!(all_time.activity, finite.activity);
}

#[test]
fn usage_analytics_keeps_selected_and_activity_totals_independent() {
    let (_dir, store) = fresh_store();
    for (id, created_at, total) in [
        (
            "activity-only",
            datetime(2026, 7, 2, 12, 0, UtcOffset::UTC),
            7,
        ),
        (
            "selected-and-activity",
            datetime(2026, 8, 2, 12, 0, UtcOffset::UTC),
            20,
        ),
        (
            "outside-activity",
            datetime(2026, 8, 5, 12, 0, UtcOffset::UTC),
            30,
        ),
    ] {
        insert_usage(
            &store,
            id,
            "provider",
            "model",
            "ignored",
            created_at,
            TokenCounts {
                total,
                ..TokenCounts::ZERO
            },
        );
    }

    let selected_range = finite_range(2026, 8, 1, 4, UtcOffset::UTC);
    let activity_range = finite_range_for_days(2026, 7, 1, 34, UtcOffset::UTC);
    let snapshot = store
        .repository()
        .usage_analytics(UsageAnalyticsQuery {
            selected_range: UsageAnalyticsRange::Finite(selected_range),
            activity_range,
        })
        .unwrap();
    assert_eq!(snapshot.selected_summary.request_count, 1);
    assert_eq!(snapshot.selected_summary.total_tokens, 20);
    assert_eq!(snapshot.activity.summary.request_count, 2);
    assert_eq!(snapshot.activity.summary.total_tokens, 27);
    assert_eq!(snapshot.provider_models.len(), 1);
    assert_eq!(snapshot.provider_models[0].aggregate.request_count, 1);
    assert_eq!(snapshot.provider_models[0].aggregate.total_tokens, 20);
    assert_eq!(snapshot.activity.daily.len(), 34);
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
        .usage_analytics(all_time_query(finite_range(2026, 8, 1, 4, UtcOffset::UTC)))
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
    assert_eq!(snapshot.provider_models[2].model_label, None);
}

#[test]
fn usage_analytics_empty_snapshot_preserves_requested_shape() {
    let (_dir, store) = fresh_store();
    let selected_range = finite_range(2026, 8, 1, 4, UtcOffset::UTC);
    let activity_range = finite_range_for_days(2025, 8, 3, 365, UtcOffset::UTC);
    let snapshot = store
        .repository()
        .usage_analytics(UsageAnalyticsQuery {
            selected_range: UsageAnalyticsRange::Finite(selected_range),
            activity_range,
        })
        .unwrap();
    assert_eq!(
        snapshot.selected_range,
        UsageAnalyticsRange::Finite(selected_range)
    );
    assert!(snapshot.selected_summary.is_empty());
    assert_eq!(snapshot.selected_summary.partial_request_count(), Some(0));
    assert!(snapshot.provider_models.is_empty());
    assert!(snapshot.selected_cost_daily.is_empty());
    assert_eq!(snapshot.activity.range, activity_range);
    assert!(snapshot.activity.summary.is_empty());
    assert_eq!(snapshot.activity.daily.len(), 365);
    assert!(
        snapshot
            .activity
            .daily
            .iter()
            .all(|bucket| bucket.aggregate == UsageAnalyticsAggregate::default())
    );
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

    let cost_daily_plan = sql_query(format!(
        "EXPLAIN QUERY PLAN {}",
        crate::repository::COST_DAILY_FINITE_SQL
    ))
    .bind::<TimestamptzSqlite, _>(start)
    .bind::<TimestamptzSqlite, _>(end)
    .bind::<diesel::sql_types::Integer, _>(0)
    .load::<QueryPlanRow>(&mut conn)
    .unwrap();
    assert_plan_uses_created_at_index(&cost_daily_plan, "cost daily");

    let provider_model_plan = sql_query(format!(
        "EXPLAIN QUERY PLAN {}",
        crate::repository::PROVIDER_MODELS_FINITE_SQL
    ))
    .bind::<TimestamptzSqlite, _>(start)
    .bind::<TimestamptzSqlite, _>(end)
    .load::<QueryPlanRow>(&mut conn)
    .unwrap();
    assert_plan_uses_created_at_index(&provider_model_plan, "provider/model");
    assert!(
        crate::repository::PROVIDER_MODELS_FINITE_SQL
            .contains("LEFT JOIN providers ON providers.id = aggregates.provider_id")
    );
    assert!(crate::repository::PROVIDER_MODELS_FINITE_SQL.contains(
        "LEFT JOIN provider_models ON\n    provider_models.provider_id = aggregates.provider_id"
    ));
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
