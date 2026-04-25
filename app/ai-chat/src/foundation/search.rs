use pinyin::ToPinyin;

pub(crate) fn field_matches_query(value: &str, raw_query: &str) -> bool {
    let query = raw_query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }

    let value_lower = value.to_lowercase();
    if value_lower.contains(&query) {
        return true;
    }

    let (plain, initials) = pinyin_search_keys(value);
    plain.contains(&query) || initials.contains(&query)
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

#[cfg(test)]
mod tests {
    use super::{field_matches_query, pinyin_search_keys};

    #[test]
    fn field_matches_query_supports_plain_text() {
        assert!(field_matches_query("OpenAI API Key", "api key"));
        assert!(field_matches_query("OpenAI API Key", "  OPENAI  "));
        assert!(!field_matches_query("OpenAI API Key", "ollama"));
    }

    #[test]
    fn field_matches_query_supports_pinyin() {
        assert!(field_matches_query("命名助手", "命名"));
        assert!(field_matches_query("命名助手", "mingming"));
        assert!(field_matches_query("命名助手", "mmzs"));
        assert!(field_matches_query("生成更好的名字", "shengcheng"));
        assert!(!field_matches_query("命名助手", "shengcheng"));
    }

    #[test]
    fn pinyin_search_keys_include_full_spelling_and_initials() {
        assert_eq!(
            pinyin_search_keys("命名助手"),
            ("mingmingzhushou".to_string(), "mmzs".to_string())
        );
    }
}
