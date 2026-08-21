use super::*;
use diesel::{
    Connection, QueryableByName,
    sql_types::{BigInt, Nullable, Text, TimestamptzSqlite},
};
use time::{Date, OffsetDateTime};

const SUMMARY_ALL_TIME_SQL: &str = r#"
SELECT
    COUNT(*) AS request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens != 0 OR output_tokens != 0 OR cached_input_tokens != 0 OR
        cache_write_input_tokens != 0 OR reasoning_tokens != 0 OR total_tokens != 0
        THEN 1 ELSE 0 END), 0) AS reported_request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens = 0 AND output_tokens = 0 AND cached_input_tokens = 0 AND
        cache_write_input_tokens = 0 AND reasoning_tokens = 0 AND total_tokens = 0
        THEN 1 ELSE 0 END), 0) AS unreported_request_count,
    COALESCE(SUM(CASE WHEN total_tokens > 0 THEN 1 ELSE 0 END), 0)
        AS total_covered_request_count,
    COALESCE(SUM(input_tokens), 0) AS input_tokens,
    COALESCE(SUM(output_tokens), 0) AS output_tokens,
    COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
    COALESCE(SUM(cache_write_input_tokens), 0) AS cache_write_input_tokens,
    COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
    COALESCE(SUM(total_tokens), 0) AS total_tokens,
    COALESCE(SUM(CASE WHEN cost_amount_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0)
        AS priced_request_count,
    COALESCE(SUM(cost_amount_nano_usd), 0) AS estimated_cost_nano_usd,
    COALESCE(SUM(CASE WHEN
        input_tokens < 0 OR output_tokens < 0 OR cached_input_tokens < 0 OR
        cache_write_input_tokens < 0 OR reasoning_tokens < 0 OR total_tokens < 0
        THEN 1 ELSE 0 END), 0) AS invalid_request_count,
    COALESCE(SUM(CASE WHEN cost_amount_nano_usd < 0 THEN 1 ELSE 0 END), 0)
        AS invalid_cost_count
FROM usage_events
"#;

pub(crate) const SUMMARY_FINITE_SQL: &str = r#"
SELECT
    COUNT(*) AS request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens != 0 OR output_tokens != 0 OR cached_input_tokens != 0 OR
        cache_write_input_tokens != 0 OR reasoning_tokens != 0 OR total_tokens != 0
        THEN 1 ELSE 0 END), 0) AS reported_request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens = 0 AND output_tokens = 0 AND cached_input_tokens = 0 AND
        cache_write_input_tokens = 0 AND reasoning_tokens = 0 AND total_tokens = 0
        THEN 1 ELSE 0 END), 0) AS unreported_request_count,
    COALESCE(SUM(CASE WHEN total_tokens > 0 THEN 1 ELSE 0 END), 0)
        AS total_covered_request_count,
    COALESCE(SUM(input_tokens), 0) AS input_tokens,
    COALESCE(SUM(output_tokens), 0) AS output_tokens,
    COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
    COALESCE(SUM(cache_write_input_tokens), 0) AS cache_write_input_tokens,
    COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
    COALESCE(SUM(total_tokens), 0) AS total_tokens,
    COALESCE(SUM(CASE WHEN cost_amount_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0)
        AS priced_request_count,
    COALESCE(SUM(cost_amount_nano_usd), 0) AS estimated_cost_nano_usd,
    COALESCE(SUM(CASE WHEN
        input_tokens < 0 OR output_tokens < 0 OR cached_input_tokens < 0 OR
        cache_write_input_tokens < 0 OR reasoning_tokens < 0 OR total_tokens < 0
        THEN 1 ELSE 0 END), 0) AS invalid_request_count,
    COALESCE(SUM(CASE WHEN cost_amount_nano_usd < 0 THEN 1 ELSE 0 END), 0)
        AS invalid_cost_count
FROM usage_events
WHERE created_at >= ?1 AND created_at < ?2
"#;

const ACTIVITY_SUMMARY_FINITE_SQL: &str = r#"
SELECT
    COUNT(*) AS request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens != 0 OR output_tokens != 0 OR cached_input_tokens != 0 OR
        cache_write_input_tokens != 0 OR reasoning_tokens != 0 OR total_tokens != 0
        THEN 1 ELSE 0 END), 0) AS reported_request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens = 0 AND output_tokens = 0 AND cached_input_tokens = 0 AND
        cache_write_input_tokens = 0 AND reasoning_tokens = 0 AND total_tokens = 0
        THEN 1 ELSE 0 END), 0) AS unreported_request_count,
    COALESCE(SUM(CASE WHEN total_tokens > 0 THEN 1 ELSE 0 END), 0)
        AS total_covered_request_count,
    COALESCE(SUM(input_tokens), 0) AS input_tokens,
    COALESCE(SUM(output_tokens), 0) AS output_tokens,
    COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
    COALESCE(SUM(cache_write_input_tokens), 0) AS cache_write_input_tokens,
    COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
    COALESCE(SUM(total_tokens), 0) AS total_tokens,
    0 AS priced_request_count,
    0 AS estimated_cost_nano_usd,
    COALESCE(SUM(CASE WHEN
        input_tokens < 0 OR output_tokens < 0 OR cached_input_tokens < 0 OR
        cache_write_input_tokens < 0 OR reasoning_tokens < 0 OR total_tokens < 0
        THEN 1 ELSE 0 END), 0) AS invalid_request_count,
    0 AS invalid_cost_count
