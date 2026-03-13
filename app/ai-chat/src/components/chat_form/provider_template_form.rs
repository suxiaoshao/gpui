use crate::{
    i18n::{I18n, t_static},
    llm::{InputItem, InputType},
};
use gpui::*;
use gpui_component::{
    IndexPath,
    form::field,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    select::{Select, SelectState},
    v_flex,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderTemplateFormEvent {
    Changed,
}

impl EventEmitter<ProviderTemplateFormEvent> for ProviderTemplateFormState {}

struct ProviderTemplateFieldRow {
    item: InputItem,
    value_state: ProviderTemplateFieldValueState,
}

#[derive(Clone, Copy)]
struct NumberFieldOptions {
    min: f64,
    max: f64,
    step: f64,
    integer: bool,
}

enum ProviderTemplateFieldValueState {
    Input(Entity<InputState>),
    Select(Entity<SelectState<Vec<String>>>),
}

pub(crate) struct ProviderTemplateFormState {
    template_rows: Vec<ProviderTemplateFieldRow>,
    _subscriptions: Vec<Subscription>,
}

// Builds field state from adapter metadata and binds interactive inputs.
impl ProviderTemplateFormState {
    pub(crate) fn new(
        items: Vec<InputItem>,
        template: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let template_rows = items
            .into_iter()
            .map(|item| {
                let value = template
                    .get(item.id())
                    .cloned()
                    .or_else(|| Self::default_value(item.input_type()))
                    .unwrap_or(serde_json::Value::Null);
                let value_state = match item.input_type() {
                    InputType::Boolean { .. } => {
                        let options = vec!["true".to_string(), "false".to_string()];
                        let selected = value.as_bool().unwrap_or_default().to_string();
                        let selected_index = options
                            .iter()
                            .position(|option| *option == selected)
                            .unwrap_or(0);
                        let select_state = cx.new(|cx| {
                            SelectState::new(
                                options,
                                Some(IndexPath::default().row(selected_index)),
                                window,
                                cx,
                            )
                        });
                        ProviderTemplateFieldValueState::Select(select_state)
                    }
                    InputType::Select(options) => {
                        let mut options = options.clone();
                        let selected = value
                            .as_str()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| options.first().cloned().unwrap_or_default());
                        if !selected.is_empty() && !options.contains(&selected) {
                            options.insert(0, selected.clone());
                        }
                        if options.is_empty() {
                            options.push(String::new());
                        }
                        let selected_index = options
                            .iter()
                            .position(|option| *option == selected)
                            .unwrap_or(0);
                        let select_state = cx.new(|cx| {
                            SelectState::new(
                                options,
                                Some(IndexPath::default().row(selected_index)),
                                window,
                                cx,
                            )
                        });
                        ProviderTemplateFieldValueState::Select(select_state)
                    }
                    _ => {
                        let input_state = cx.new(|cx| InputState::new(window, cx));
                        let text = Self::value_as_string(&value);
                        input_state.update(cx, |input, cx| input.set_value(text, window, cx));
                        ProviderTemplateFieldValueState::Input(input_state)
                    }
                };
                ProviderTemplateFieldRow { item, value_state }
            })
            .collect();

        let mut this = Self {
            template_rows,
            _subscriptions: Vec::new(),
        };
        this.bind_number_input_events(window, cx);
        this
    }

    pub(crate) fn collect_merged_template(
        &self,
        base_template: &serde_json::Value,
        cx: &App,
    ) -> Result<serde_json::Value, String> {
        Ok(Self::merge_template(
            base_template,
            self.collect_value_map(cx)?,
        ))
    }

    pub(crate) fn apply_template(
        &mut self,
        template: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for row in &self.template_rows {
            let value = template
                .get(row.item.id())
                .cloned()
                .or_else(|| Self::default_value(row.item.input_type()))
                .unwrap_or(serde_json::Value::Null);
            match &row.value_state {
                ProviderTemplateFieldValueState::Input(input) => {
                    input.update(cx, |state, cx| {
                        state.set_value(Self::value_as_string(&value), window, cx);
                    });
                }
                ProviderTemplateFieldValueState::Select(select) => {
                    let selected = Self::value_as_string(&value);
                    select.update(cx, |state, cx| {
                        state.set_selected_value(&selected, window, cx);
                    });
                }
            }
        }
        cx.emit(ProviderTemplateFormEvent::Changed);
        cx.notify();
    }
}

