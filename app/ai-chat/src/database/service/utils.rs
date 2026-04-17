use serde::{Deserialize, Deserializer, Serializer};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use pinyin::ToPinyin;

pub fn serialize_offset_date_time<S>(
    date: &OffsetDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = date.format(&Rfc3339).map_err(serde::ser::Error::custom)?; // ISO 8601 format
    serializer.serialize_str(&s)
}

pub fn deserialize_offset_date_time<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    OffsetDateTime::parse(&s, &Rfc3339).map_err(serde::de::Error::custom)
}

pub(crate) fn field_matches_query(value: &str, query: &str) -> bool {
    let value_lower = value.to_lowercase();
    if value_lower.contains(query) {
        return true;
    }

    let (plain, initials) = pinyin_search_keys(value);
    plain.contains(query) || initials.contains(query)
}

pub(crate) fn pinyin_search_keys(value: &str) -> (String, String) {
    let mut plain = String::new();
    let mut initials = String::new();

    for ch in value.chars() {
        if let Some(pinyin) = ch.to_pinyin() {
            let syllable = pinyin.plain();
            plain.push_str(syllable);
            if let Some(initial) = syllable.chars().next() {
                initials.push(initial);
            }
            continue;
        }

        if ch.is_ascii_alphanumeric() {
            let lower = ch.to_ascii_lowercase();
            plain.push(lower);
            initials.push(lower);
        }
    }

    (plain, initials)
}