FROM usage_events
WHERE created_at >= ?1 AND created_at < ?2
"#;

pub(crate) const DAILY_FINITE_SQL: &str = r#"
SELECT
    jaco_local_date(created_at) AS local_date,
    COUNT(*) AS request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens != 0 OR output_tokens != 0 OR cached_input_tokens != 0 OR
        cache_write_input_tokens != 0 OR reasoning_tokens != 0 OR total_tokens != 0
        THEN 1 ELSE 0 END), 0) AS reported_request_count,
    COALESCE(SUM(CASE WHEN
        input_tokens = 0 AND output_tokens = 0 AND cached_input_tokens = 0 AND
        cache_write_input_tokens = 0 AND reasoning_tokens = 0 AND total_tokens = 0
        THEN 1 ELSE 0 END), 0) AS unreported_request_count,
    COALESCE(SUM(CASE WHEN total_tokens > 0 THEN 1 ELSE 0 END), 0)
        AS total_covered_request_count,
    COALESCE(SUM(input_tokens), 0) AS input_tokens,
    COALESCE(SUM(output_tokens), 0) AS output_tokens,
    COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
    COALESCE(SUM(cache_write_input_tokens), 0) AS cache_write_input_tokens,
    COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
    COALESCE(SUM(total_tokens), 0) AS total_tokens,
    0 AS priced_request_count,
    0 AS estimated_cost_nano_usd,
    COALESCE(SUM(CASE WHEN
        input_tokens < 0 OR output_tokens < 0 OR cached_input_tokens < 0 OR
        cache_write_input_tokens < 0 OR reasoning_tokens < 0 OR total_tokens < 0
        THEN 1 ELSE 0 END), 0) AS invalid_request_count,
    0 AS invalid_cost_count
FROM usage_events
WHERE created_at >= ?1 AND created_at < ?2
GROUP BY local_date
ORDER BY local_date ASC
"#;

pub(crate) const COST_DAILY_FINITE_SQL: &str = r#"
SELECT
    jaco_local_date(created_at) AS local_date,
    COUNT(*) AS priced_request_count,
    COALESCE(SUM(cost_amount_nano_usd), 0) AS estimated_cost_nano_usd
FROM usage_events
WHERE created_at >= ?1 AND created_at < ?2
  AND cost_amount_nano_usd IS NOT NULL
GROUP BY local_date
ORDER BY local_date ASC
"#;

