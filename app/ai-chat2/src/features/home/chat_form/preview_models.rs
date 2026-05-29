use super::thinking_effort::{ThinkingEffort, computed_default_effort, selectable_efforts};

pub(crate) struct PreviewModel {
    pub(crate) provider: &'static str,
    pub(crate) name: &'static str,
    efforts: &'static [ThinkingEffort],
    default_effort: Option<ThinkingEffort>,
}

impl PreviewModel {
    pub(crate) fn selectable_efforts(&self) -> Vec<ThinkingEffort> {
        selectable_efforts(self.efforts)
    }

    pub(crate) fn computed_default_effort(&self) -> Option<ThinkingEffort> {
        computed_default_effort(self.efforts, self.default_effort)
    }
}

const GPT_55_EFFORTS: &[ThinkingEffort] = &[
    ThinkingEffort::None,
    ThinkingEffort::Low,
    ThinkingEffort::Medium,
    ThinkingEffort::High,
    ThinkingEffort::XHigh,
];
const GPT_55_PRO_EFFORTS: &[ThinkingEffort] = &[
    ThinkingEffort::Medium,
    ThinkingEffort::High,
    ThinkingEffort::XHigh,
];
const CLAUDE_EFFORTS: &[ThinkingEffort] = &[
    ThinkingEffort::Low,
    ThinkingEffort::Medium,
    ThinkingEffort::High,
];

const PREVIEW_MODELS: &[PreviewModel] = &[
    PreviewModel {
        provider: "OpenAI",
        name: "GPT-5.5",
        efforts: GPT_55_EFFORTS,
        default_effort: Some(ThinkingEffort::Medium),
    },
    PreviewModel {
        provider: "OpenAI",
        name: "GPT-5.5 Pro",
        efforts: GPT_55_PRO_EFFORTS,
        default_effort: Some(ThinkingEffort::Medium),
    },
    PreviewModel {
        provider: "Anthropic",
        name: "Claude Sonnet 4.5",
        efforts: CLAUDE_EFFORTS,
        default_effort: Some(ThinkingEffort::Medium),
    },
];

pub(crate) fn preview_models() -> &'static [PreviewModel] {
    PREVIEW_MODELS
}

pub(crate) fn preview_model(index: usize) -> &'static PreviewModel {
    &PREVIEW_MODELS[index.min(PREVIEW_MODELS.len().saturating_sub(1))]
}

#[cfg(test)]
mod tests {
    use super::{ThinkingEffort, preview_model};

    #[test]
    fn preview_model_efforts_hide_none() {
        assert_eq!(
            preview_model(0).selectable_efforts(),
            vec![
                ThinkingEffort::Low,
                ThinkingEffort::Medium,
                ThinkingEffort::High,
                ThinkingEffort::XHigh,
            ]
        );
    }
}
