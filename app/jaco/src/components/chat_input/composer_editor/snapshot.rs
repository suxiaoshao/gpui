use jaco_agent::SkillActivationRequest;
use jaco_core::{ContentPart, SkillSourceKind};

use super::token::{ComposerToken, skill_requests};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ComposerSendPolicy {
    EnterToSend,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ComposerTokenSnapshot {
    pub(crate) name: String,
    pub(crate) range: std::ops::Range<usize>,
    pub(crate) source_kind: SkillSourceKind,
    pub(crate) skill_file_path: String,
    pub(crate) directory_path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ComposerSnapshot {
    pub(crate) text: String,
    pub(crate) content_parts: Vec<ContentPart>,
    pub(crate) skill_requests: Vec<SkillActivationRequest>,
    pub(crate) token_ranges: Vec<ComposerTokenSnapshot>,
    pub(crate) send_policy: ComposerSendPolicy,
}

impl ComposerSnapshot {
    pub(crate) fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.skill_requests.is_empty()
    }
}

pub(super) fn build_snapshot(text: &str, tokens: &[ComposerToken]) -> ComposerSnapshot {
    let content_parts = if text.is_empty() {
        Vec::new()
    } else {
        vec![ContentPart::Text {
            text: text.to_string(),
        }]
    };

    ComposerSnapshot {
        text: text.to_string(),
        content_parts,
        skill_requests: skill_requests(tokens),
        token_ranges: tokens
            .iter()
            .map(|token| ComposerTokenSnapshot {
                name: token.name.clone(),
                range: token.range.clone(),
                source_kind: token.skill.source_kind,
                skill_file_path: token.skill.skill_file_path.clone(),
                directory_path: token.skill.directory_path.clone(),
            })
            .collect(),
        send_policy: ComposerSendPolicy::EnterToSend,
    }
}
