use jaco_core::{ProviderId, ProviderModelId};
use time::{OffsetDateTime, Time, UtcOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageAnalyticsFiniteRange {
    start_utc: OffsetDateTime,
    end_utc: OffsetDateTime,
    local_offset: UtcOffset,
}

impl UsageAnalyticsFiniteRange {
    pub fn new(
        start_utc: OffsetDateTime,
        end_utc: OffsetDateTime,
        local_offset: UtcOffset,
    ) -> Option<Self> {
        // `time` can represent larger years when another workspace crate enables its
        // `large-dates` feature, while offset conversion is only defined safely at the
        // standard-range boundary. Keep one year of headroom for either offset direction.
        if !(-9_998..=9_998).contains(&start_utc.year())
            || !(-9_998..=9_998).contains(&end_utc.year())
        {
            return None;
        }
        let start_utc = start_utc.checked_to_offset(UtcOffset::UTC)?;
        let end_utc = end_utc.checked_to_offset(UtcOffset::UTC)?;
        let local_start = start_utc.checked_to_offset(local_offset)?;
        let local_end = end_utc.checked_to_offset(local_offset)?;
        if start_utc >= end_utc
            || local_start.time() != Time::MIDNIGHT
            || local_end.time() != Time::MIDNIGHT
        {
            return None;
        }

        Some(Self {
            start_utc,
            end_utc,
            local_offset,
        })
    }

    pub fn start_utc(&self) -> OffsetDateTime {
        self.start_utc
    }

    pub fn end_utc(&self) -> OffsetDateTime {
        self.end_utc
    }

    pub fn local_offset(&self) -> UtcOffset {
        self.local_offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageAnalyticsRange {
    Finite(UsageAnalyticsFiniteRange),
    AllTime,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageAnalyticsAggregate {
    pub request_count: u64,
    pub reported_request_count: u64,
    pub unreported_request_count: u64,
    pub total_covered_request_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub priced_request_count: u64,
    pub estimated_cost_nano_usd: u64,
}

impl UsageAnalyticsAggregate {
    pub fn is_empty(&self) -> bool {
        self.request_count == 0
    }

    pub fn partial_request_count(&self) -> Option<u64> {
        self.reported_request_count
            .checked_sub(self.total_covered_request_count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsDailyBucket {
    pub local_date: time::Date,
    pub aggregate: UsageAnalyticsAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsCostDailyBucket {
    pub local_date: time::Date,
    pub priced_request_count: u64,
    pub estimated_cost_nano_usd: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsProviderModelBucket {
    pub provider_id: ProviderId,
    pub model_id: ProviderModelId,
    pub provider_label: Option<String>,
    pub model_label: Option<String>,
    pub aggregate: UsageAnalyticsAggregate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsActivity {
    pub range: UsageAnalyticsFiniteRange,
    pub summary: UsageAnalyticsAggregate,
    pub daily: Vec<UsageAnalyticsDailyBucket>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageAnalyticsQuery {
    pub selected_range: UsageAnalyticsRange,
    pub activity_range: UsageAnalyticsFiniteRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageAnalyticsSnapshot {
    pub selected_range: UsageAnalyticsRange,
    pub selected_summary: UsageAnalyticsAggregate,
    pub selected_cost_daily: Vec<UsageAnalyticsCostDailyBucket>,
    pub provider_models: Vec<UsageAnalyticsProviderModelBucket>,
    pub activity: UsageAnalyticsActivity,
}