// Renders individual template controls in inline and popover layouts.
impl ProviderTemplateFormState {
    pub(crate) fn render_inline_field(&self, id: &str) -> Option<AnyElement> {
        let row = self.find_row(id)?;
        Some(
            div()
                .w(Self::inline_width(row.item.input_type()))
                .child(Self::render_input_control(row, true))
                .into_any_element(),
        )
    }

    pub(crate) fn render_popover_field(&self, id: &str) -> Option<AnyElement> {
        let row = self.find_row(id)?;
        let label = Self::localized_item_name(row.item.id(), row.item.name());
        Some(
            field()
                .required(Self::is_required(row.item.input_type()))
                .label(label)
                .description(row.item.description())
                .child(Self::render_input_control(row, false))
                .into_any_element(),
        )
    }
}

// Subscribes numeric inputs and collects typed field values.
impl ProviderTemplateFormState {
    fn bind_number_input_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for row in &self.template_rows {
            let Some(options) = Self::number_options(row.item.input_type()) else {
                continue;
            };
            let ProviderTemplateFieldValueState::Input(input) = &row.value_state else {
                continue;
            };
            let step_subscription = cx.subscribe_in(input, window, {
                let input = input.clone();
                move |_, _state, event: &NumberInputEvent, window, cx| match event {
                    NumberInputEvent::Step(action) => {
                        input.update(cx, |state, cx| {
                            let raw = state.value();
                            let value = raw.parse::<f64>().unwrap_or(0.0);
                            let next = if *action == StepAction::Increment {
                                value + options.step
                            } else {
                                value - options.step
                            };
                            let next = next.clamp(options.min, options.max);
                            let text = if options.integer {
                                (next.round() as i64).to_string()
                            } else {
                                next.to_string()
                            };
                            state.set_value(text, window, cx);
                        });
                    }
                }
            });
            let clamp_subscription = cx.subscribe_in(input, window, {
                let input = input.clone();
                move |_, _state, event: &InputEvent, window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    input.update(cx, |state, cx| {
                        let raw = state.value();
                        if let Ok(value) = raw.parse::<f64>() {
                            let clamped = value.clamp(options.min, options.max);
                            if (clamped - value).abs() > f64::EPSILON {
                                let text = if options.integer {
                                    (clamped.round() as i64).to_string()
                                } else {
                                    clamped.to_string()
                                };
                                state.set_value(text, window, cx);
                            }
                        }
                    });
                }
            });
            self._subscriptions.push(step_subscription);
            self._subscriptions.push(clamp_subscription);
        }
        self.bind_change_events(window, cx);
    }

    fn bind_change_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for row in &self.template_rows {
            match &row.value_state {
                ProviderTemplateFieldValueState::Input(input) => {
                    let subscription =
                        cx.subscribe_in(input, window, |_, _, event: &InputEvent, _, cx| {
                            if matches!(event, InputEvent::Change) {
                                cx.emit(ProviderTemplateFormEvent::Changed);
                            }
                        });
                    self._subscriptions.push(subscription);
                }
                ProviderTemplateFieldValueState::Select(select) => {
                    let subscription = cx.observe_in(select, window, |_, _, _, cx| {
                        cx.emit(ProviderTemplateFormEvent::Changed);
                    });
                    self._subscriptions.push(subscription);
                }
            }
        }
    }

    fn find_row(&self, id: &str) -> Option<&ProviderTemplateFieldRow> {
        self.template_rows.iter().find(|row| row.item.id() == id)
    }

    fn collect_value_map(
        &self,
        cx: &App,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        let mut map = serde_json::Map::new();
        for row in &self.template_rows {
            let raw = match &row.value_state {
                ProviderTemplateFieldValueState::Input(input) => input.read(cx).value().to_string(),
                ProviderTemplateFieldValueState::Select(select) => select
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .unwrap_or_default(),
            };
            let item_name = Self::localized_item_name(row.item.id(), row.item.name());
            let value = Self::parse_value_by_type(row.item.input_type(), &raw).map_err(|err| {
                format!(
                    "{} '{}': {err}",
                    t_static("template-error-field-prefix"),
                    item_name
                )
            })?;
            map.insert(row.item.id().to_string(), value);
        }
        Ok(map)
    }
}