const COST_DAILY_ALL_TIME_SQL: &str = r#"
SELECT
    jaco_local_date(created_at) AS local_date,
    COUNT(*) AS priced_request_count,
    COALESCE(SUM(cost_amount_nano_usd), 0) AS estimated_cost_nano_usd
FROM usage_events
WHERE cost_amount_nano_usd IS NOT NULL
GROUP BY local_date
ORDER BY local_date ASC
"#;

const PROVIDER_MODELS_ALL_TIME_SQL: &str = r#"
WITH aggregates AS (
    SELECT
        provider_id,
        model_id,
        COUNT(*) AS request_count,
        COALESCE(SUM(CASE WHEN
            input_tokens != 0 OR output_tokens != 0 OR cached_input_tokens != 0 OR
            cache_write_input_tokens != 0 OR reasoning_tokens != 0 OR total_tokens != 0
            THEN 1 ELSE 0 END), 0) AS reported_request_count,
        COALESCE(SUM(CASE WHEN
            input_tokens = 0 AND output_tokens = 0 AND cached_input_tokens = 0 AND
            cache_write_input_tokens = 0 AND reasoning_tokens = 0 AND total_tokens = 0
            THEN 1 ELSE 0 END), 0) AS unreported_request_count,
        COALESCE(SUM(CASE WHEN total_tokens > 0 THEN 1 ELSE 0 END), 0)
            AS total_covered_request_count,
        COALESCE(SUM(input_tokens), 0) AS input_tokens,
        COALESCE(SUM(output_tokens), 0) AS output_tokens,
        COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
        COALESCE(SUM(cache_write_input_tokens), 0) AS cache_write_input_tokens,
        COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
        COALESCE(SUM(total_tokens), 0) AS total_tokens,
        COALESCE(SUM(CASE WHEN cost_amount_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0)
            AS priced_request_count,
        COALESCE(SUM(cost_amount_nano_usd), 0) AS estimated_cost_nano_usd,
        COALESCE(SUM(CASE WHEN
            input_tokens < 0 OR output_tokens < 0 OR cached_input_tokens < 0 OR
            cache_write_input_tokens < 0 OR reasoning_tokens < 0 OR total_tokens < 0
            THEN 1 ELSE 0 END), 0) AS invalid_request_count,
        COALESCE(SUM(CASE WHEN cost_amount_nano_usd < 0 THEN 1 ELSE 0 END), 0)
            AS invalid_cost_count
    FROM usage_events
    GROUP BY provider_id, model_id
)
SELECT
    aggregates.provider_id,
    aggregates.model_id,
    providers.display_name AS provider_label,
    provider_models.display_name AS model_label,
    aggregates.request_count,
    aggregates.reported_request_count,
    aggregates.unreported_request_count,
    aggregates.total_covered_request_count,
    aggregates.input_tokens,
    aggregates.output_tokens,
    aggregates.cached_input_tokens,
    aggregates.cache_write_input_tokens,
    aggregates.reasoning_tokens,
    aggregates.total_tokens,
    aggregates.priced_request_count,
    aggregates.estimated_cost_nano_usd,
    aggregates.invalid_request_count,
    aggregates.invalid_cost_count
FROM aggregates
LEFT JOIN providers ON providers.id = aggregates.provider_id
LEFT JOIN provider_models ON
    provider_models.provider_id = aggregates.provider_id AND
    provider_models.model_id = aggregates.model_id
ORDER BY aggregates.total_tokens DESC, aggregates.provider_id ASC, aggregates.model_id ASC
"#;

pub(crate) const PROVIDER_MODELS_FINITE_SQL: &str = r#"
WITH aggregates AS (
    SELECT
        provider_id,
        model_id,
        COUNT(*) AS request_count,
        COALESCE(SUM(CASE WHEN
            input_tokens != 0 OR output_tokens != 0 OR cached_input_tokens != 0 OR
            cache_write_input_tokens != 0 OR reasoning_tokens != 0 OR total_tokens != 0
            THEN 1 ELSE 0 END), 0) AS reported_request_count,
        COALESCE(SUM(CASE WHEN
            input_tokens = 0 AND output_tokens = 0 AND cached_input_tokens = 0 AND
            cache_write_input_tokens = 0 AND reasoning_tokens = 0 AND total_tokens = 0
            THEN 1 ELSE 0 END), 0) AS unreported_request_count,
        COALESCE(SUM(CASE WHEN total_tokens > 0 THEN 1 ELSE 0 END), 0)
            AS total_covered_request_count,
        COALESCE(SUM(input_tokens), 0) AS input_tokens,
        COALESCE(SUM(output_tokens), 0) AS output_tokens,
        COALESCE(SUM(cached_input_tokens), 0) AS cached_input_tokens,
        COALESCE(SUM(cache_write_input_tokens), 0) AS cache_write_input_tokens,
        COALESCE(SUM(reasoning_tokens), 0) AS reasoning_tokens,
        COALESCE(SUM(total_tokens), 0) AS total_tokens,
        COALESCE(SUM(CASE WHEN cost_amount_nano_usd IS NOT NULL THEN 1 ELSE 0 END), 0)
            AS priced_request_count,
        COALESCE(SUM(cost_amount_nano_usd), 0) AS estimated_cost_nano_usd,
        COALESCE(SUM(CASE WHEN
            input_tokens < 0 OR output_tokens < 0 OR cached_input_tokens < 0 OR
            cache_write_input_tokens < 0 OR reasoning_tokens < 0 OR total_tokens < 0
            THEN 1 ELSE 0 END), 0) AS invalid_request_count,
        COALESCE(SUM(CASE WHEN cost_amount_nano_usd < 0 THEN 1 ELSE 0 END), 0)
            AS invalid_cost_count
    FROM usage_events
    WHERE created_at >= ?1 AND created_at < ?2
    GROUP BY provider_id, model_id
)
SELECT
    aggregates.provider_id,
    aggregates.model_id,
    providers.display_name AS provider_label,
    provider_models.display_name AS model_label,
    aggregates.request_count,
    aggregates.reported_request_count,
    aggregates.unreported_request_count,
    aggregates.total_covered_request_count,
    aggregates.input_tokens,
    aggregates.output_tokens,
    aggregates.cached_input_tokens,
    aggregates.cache_write_input_tokens,
    aggregates.reasoning_tokens,
    aggregates.total_tokens,
    aggregates.priced_request_count,
    aggregates.estimated_cost_nano_usd,
    aggregates.invalid_request_count,
    aggregates.invalid_cost_count
FROM aggregates
LEFT JOIN providers ON providers.id = aggregates.provider_id
LEFT JOIN provider_models ON
    provider_models.provider_id = aggregates.provider_id AND
    provider_models.model_id = aggregates.model_id
ORDER BY aggregates.total_tokens DESC, aggregates.provider_id ASC, aggregates.model_id ASC
"#;

#[derive(Debug, QueryableByName)]
struct AggregateSqlRow {
    #[diesel(sql_type = BigInt)]
    request_count: i64,
    #[diesel(sql_type = BigInt)]
    reported_request_count: i64,
    #[diesel(sql_type = BigInt)]
    unreported_request_count: i64,
    #[diesel(sql_type = BigInt)]
    total_covered_request_count: i64,
    #[diesel(sql_type = BigInt)]
    input_tokens: i64,
    #[diesel(sql_type = BigInt)]
    output_tokens: i64,
    #[diesel(sql_type = BigInt)]
    cached_input_tokens: i64,
    #[diesel(sql_type = BigInt)]
    cache_write_input_tokens: i64,
    #[diesel(sql_type = BigInt)]
    reasoning_tokens: i64,
    #[diesel(sql_type = BigInt)]
    total_tokens: i64,
    #[diesel(sql_type = BigInt)]
    priced_request_count: i64,
    #[diesel(sql_type = BigInt)]
    estimated_cost_nano_usd: i64,
    #[diesel(sql_type = BigInt)]
    invalid_request_count: i64,
    #[diesel(sql_type = BigInt)]
    invalid_cost_count: i64,
}

#[derive(Debug, QueryableByName)]
struct DailyAggregateSqlRow {
    #[diesel(sql_type = Nullable<Text>)]
    local_date: Option<String>,
    #[diesel(embed)]
    aggregate: AggregateSqlRow,
}

#[derive(Debug, QueryableByName)]
struct CostDailySqlRow {
    #[diesel(sql_type = Nullable<Text>)]
    local_date: Option<String>,
    #[diesel(sql_type = BigInt)]
    priced_request_count: i64,
    #[diesel(sql_type = BigInt)]
    estimated_cost_nano_usd: i64,
}

#[derive(Debug, QueryableByName)]
struct ProviderModelAggregateSqlRow {
    #[diesel(sql_type = Text)]
    provider_id: String,
    #[diesel(sql_type = Text)]
    model_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    provider_label: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    model_label: Option<String>,
    #[diesel(embed)]
    aggregate: AggregateSqlRow,
}

impl FreshRepository {
    pub fn usage_analytics(&self, query: UsageAnalyticsQuery) -> Result<UsageAnalyticsSnapshot> {
        let mut conn = self.conn()?;
        conn.transaction(|conn| usage_analytics_with_conn(conn, query))
    }
}

fn usage_analytics_with_conn(
    conn: &mut SqliteConnection,
    query: UsageAnalyticsQuery,
) -> Result<UsageAnalyticsSnapshot> {
    let time_zone = query.activity_range.time_zone();
    if let UsageAnalyticsRange::Finite(selected_range) = query.selected_range
        && selected_range.time_zone() != time_zone
    {
        return Err(DbError::Invariant(
            "usage analytics ranges use different local time zones".to_string(),
        ));
    }
    register_local_date_function(conn, time_zone)?;

    let selected_summary = load_summary(conn, query.selected_range)?;
    validate_aggregate(&selected_summary, "selected summary")?;
    let selected_cost_daily = load_cost_daily(conn, query.selected_range)?;
    validate_cost_daily_totals(&selected_cost_daily, &selected_summary)?;
    let provider_models = load_provider_models(conn, query.selected_range)?;
    validate_bucket_totals(
        provider_models.iter().map(|bucket| &bucket.aggregate),
        &selected_summary,
        "provider/model",
    )?;

    let activity_summary = load_activity_summary(conn, query.activity_range)?;
    validate_aggregate(&activity_summary, "activity summary")?;
    let activity_daily = load_daily(conn, query.activity_range)?;
    validate_daily_shape(&activity_daily, query.activity_range, "activity daily")?;
    validate_bucket_totals(
        activity_daily.iter().map(|bucket| &bucket.aggregate),
        &activity_summary,
        "activity daily",
    )?;

    Ok(UsageAnalyticsSnapshot {
        selected_range: query.selected_range,
        selected_summary,
        selected_cost_daily,
        provider_models,
        activity: UsageAnalyticsActivity {
            range: query.activity_range,
            summary: activity_summary,
            daily: activity_daily,
        },
    })
}

pub(crate) fn register_local_date_function(
    conn: &mut SqliteConnection,
    time_zone: UsageAnalyticsTimeZone,
) -> Result<()> {
    conn.register_sql_function::<
        TimestamptzSqlite,
        Nullable<Text>,
        OffsetDateTime,
        Option<String>,
        _,
    >("jaco_local_date", false, move |created_at| {
        time_zone
            .local_date_at(created_at)
            .map(|date| date.to_string())
    })?;
    Ok(())
}

fn load_summary(
    conn: &mut SqliteConnection,
    range: UsageAnalyticsRange,
) -> Result<UsageAnalyticsAggregate> {
    let row = match range {
        UsageAnalyticsRange::Finite(range) => sql_query(SUMMARY_FINITE_SQL)
            .bind::<TimestamptzSqlite, _>(range.start_utc())
            .bind::<TimestamptzSqlite, _>(range.end_utc())
            .get_result::<AggregateSqlRow>(conn)?,
        UsageAnalyticsRange::AllTime => {
            sql_query(SUMMARY_ALL_TIME_SQL).get_result::<AggregateSqlRow>(conn)?
        }
    };
    row.try_into()
}

fn load_activity_summary(
    conn: &mut SqliteConnection,
    range: UsageAnalyticsFiniteRange,
) -> Result<UsageAnalyticsAggregate> {
    sql_query(ACTIVITY_SUMMARY_FINITE_SQL)
        .bind::<TimestamptzSqlite, _>(range.start_utc())
        .bind::<TimestamptzSqlite, _>(range.end_utc())
        .get_result::<AggregateSqlRow>(conn)?
        .try_into()
}

fn load_daily(
    conn: &mut SqliteConnection,
    range: UsageAnalyticsFiniteRange,
) -> Result<Vec<UsageAnalyticsDailyBucket>> {
    let format = time::format_description::parse_borrowed::<3>("[year]-[month]-[day]")
        .map_err(|error| DbError::Invariant(format!("daily date format is invalid: {error}")))?;
    let rows = sql_query(DAILY_FINITE_SQL)
        .bind::<TimestamptzSqlite, _>(range.start_utc())
        .bind::<TimestamptzSqlite, _>(range.end_utc())
        .load::<DailyAggregateSqlRow>(conn)?;

    let mut queried = std::collections::BTreeMap::new();
    for row in rows {
        let local_date = row.local_date.ok_or_else(|| {
            DbError::Invariant(
                "usage event timestamp could not be grouped by local date".to_string(),
            )
        })?;
        let local_date = Date::parse(&local_date, &format)?;
        let aggregate: UsageAnalyticsAggregate = row.aggregate.try_into()?;
        validate_aggregate(&aggregate, "daily bucket")?;
        if queried.insert(local_date, aggregate).is_some() {
            return Err(DbError::Invariant(format!(
                "duplicate daily analytics bucket for {local_date}"
            )));
        }
    }

    let start_date = range.start_date();
    let end_date = range.end_date();
    let mut local_date = start_date;
    let mut daily = Vec::new();
    while local_date < end_date {
        daily.push(UsageAnalyticsDailyBucket {
            local_date,
            aggregate: queried.remove(&local_date).unwrap_or_default(),
        });
        local_date = local_date.next_day().ok_or_else(|| {
            DbError::Invariant("finite analytics date range exceeds supported dates".to_string())
        })?;
    }
    if let Some((date, _)) = queried.first_key_value() {
        return Err(DbError::Invariant(format!(
            "daily analytics bucket {date} falls outside requested range"
        )));
    }
    Ok(daily)
}

fn load_cost_daily(
    conn: &mut SqliteConnection,
    range: UsageAnalyticsRange,
) -> Result<Vec<UsageAnalyticsCostDailyBucket>> {
    let rows = match range {
        UsageAnalyticsRange::Finite(range) => sql_query(COST_DAILY_FINITE_SQL)
            .bind::<TimestamptzSqlite, _>(range.start_utc())
            .bind::<TimestamptzSqlite, _>(range.end_utc())
            .load::<CostDailySqlRow>(conn)?,
        UsageAnalyticsRange::AllTime => {
            sql_query(COST_DAILY_ALL_TIME_SQL).load::<CostDailySqlRow>(conn)?
        }
    };

    let format =
        time::format_description::parse_borrowed::<3>("[year]-[month]-[day]").map_err(|error| {
            DbError::Invariant(format!("cost daily date format is invalid: {error}"))
        })?;
    let mut daily = Vec::with_capacity(rows.len());
    for row in rows {
        let local_date = row.local_date.ok_or_else(|| {
            DbError::Invariant(
                "priced usage event timestamp could not be grouped by local date".to_string(),
            )
        })?;
        let local_date = Date::parse(&local_date, &format)?;
        if let Some(previous_date) = daily
            .last()
            .map(|bucket: &UsageAnalyticsCostDailyBucket| bucket.local_date)
            && previous_date >= local_date
        {
            return Err(DbError::Invariant(format!(
                "cost daily analytics rows are not strictly ordered at {local_date}"
            )));
        }
        daily.push(UsageAnalyticsCostDailyBucket {
            local_date,
            priced_request_count: checked_u64(row.priced_request_count, "priced request count")?,
            estimated_cost_nano_usd: checked_u64(
                row.estimated_cost_nano_usd,
                "estimated cost nano USD",
            )?,
        });
    }
    Ok(daily)
}

fn load_provider_models(
    conn: &mut SqliteConnection,
    range: UsageAnalyticsRange,
) -> Result<Vec<UsageAnalyticsProviderModelBucket>> {
    let rows = match range {
        UsageAnalyticsRange::Finite(range) => sql_query(PROVIDER_MODELS_FINITE_SQL)
            .bind::<TimestamptzSqlite, _>(range.start_utc())
            .bind::<TimestamptzSqlite, _>(range.end_utc())
            .load::<ProviderModelAggregateSqlRow>(conn)?,
        UsageAnalyticsRange::AllTime => {
            sql_query(PROVIDER_MODELS_ALL_TIME_SQL).load::<ProviderModelAggregateSqlRow>(conn)?
        }
    };

    rows.into_iter()
        .map(|row| {
            let aggregate = row.aggregate.try_into()?;
            validate_aggregate(&aggregate, "provider/model bucket")?;
            Ok(UsageAnalyticsProviderModelBucket {
                provider_id: row.provider_id,
                model_id: row.model_id,
                provider_label: row.provider_label,
                model_label: row.model_label,
                aggregate,
            })
        })
        .collect()
}

impl TryFrom<AggregateSqlRow> for UsageAnalyticsAggregate {
    type Error = DbError;

    fn try_from(row: AggregateSqlRow) -> Result<Self> {
        if row.invalid_request_count != 0 || row.invalid_cost_count != 0 {
            return Err(DbError::Invariant(
                "usage analytics encountered a negative token or cost value".to_string(),
            ));
        }
        Ok(Self {
            request_count: checked_u64(row.request_count, "request count")?,
            reported_request_count: checked_u64(
                row.reported_request_count,
                "reported request count",
            )?,
            unreported_request_count: checked_u64(
                row.unreported_request_count,
                "unreported request count",
            )?,
            total_covered_request_count: checked_u64(
                row.total_covered_request_count,
                "total-covered request count",
            )?,
            input_tokens: checked_u64(row.input_tokens, "input tokens")?,
            output_tokens: checked_u64(row.output_tokens, "output tokens")?,
            cached_input_tokens: checked_u64(row.cached_input_tokens, "cached input tokens")?,
            cache_write_input_tokens: checked_u64(
                row.cache_write_input_tokens,
                "cache-write input tokens",
            )?,
            reasoning_tokens: checked_u64(row.reasoning_tokens, "reasoning tokens")?,
            total_tokens: checked_u64(row.total_tokens, "total tokens")?,
            priced_request_count: checked_u64(row.priced_request_count, "priced request count")?,
            estimated_cost_nano_usd: checked_u64(
                row.estimated_cost_nano_usd,
                "estimated cost nano USD",
            )?,
        })
    }
}

fn checked_u64(value: i64, field: &str) -> Result<u64> {
    u64::try_from(value).map_err(|_| {
        DbError::Invariant(format!(
            "usage analytics {field} is outside the supported range"
        ))
    })
}

fn validate_aggregate(aggregate: &UsageAnalyticsAggregate, context: &str) -> Result<()> {
    if aggregate
        .reported_request_count
        .checked_add(aggregate.unreported_request_count)
        != Some(aggregate.request_count)
    {
        return Err(DbError::Invariant(format!(
            "{context} usage analytics coverage does not equal request count"
        )));
    }
    if aggregate.total_covered_request_count > aggregate.reported_request_count {
        return Err(DbError::Invariant(format!(
            "{context} total-covered request count exceeds reported request count"
        )));
    }
    if aggregate.priced_request_count > aggregate.request_count {
        return Err(DbError::Invariant(format!(
            "{context} priced request count exceeds request count"
        )));
    }
    Ok(())
}

fn validate_bucket_totals<'a>(
    buckets: impl IntoIterator<Item = &'a UsageAnalyticsAggregate>,
    summary: &UsageAnalyticsAggregate,
    context: &str,
) -> Result<()> {
    let mut total = UsageAnalyticsAggregate::default();
    for bucket in buckets {
        checked_add_aggregate(&mut total, bucket, context)?;
    }
    if &total != summary {
        return Err(DbError::Invariant(format!(
            "{context} usage analytics totals do not match summary"
        )));
    }
    Ok(())
}

