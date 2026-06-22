use std::{collections::BTreeMap, ops::Range};

use ai_chat_agent::{SkillActivationRequest, SkillCatalog, SkillCatalogEntry};
use ai_chat_core::SkillSourceKind;

use super::buffer::is_word_char;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ComposerSkill {
    pub(super) name: String,
    pub(super) source_kind: SkillSourceKind,
    pub(super) skill_file_path: String,
    pub(super) directory_path: String,
}

impl From<&SkillCatalogEntry> for ComposerSkill {
    fn from(entry: &SkillCatalogEntry) -> Self {
        Self {
            name: entry.name.clone(),
            source_kind: entry.source_kind,
            skill_file_path: entry.skill_file_path.to_string_lossy().to_string(),
            directory_path: entry.directory_path.to_string_lossy().to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ComposerToken {
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) range: Range<usize>,
    pub(super) skill: ComposerSkill,
}

pub(super) fn skills_from_catalog(catalog: &SkillCatalog) -> BTreeMap<String, ComposerSkill> {
    catalog
        .entries()
        .map(|entry| (entry.name.clone(), ComposerSkill::from(entry)))
        .collect()
}

pub(super) fn parse_skill_tokens(
    text: &str,
    skills: &BTreeMap<String, ComposerSkill>,
    next_id: &mut u64,
) -> Vec<ComposerToken> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while let Some(relative_ix) = text[cursor..].find('$') {
        let start = cursor + relative_ix;
        let before_is_boundary = start == 0
            || text[..start]
                .chars()
                .next_back()
                .is_none_or(|ch| !is_word_char(ch));

        let name_start = start + '$'.len_utf8();
        let mut end = name_start;
        for (relative, ch) in text[name_start..].char_indices() {
            if !is_word_char(ch) {
                break;
            }
            end = name_start + relative + ch.len_utf8();
        }

        if before_is_boundary && end > name_start {
            let name = &text[name_start..end];
            if let Some(skill) = skills.get(name) {
                let id = *next_id;
                *next_id = next_id.wrapping_add(1).max(1);
                tokens.push(ComposerToken {
                    id,
                    name: name.to_string(),
                    range: start..end,
                    skill: skill.clone(),
                });
            }
        }

        cursor = end.max(name_start);
    }

    tokens
}

pub(super) fn skill_requests(tokens: &[ComposerToken]) -> Vec<SkillActivationRequest> {
    let mut seen = BTreeMap::new();
    for token in tokens {
        seen.entry(token.name.clone())
            .or_insert_with(|| SkillActivationRequest::new(token.name.clone()));
    }
    seen.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skills(names: &[&str]) -> BTreeMap<String, ComposerSkill> {
        names
            .iter()
            .map(|name| {
                (
                    (*name).to_string(),
                    ComposerSkill {
                        name: (*name).to_string(),
                        source_kind: SkillSourceKind::User,
                        skill_file_path: format!("/skills/{name}/SKILL.md"),
                        directory_path: format!("/skills/{name}"),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn parses_only_catalog_backed_skill_tokens() {
        let mut next_id = 1;
        let tokens = parse_skill_tokens("use $rust and $missing", &skills(&["rust"]), &mut next_id);

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].name, "rust");
        assert_eq!(tokens[0].range, 4..9);
    }

    #[test]
    fn skill_requests_are_unique() {
        let mut next_id = 1;
        let tokens = parse_skill_tokens("$rust $rust", &skills(&["rust"]), &mut next_id);
        assert_eq!(
            skill_requests(&tokens),
            vec![SkillActivationRequest::new("rust")]
        );
    }
}
