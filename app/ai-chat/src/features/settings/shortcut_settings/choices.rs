use crate::{
    database::{ConversationTemplate, Mode, ShortcutInputSource},
    foundation::i18n::I18n,
    llm::ProviderModel,
};
use gpui::*;
use gpui_component::select::SelectItem;

#[derive(Clone)]
pub(super) struct TemplateChoice {
    pub(super) id: Option<i32>,
    pub(super) label: SharedString,
    template: Option<ConversationTemplate>,
}

impl TemplateChoice {
    pub(super) fn none(cx: &App) -> Self {
        Self {
            id: None,
            label: cx.global::<I18n>().t("field-none").into(),
            template: None,
        }
    }

    pub(super) fn from_template(template: &ConversationTemplate) -> Self {
        Self {
            id: Some(template.id),
            label: SharedString::from(format!("{} {}", template.icon, template.name)),
            template: Some(template.clone()),
        }
    }
}

impl SelectItem for TemplateChoice {
    type Value = Option<i32>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn matches(&self, query: &str) -> bool {
        self.template.as_ref().map_or_else(
            || self.label.to_lowercase().contains(&query.to_lowercase()),
            |template| template.matches_search_query(query),
        )
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

#[derive(Clone)]
pub(super) struct ModelChoice {
    pub(super) value: String,
    pub(super) title: SharedString,
}

impl ModelChoice {
    pub(super) fn key(provider_name: &str, model_id: &str) -> String {
        format!("{provider_name}\u{1f}{model_id}")
    }

    pub(super) fn from_model(model: &ProviderModel) -> Self {
        Self {
            value: Self::key(&model.provider_name, &model.id),
            title: model.id.clone().into(),
        }
    }

    pub(super) fn unresolved(provider_name: &str, model_id: &str, cx: &App) -> Self {
        Self {
            value: Self::key(provider_name, model_id),
            title: SharedString::from(format!(
                "{} ({})",
                model_id,
                cx.global::<I18n>().t("shortcut-model-unavailable")
            )),
        }
    }
}

impl SelectItem for ModelChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
pub(super) struct ModeChoice {
    pub(super) value: Mode,
    pub(super) label: SharedString,
}

impl ModeChoice {
    pub(super) fn new(value: Mode, cx: &App) -> Self {
        let label = {
            let key = match value {
                Mode::Contextual => "mode-contextual",
                Mode::Single => "mode-single",
                Mode::AssistantOnly => "mode-assistant-only",
            };
            cx.global::<I18n>().t(key).into()
        };
        Self { value, label }
    }

    pub(super) fn label(&self) -> SharedString {
        self.label.clone()
    }
}

impl SelectItem for ModeChoice {
    type Value = Mode;

    fn title(&self) -> SharedString {
        self.value.to_string().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(div().child(self.label()).into_any_element())
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = cx;
        div().child(self.label())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
pub(super) struct InputSourceChoice {
    pub(super) value: ShortcutInputSource,
    pub(super) label: SharedString,
}

impl InputSourceChoice {
    pub(super) fn new(value: ShortcutInputSource, cx: &App) -> Self {
        let label = {
            let key = match value {
                ShortcutInputSource::SelectionOrClipboard => "send-content-selection-or-clipboard",
                ShortcutInputSource::Screenshot => "send-content-screenshot",
            };
            cx.global::<I18n>().t(key).into()
        };
        Self { value, label }
    }

    pub(super) fn label(&self) -> SharedString {
        self.label.clone()
    }
}

impl SelectItem for InputSourceChoice {
    type Value = ShortcutInputSource;

    fn title(&self) -> SharedString {
        self.value.to_string().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(div().child(self.label()).into_any_element())
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = cx;
        div().child(self.label())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
pub(super) struct ExtSettingChoice {
    pub(super) value: String,
    pub(super) label: SharedString,
}

impl SelectItem for ExtSettingChoice {
    type Value = String;

    fn title(&self) -> SharedString {
        self.value.clone().into()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(div().child(self.label.clone()).into_any_element())
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let _ = cx;
        div().child(self.label.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}