// Maps field definitions to reusable GPUI controls and layout hints.
impl ProviderTemplateFormState {
    fn render_input_control(row: &ProviderTemplateFieldRow, compact: bool) -> AnyElement {
        match &row.value_state {
            ProviderTemplateFieldValueState::Input(input) => {
                if Self::number_options(row.item.input_type()).is_some() {
                    let input = NumberInput::new(input);
                    if compact {
                        input.into_any_element()
                    } else {
                        input.w_full().into_any_element()
                    }
                } else {
                    let input = Input::new(input);
                    if compact {
                        input.into_any_element()
                    } else {
                        input.w_full().into_any_element()
                    }
                }
            }
            ProviderTemplateFieldValueState::Select(select) => {
                let select = Select::new(select);
                if compact {
                    select.into_any_element()
                } else {
                    select.w_full().into_any_element()
                }
            }
        }
    }

    fn inline_width(input_type: &InputType) -> Pixels {
        match input_type {
            InputType::Select(_) => px(150.),
            InputType::Boolean { .. } => px(120.),
            _ if Self::number_options(input_type).is_some() => px(110.),
            _ => px(140.),
        }
    }
}

// Converts between serialized template values and typed form data.
impl ProviderTemplateFormState {
    fn merge_template(
        base_template: &serde_json::Value,
        updates: serde_json::Map<String, serde_json::Value>,
    ) -> serde_json::Value {
        let mut map = base_template
            .as_object()
            .cloned()
            .unwrap_or_else(serde_json::Map::new);
        map.extend(updates);
        serde_json::Value::Object(map)
    }

    fn default_value(input_type: &InputType) -> Option<serde_json::Value> {
        match input_type {
            InputType::Float { default, .. } => {
                default.and_then(|value| serde_json::Number::from_f64(value).map(Into::into))
            }
            InputType::Boolean { default } => default.map(Into::into),
            InputType::Integer { default, .. } => {
                default.map(|value| serde_json::Value::Number(serde_json::Number::from(value)))
            }
            InputType::Select(options) => options.first().cloned().map(Into::into),
            InputType::Optional(_) => Some(serde_json::Value::Null),
            _ => None,
        }
    }