fn validate_cost_daily_totals(
    daily: &[UsageAnalyticsCostDailyBucket],
    summary: &UsageAnalyticsAggregate,
) -> Result<()> {
    let mut priced_request_count = 0_u64;
    let mut estimated_cost_nano_usd = 0_u64;
    for bucket in daily {
        priced_request_count = priced_request_count
            .checked_add(bucket.priced_request_count)
            .ok_or_else(|| {
                DbError::Invariant("cost daily priced request count overflowed".to_string())
            })?;
        estimated_cost_nano_usd = estimated_cost_nano_usd
            .checked_add(bucket.estimated_cost_nano_usd)
            .ok_or_else(|| {
                DbError::Invariant("cost daily estimated cost overflowed".to_string())
            })?;
    }
    if priced_request_count != summary.priced_request_count
        || estimated_cost_nano_usd != summary.estimated_cost_nano_usd
    {
        return Err(DbError::Invariant(
            "cost daily usage analytics totals do not match selected summary".to_string(),
        ));
    }
    Ok(())
}

fn validate_daily_shape(
    daily: &[UsageAnalyticsDailyBucket],
    range: UsageAnalyticsFiniteRange,
    context: &str,
) -> Result<()> {
    let start_date = range.start_date();
    let end_date = range.end_date();

    let mut expected_date = start_date;
    for bucket in daily {
        if bucket.local_date != expected_date {
            return Err(DbError::Invariant(format!(
                "{context} expected {expected_date}, found {}",
                bucket.local_date
            )));
        }
        expected_date = expected_date.next_day().ok_or_else(|| {
            DbError::Invariant(format!("{context} exceeds supported local dates"))
        })?;
    }
    if expected_date != end_date {
        return Err(DbError::Invariant(format!(
            "{context} does not densely cover the requested range"
        )));
    }
    Ok(())
}

fn checked_add_aggregate(
    total: &mut UsageAnalyticsAggregate,
    value: &UsageAnalyticsAggregate,
    context: &str,
) -> Result<()> {
    macro_rules! checked_add_field {
        ($field:ident) => {
            total.$field = total.$field.checked_add(value.$field).ok_or_else(|| {
                DbError::Invariant(format!(
                    "{context} usage analytics {} overflowed",
                    stringify!($field)
                ))
            })?;
        };
    }
    checked_add_field!(request_count);
    checked_add_field!(reported_request_count);
    checked_add_field!(unreported_request_count);
    checked_add_field!(total_covered_request_count);
    checked_add_field!(input_tokens);
    checked_add_field!(output_tokens);
    checked_add_field!(cached_input_tokens);
    checked_add_field!(cache_write_input_tokens);
    checked_add_field!(reasoning_tokens);
    checked_add_field!(total_tokens);
    checked_add_field!(priced_request_count);
    checked_add_field!(estimated_cost_nano_usd);
    Ok(())
}
