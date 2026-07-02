use gpui::{App, AppContext as _, Context, Entity, Focusable, IntoElement, SharedString, Window};
use gpui_component::{
    searchable_list::SearchableListDelegate,
    select::{SelectEvent, SelectItem, SelectState},
};
use gpui_form::{
    ComponentStateOptions, FieldChangeCause, FieldError, FormComponentBinding, FormComponentEvent,
    FormComponentEventSink, SubscriptionSet,
};

use crate::foundation::I18n;

type StringInputBinding = gpui_form_gpui_component::TextInputBinding<String>;
type BoolInputBinding = gpui_form_gpui_component::BoolBinding;
type SecretInputBinding = super::ProviderSecretInputBinding;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::features::settings::provider) enum ProviderApiMode {
    #[default]
    Responses,
    ChatCompletions,
}

impl ProviderApiMode {
    pub(in crate::features::settings::provider) fn from_key(value: &str) -> Self {
        match value {
            "chat_completions" => Self::ChatCompletions,
            _ => Self::Responses,
        }
    }

    pub(in crate::features::settings::provider) const fn key(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
        }
    }
}

impl gpui_form_gpui_component::SelectFieldValue for ProviderApiMode {
    type Selected = ProviderApiMode;

    fn to_selected_value(&self) -> Option<Self::Selected> {
        Some(*self)
    }

    fn from_selected_value(selected: Option<Self::Selected>, previous: &Self) -> Self {
        selected.unwrap_or(*previous)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::features::settings::provider) struct ApiModeChoice {
    value: ProviderApiMode,
    label: SharedString,
}

impl ApiModeChoice {
    fn new(value: ProviderApiMode, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

impl SelectItem for ApiModeChoice {
    type Value = ProviderApiMode;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn display_title(&self) -> Option<gpui::AnyElement> {
        Some(self.label.clone().into_any_element())
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub(in crate::features::settings::provider) fn localized_api_mode_choices(
    i18n: &I18n,
) -> Vec<ApiModeChoice> {
    vec![
        ApiModeChoice::new(
            ProviderApiMode::Responses,
            i18n.t("provider-api-mode-responses"),
        ),
        ApiModeChoice::new(
            ProviderApiMode::ChatCompletions,
            i18n.t("provider-api-mode-chat-completions"),
        ),
    ]
}

pub(in crate::features::settings::provider) struct ProviderApiModeSelectBinding;

impl FormComponentBinding<ProviderApiMode> for ProviderApiModeSelectBinding {
    type State = SelectState<Vec<ApiModeChoice>>;
    type Draft = ProviderApiMode;

    fn new_state(
        initial: &ProviderApiMode,
        _options: ComponentStateOptions,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let choices = localized_api_mode_choices(cx.global::<I18n>());
        let selected_index = choices.position(initial);
        cx.new(|cx| SelectState::new(choices, selected_index, window, cx))
    }

    fn draft_from_value(value: &ProviderApiMode) -> Self::Draft {
        *value
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).selected_value().copied().unwrap_or_default()
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: gpui_form::FieldPath,
        _trigger: gpui_form::ValidationTrigger,
        _cx: &App,
    ) -> Result<ProviderApiMode, Box<FieldError>> {
        Ok(*draft)
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &ProviderApiMode,
        _cause: FieldChangeCause,
        window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |select, cx| {
            select.set_selected_value(value, window, cx);
        });
    }

    fn focus(state: &Entity<Self::State>, window: &mut Window, cx: &mut App) -> bool {
        let focus_handle = state.read(cx).focus_handle(cx);
        focus_handle.focus(window, cx);
        true
    }

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        sink: FormComponentEventSink<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> SubscriptionSet
    where
        Form: 'static,
    {
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |form, _state, event: &SelectEvent<Vec<ApiModeChoice>>, window, cx| {
                let SelectEvent::Confirm(_) = event;
                sink.emit(
                    form,
                    FormComponentEvent::Change(FieldChangeCause::UserInput),
                    window,
                    cx,
                );
            },
        ));
        subscriptions
    }
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = CustomOpenAiProviderFormStore)]
pub(in crate::features::settings::provider) struct CustomOpenAiProviderFormInput {
    #[form(binding = "BoolInputBinding")]
    pub(super) enabled: bool,
    #[form(
        binding = "StringInputBinding",
        label = "provider-field-name",
        placeholder = "provider-placeholder-provider-name",
        required,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) name: String,
    #[form(
        binding = "SecretInputBinding",
        label = "provider-field-api-key",
        placeholder = "provider-placeholder-api-key",
        required,
        mask,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) api_key: super::ProviderSecretValue,
    #[form(
        binding = "StringInputBinding",
        label = "provider-field-base-url",
        placeholder = "provider-placeholder-custom-base-url",
        required,
        validate(on_change, on_blur, on_submit)
    )]
    pub(super) base_url: String,
    #[form(
        binding = "ProviderApiModeSelectBinding",
        label = "provider-field-api-mode",
        placeholder = "provider-placeholder-api-mode",
        validate(on_change, on_submit)
    )]
    pub(super) api_mode: ProviderApiMode,
}
