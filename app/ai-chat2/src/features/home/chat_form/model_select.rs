use super::{
    ChatForm,
    picker::{PickerSection, picker_popover, picker_trigger},
    preview_models::{PreviewModel, preview_models},
};
use crate::{foundation, foundation::assets::IconName};
use gpui::*;
use gpui_component::{label::Label, select::SelectItem};

#[derive(Clone, Debug)]
pub(super) struct ModelOption {
    pub(super) index: usize,
    provider: &'static str,
    name: &'static str,
}

impl ModelOption {
    fn new(index: usize, model: &'static PreviewModel) -> Self {
        Self {
            index,
            provider: model.provider,
            name: model.name,
        }
    }
}

impl SelectItem for ModelOption {
    type Value = usize;

    fn title(&self) -> SharedString {
        self.name.into()
    }

    fn render(&self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        Label::new(self.name).text_sm().truncate()
    }

    fn value(&self) -> &Self::Value {
        &self.index
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.name.to_lowercase().contains(&query) || self.provider.to_lowercase().contains(&query)
    }
}

pub(super) fn model_sections() -> Vec<PickerSection<ModelOption>> {
    let mut sections = Vec::new();
    let mut provider = None;
    let mut items = Vec::new();

    for (index, model) in preview_models().iter().enumerate() {
        if provider != Some(model.provider) {
            if let Some(provider) = provider.take() {
                sections.push(PickerSection::section(provider, items));
                items = Vec::new();
            }
            provider = Some(model.provider);
        }

        items.push(ModelOption::new(index, model));
    }

    if let Some(provider) = provider {
        sections.push(PickerSection::section(provider, items));
    }

    sections
}

impl ChatForm {
    pub(super) fn render_model_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.global::<foundation::I18n>();

        picker_popover(
            cx,
            "chat-form-model-popover",
            self.model_picker_open,
            picker_trigger(
                "chat-form-model-trigger",
                IconName::Sparkles,
                self.selected_model().name,
                self.model_picker_open,
            ),
            self.model_picker.clone(),
            px(260.),
            rems(18.).into(),
            Some(i18n.t("chat-form-model-search-placeholder").into()),
            cx.listener(|form, open: &bool, window, cx| {
                form.set_model_picker_open(*open, window, cx);
            }),
        )
    }
}