    fn value_as_string(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Number(number) => number.to_string(),
            serde_json::Value::Bool(boolean) => boolean.to_string(),
            serde_json::Value::Null => String::new(),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
            }
        }
    }

    fn parse_value_by_type(input_type: &InputType, raw: &str) -> Result<serde_json::Value, String> {
        let raw = raw.trim();
        match input_type {
            InputType::Text { .. } => Ok(serde_json::Value::String(raw.to_string())),
            InputType::Float { .. } => {
                let number = raw
                    .parse::<f64>()
                    .ok()
                    .and_then(serde_json::Number::from_f64)
                    .ok_or_else(|| t_static("template-error-float"))?;
                Ok(serde_json::Value::Number(number))
            }
            InputType::Boolean { .. } => {
                let boolean = raw
                    .parse::<bool>()
                    .map_err(|_| t_static("template-error-boolean"))?;
                Ok(serde_json::Value::Bool(boolean))
            }
            InputType::Integer { .. } => {
                let number = raw
                    .parse::<i64>()
                    .map(Into::into)
                    .or_else(|_| raw.parse::<u64>().map(Into::into))
                    .map(serde_json::Value::Number)
                    .map_err(|_| t_static("template-error-integer"))?;
                Ok(number)
            }
            InputType::Select(options) => {
                if options.is_empty() || options.iter().any(|option| option == raw) {
                    Ok(serde_json::Value::String(raw.to_string()))
                } else {
                    Err(t_static("template-error-select"))
                }
            }
            InputType::Optional(inner) => {
                if raw.is_empty() {
                    Ok(serde_json::Value::Null)
                } else {
                    Self::parse_value_by_type(inner, raw)
                }
            }
            InputType::Object(_) | InputType::ArrayObject(_) | InputType::Array { .. } => {
                serde_json::from_str::<serde_json::Value>(raw)
                    .map_err(|_| t_static("template-error-json"))
            }
        }
    }

    fn number_options(input_type: &InputType) -> Option<NumberFieldOptions> {
        match input_type {
            InputType::Float { min, max, step, .. } => Some(NumberFieldOptions {
                min: min.unwrap_or(f64::MIN),
                max: max.unwrap_or(f64::MAX),
                step: step.unwrap_or(1.0),
                integer: false,
            }),
            InputType::Integer { min, max, step, .. } => Some(NumberFieldOptions {
                min: min.map(|value| value as f64).unwrap_or(f64::MIN),
                max: max.map(|value| value as f64).unwrap_or(f64::MAX),
                step: step.map(|value| value as f64).unwrap_or(1.0),
                integer: true,
            }),
            InputType::Optional(inner) => Self::number_options(inner),
            _ => None,
        }
    }

    fn is_required(input_type: &InputType) -> bool {
        !matches!(input_type, InputType::Optional(_))
    }

    fn localized_item_name(id: &str, fallback: &str) -> String {
        match id {
            "model" => t_static("field-model"),
            "web_search" => t_static("field-web-search"),
            "temperature" => t_static("field-temperature"),
            "top_p" => t_static("field-top-p"),
            "n" => t_static("field-n"),
            "max_completion_tokens" => t_static("field-max-completion-tokens"),
            "presence_penalty" => t_static("field-presence-penalty"),
            "frequency_penalty" => t_static("field-frequency-penalty"),
            _ => fallback.to_string(),
        }
    }
}

// Renders the full template editor form.
impl Render for ProviderTemplateFormState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let template_prefix = cx.global::<I18n>().t("field-template-prefix");
        let template_fields = self
            .template_rows
            .iter()
            .map(|row| {
                field()
                    .required(Self::is_required(row.item.input_type()))
                    .label(format!(
                        "{template_prefix} / {}",
                        Self::localized_item_name(row.item.id(), row.item.name())
                    ))
                    .child(Self::render_input_control(row, false))
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        v_flex().gap_2().children(template_fields)
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderTemplateFormState;

    #[test]
    fn merged_template_preserves_unknown_keys() {
        let base = serde_json::json!({
            "model": "base",
            "temperature": 1.0,
            "unknown": true
        });
        let merged = ProviderTemplateFormState::merge_template(
            &base,
            serde_json::Map::from_iter([("model".to_string(), serde_json::json!("override"))]),
        );
        assert_eq!(merged["model"], serde_json::json!("override"));
        assert_eq!(merged["temperature"], serde_json::json!(1.0));
        assert_eq!(merged["unknown"], serde_json::json!(true));
    }

    #[test]
    fn parse_integer_accepts_unsigned_values() {
        let value = ProviderTemplateFormState::parse_value_by_type(
            &crate::llm::InputType::Integer {
                max: None,
                min: Some(1),
                step: Some(1),
                default: Some(1),
            },
            "42",
        )
        .expect("integer");
        assert_eq!(value, serde_json::json!(42));
    }

    #[test]
    fn parse_invalid_float_returns_error() {
        let err = ProviderTemplateFormState::parse_value_by_type(
            &crate::llm::InputType::Float {
                min: Some(0.0),
                max: Some(2.0),
                step: Some(0.1),
                default: Some(1.0),
            },
            "abc",
        )
        .expect_err("invalid float");
        assert_eq!(err, crate::i18n::t_static("template-error-float"));
    }

    #[test]
    fn value_as_string_converts_booleans_for_select_restoration() {
        assert_eq!(
            ProviderTemplateFormState::value_as_string(&serde_json::json!(true)),
            "true"
        );
        assert_eq!(
            ProviderTemplateFormState::value_as_string(&serde_json::json!(false)),
            "false"
        );
    }
}
