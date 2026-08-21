use jaco_core::{ProviderId, ProviderModelId};
use time::{Date, Duration, OffsetDateTime, Time, UtcOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageAnalyticsTimeZone {
    kind: UsageAnalyticsTimeZoneKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UsageAnalyticsTimeZoneKind {
    System,
    Fixed(UtcOffset),
    #[cfg(test)]
    Transition {
        transition_utc: OffsetDateTime,
        before_offset: UtcOffset,
        after_offset: UtcOffset,
    },
}

impl UsageAnalyticsTimeZone {
    pub const fn system() -> Self {
        Self {
            kind: UsageAnalyticsTimeZoneKind::System,
        }
    }

    pub const fn fixed(offset: UtcOffset) -> Self {
        Self {
            kind: UsageAnalyticsTimeZoneKind::Fixed(offset),
        }
    }

    #[cfg(test)]
    pub(crate) fn transition(
        transition_utc: OffsetDateTime,
        before_offset: UtcOffset,
        after_offset: UtcOffset,
    ) -> Self {
        Self {
            kind: UsageAnalyticsTimeZoneKind::Transition {
                transition_utc: transition_utc.to_offset(UtcOffset::UTC),
                before_offset,
                after_offset,
            },
        }
    }

    pub fn local_date_at(self, instant: OffsetDateTime) -> Option<Date> {
        self.local_datetime_at(instant)
            .map(|datetime| datetime.date())
    }

    fn local_datetime_at(self, instant: OffsetDateTime) -> Option<OffsetDateTime> {
        instant.checked_to_offset(self.offset_at(instant)?)
    }

    fn local_midnight_utc(self, date: Date) -> Option<OffsetDateTime> {
        if !(-9_998..=9_998).contains(&date.year()) {
            return None;
        }

        let local_midnight_as_utc = date.midnight().assume_utc();
        let mut candidate = local_midnight_as_utc;
        for _ in 0..4 {
            let offset = self.offset_at(candidate)?;
            let next = local_midnight_as_utc
                .checked_sub(Duration::seconds(i64::from(offset.whole_seconds())))?;
            if next == candidate {
                let local = self.local_datetime_at(candidate)?;
                return (local.date() == date && local.time() == Time::MIDNIGHT)
                    .then_some(candidate);
            }
            candidate = next;
        }

        None
    }

    fn offset_at(self, instant: OffsetDateTime) -> Option<UtcOffset> {
        match self.kind {
            UsageAnalyticsTimeZoneKind::System => UtcOffset::local_offset_at(instant).ok(),
            UsageAnalyticsTimeZoneKind::Fixed(offset) => Some(offset),
            #[cfg(test)]
            UsageAnalyticsTimeZoneKind::Transition {
                transition_utc,
                before_offset,
                after_offset,
            } => Some(if instant < transition_utc {
                before_offset
            } else {
                after_offset
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageAnalyticsFiniteRange {
    start_utc: OffsetDateTime,
    end_utc: OffsetDateTime,
    start_date: Date,
    end_date: Date,
    time_zone: UsageAnalyticsTimeZone,
}

impl UsageAnalyticsFiniteRange {
    pub fn new(
        start_utc: OffsetDateTime,
        end_utc: OffsetDateTime,
        local_offset: UtcOffset,
    ) -> Option<Self> {
        if !(-9_998..=9_998).contains(&start_utc.year())
            || !(-9_998..=9_998).contains(&end_utc.year())
        {
            return None;
        }
        let time_zone = UsageAnalyticsTimeZone::fixed(local_offset);
        let start_utc = start_utc.checked_to_offset(UtcOffset::UTC)?;
        let end_utc = end_utc.checked_to_offset(UtcOffset::UTC)?;
        let local_start = time_zone.local_datetime_at(start_utc)?;
        let local_end = time_zone.local_datetime_at(end_utc)?;
        if start_utc >= end_utc
            || local_start.time() != Time::MIDNIGHT
            || local_end.time() != Time::MIDNIGHT
        {
            return None;
        }

        Some(Self {
            start_utc,
            end_utc,
            start_date: local_start.date(),
            end_date: local_end.date(),
            time_zone,
        })
    }

    pub fn for_local_dates(
        start_date: Date,
        end_date: Date,
        time_zone: UsageAnalyticsTimeZone,
    ) -> Option<Self> {
        if start_date >= end_date {
            return None;
        }
        let start_utc = time_zone.local_midnight_utc(start_date)?;
        let end_utc = time_zone.local_midnight_utc(end_date)?;
        if start_utc >= end_utc {
            return None;
        }

        Some(Self {
            start_utc,
            end_utc,
            start_date,
            end_date,
            time_zone,
        })
    }

    pub fn start_utc(&self) -> OffsetDateTime {
        self.start_utc
    }

    pub fn end_utc(&self) -> OffsetDateTime {
        self.end_utc
    }

    pub fn start_date(&self) -> Date {
        self.start_date
    }

    pub fn end_date(&self) -> Date {
        self.end_date
    }

    pub fn time_zone(&self) -> UsageAnalyticsTimeZone {
        self.time_zone
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
