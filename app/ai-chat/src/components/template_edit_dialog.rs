use crate::{
    adapter::{adapter_names, template_inputs_by_adapter, InputItem, InputType},
    config::AiChatConfig,
    database::{
        ConversationTemplate, ConversationTemplatePrompt, Db, Mode, NewConversationTemplate, Role,
    },
};
use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    divider::Divider,
    form::{field, v_form},
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    label::Label,
    notification::{Notification, NotificationType},
    select::{Select, SelectEvent, SelectState},
    v_flex, IndexPath, WindowExt,
};
use std::{cell::RefCell, rc::Rc};
use time::OffsetDateTime;

struct TemplateFieldRow {
    item: InputItem,
    value_state: TemplateFieldValueState,
}

#[derive(Clone, Copy)]
struct NumberFieldOptions {
    min: f64,
    max: f64,
    step: f64,
    integer: bool,
}

enum TemplateFieldValueState {
    Input(Entity<InputState>),
    Select(Entity<SelectState<Vec<String>>>),
}

struct PromptEditorRow {
    role_input: Entity<SelectState<Vec<Role>>>,
    prompt_input: Entity<InputState>,
}

struct TemplateEditForm {
    template_rows: Vec<TemplateFieldRow>,
    _subscriptions: Vec<Subscription>,
}

struct PromptListForm {
    prompt_rows: Vec<PromptEditorRow>,
}

struct TemplateFormContainer {
    form: Entity<TemplateEditForm>,
}

type OnSaved = Rc<dyn Fn(ConversationTemplate, &mut Window, &mut App) + 'static>;

impl TemplateEditForm {
    fn new(
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
                        TemplateFieldValueState::Select(select_state)
                    }
                    _ => {
                        let input_state = cx.new(|cx| InputState::new(window, cx));
                        let text = Self::value_as_string(&value);
                        input_state.update(cx, |input, cx| input.set_value(text, window, cx));
                        TemplateFieldValueState::Input(input_state)
                    }
                };
                TemplateFieldRow { item, value_state }
            })
            .collect();

        let mut this = Self {
            template_rows,
            _subscriptions: Vec::new(),
        };
        this.bind_number_input_events(window, cx);
        this
    }

    fn bind_number_input_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for row in &self.template_rows {
            let Some(options) = Self::number_options(row.item.input_type()) else {
                continue;
            };
            let TemplateFieldValueState::Input(input) = &row.value_state else {
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
                    .ok_or_else(|| "Must be a valid float".to_string())?;
                Ok(serde_json::Value::Number(number))
            }
            InputType::Boolean { .. } => {
                let boolean = raw
                    .parse::<bool>()
                    .map_err(|_| "Must be true or false".to_string())?;
                Ok(serde_json::Value::Bool(boolean))
            }
            InputType::Integer { .. } => {
                let number = raw
                    .parse::<i64>()
                    .map(Into::into)
                    .or_else(|_| raw.parse::<u64>().map(Into::into))
                    .map(serde_json::Value::Number)
                    .map_err(|_| "Must be a valid integer".to_string())?;
                Ok(number)
            }
            InputType::Select(options) => {
                if options.is_empty() || options.iter().any(|option| option == raw) {
                    Ok(serde_json::Value::String(raw.to_string()))
                } else {
                    Err("Selected value is not in available options".to_string())
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
                    .map_err(|_| "Must be a valid JSON value".to_string())
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

    fn collect_template(&self, cx: &App) -> Result<serde_json::Value, String> {
        let mut map = serde_json::Map::new();
        for row in &self.template_rows {
            let raw = match &row.value_state {
                TemplateFieldValueState::Input(input) => input.read(cx).value().to_string(),
                TemplateFieldValueState::Select(select) => select
                    .read(cx)
                    .selected_value()
                    .cloned()
                    .unwrap_or_default(),
            };
            let value = Self::parse_value_by_type(row.item.input_type(), &raw)
                .map_err(|err| format!("Template field '{}': {err}", row.item.name()))?;
            map.insert(row.item.id().to_string(), value);
        }
        Ok(serde_json::Value::Object(map))
    }
}

impl TemplateFormContainer {
    fn new(form: Entity<TemplateEditForm>) -> Self {
        Self { form }
    }

    fn set_form(&mut self, form: Entity<TemplateEditForm>, cx: &mut Context<Self>) {
        self.form = form;
        cx.notify();
    }

    fn collect_template(&self, cx: &App) -> Result<serde_json::Value, String> {
        self.form.read(cx).collect_template(cx)
    }
}

impl PromptListForm {
    fn new(template: &ConversationTemplate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prompt_rows = template
            .prompts
            .iter()
            .map(|prompt| PromptEditorRow::new(prompt.role, prompt.prompt.clone(), window, cx))
            .collect();
        Self { prompt_rows }
    }

    fn role_options(role: Role) -> Vec<Role> {
        match role {
            Role::Developer => vec![Role::Developer, Role::User, Role::Assistant],
            Role::User => vec![Role::User, Role::Developer, Role::Assistant],
            Role::Assistant => vec![Role::Assistant, Role::Developer, Role::User],
        }
    }

    fn collect_prompts(&self, cx: &App) -> Result<Vec<ConversationTemplatePrompt>, String> {
        let mut prompts = Vec::with_capacity(self.prompt_rows.len());
        for (index, row) in self.prompt_rows.iter().enumerate() {
            let role = row
                .role_input
                .read(cx)
                .selected_value()
                .copied()
                .ok_or_else(|| format!("Please select role for prompt {}", index + 1))?;
            let prompt = row.prompt_input.read(cx).value().trim().to_string();
            if prompt.is_empty() {
                return Err(format!("Prompt {} cannot be empty", index + 1));
            }
            prompts.push(ConversationTemplatePrompt { role, prompt });
        }
        Ok(prompts)
    }

    fn add_prompt_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt_rows
            .push(PromptEditorRow::new(Role::User, String::new(), window, cx));
        cx.notify();
    }

    fn remove_prompt_row(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.prompt_rows.len() {
            self.prompt_rows.remove(index);
            cx.notify();
        }
    }
}

impl PromptEditorRow {
    fn new(
        role: Role,
        prompt: String,
        window: &mut Window,
        cx: &mut Context<PromptListForm>,
    ) -> Self {
        let role_input = cx.new(|cx| {
            SelectState::new(
                PromptListForm::role_options(role),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let prompt_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Prompt")
                .multi_line(true)
                .line_number(true)
        });
        prompt_input.update(cx, |input, cx| input.set_value(prompt, window, cx));
        Self {
            role_input,
            prompt_input,
        }
    }
}

impl Render for TemplateEditForm {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let template_fields = self
            .template_rows
            .iter()
            .map(|row| {
                let child = match &row.value_state {
                    TemplateFieldValueState::Input(input) => {
                        if Self::number_options(row.item.input_type()).is_some() {
                            NumberInput::new(input).into_any_element()
                        } else {
                            Input::new(input).into_any_element()
                        }
                    }
                    TemplateFieldValueState::Select(select) => {
                        Select::new(select).into_any_element()
                    }
                };
                field()
                    .required(Self::is_required(row.item.input_type()))
                    .label(format!("Template / {}", row.item.name()))
                    .child(child)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        v_flex().gap_2().children(template_fields)
    }
}

impl Render for PromptListForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().clone();
        let prompt_fields = self
            .prompt_rows
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let this = this.clone();
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(Label::new(format!("Prompt {}", index + 1)))
                            .child(
                                Button::new(("prompt-delete", index))
                                    .label("Delete")
                                    .on_click(move |_, _window, cx| {
                                        this.update(cx, |form, cx| {
                                            form.remove_prompt_row(index, cx);
                                        });
                                    }),
                            ),
                    )
                    .child(
                        field()
                            .required(true)
                            .label("Role")
                            .child(Select::new(&row.role_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label("Prompt")
                            .child(Input::new(&row.prompt_input).h_24()),
                    )
                    .child(Divider::horizontal())
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(Label::new("Prompts"))
                    .child(Button::new("prompt-add").label("Add Prompt").on_click(
                        move |_, window, cx| {
                            this.update(cx, |form, cx| {
                                form.add_prompt_row(window, cx);
                            });
                        },
                    )),
            )
            .child(v_flex().gap_3().children(prompt_fields))
    }
}

impl Render for TemplateFormContainer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.form.clone()
    }
}

pub(crate) fn open_template_edit_dialog(
    template_id: i32,
    template: ConversationTemplate,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    open_template_dialog(
        Some(template_id),
        "Edit Template",
        "Submit",
        "Template updated successfully",
        "Update template failed",
        template,
        on_saved,
        window,
        cx,
    );
}

pub(crate) fn open_add_template_dialog(on_saved: OnSaved, window: &mut Window, cx: &mut App) {
    let Some(default_adapter) = adapter_names().first().copied() else {
        window.push_notification(
            Notification::new()
                .title("No adapter available")
                .with_type(NotificationType::Error),
            cx,
        );
        return;
    };
    let now = OffsetDateTime::now_utc();
    let template = ConversationTemplate {
        id: 0,
        name: String::new(),
        icon: "🧩".to_string(),
        description: None,
        mode: Mode::Contextual,
        adapter: default_adapter.to_string(),
        template: serde_json::json!({}),
        prompts: vec![ConversationTemplatePrompt {
            role: Role::Developer,
            prompt: "You are a helpful assistant.".to_string(),
        }],
        created_time: now,
        updated_time: now,
    };
    open_template_dialog(
        None,
        "Add Template",
        "Create",
        "Template created successfully",
        "Create template failed",
        template,
        on_saved,
        window,
        cx,
    );
}

fn open_template_dialog(
    template_id: Option<i32>,
    dialog_title: &'static str,
    submit_label: &'static str,
    success_title: &'static str,
    failure_title: &'static str,
    template: ConversationTemplate,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
    name_input.update(cx, |input, cx| input.set_value(&template.name, window, cx));

    let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder("Icon"));
    icon_input.update(cx, |input, cx| input.set_value(&template.icon, window, cx));

    let description_input = cx.new(|cx| InputState::new(window, cx).placeholder("Description"));
    description_input.update(cx, |input, cx| {
        input.set_value(template.description.clone().unwrap_or_default(), window, cx)
    });

    let mode_options: Vec<Mode> = match template.mode {
        Mode::Contextual => vec![Mode::Contextual, Mode::Single, Mode::AssistantOnly],
        Mode::Single => vec![Mode::Single, Mode::Contextual, Mode::AssistantOnly],
        Mode::AssistantOnly => vec![Mode::AssistantOnly, Mode::Contextual, Mode::Single],
    };
    let mode_input: Entity<SelectState<Vec<Mode>>> =
        cx.new(|cx| SelectState::new(mode_options, Some(IndexPath::default()), window, cx));

    let mut adapter_options = vec![template.adapter.clone()];
    adapter_options.extend(
        adapter_names()
            .into_iter()
            .filter(|adapter| *adapter != template.adapter)
            .map(ToString::to_string),
    );
    let adapter_input: Entity<SelectState<Vec<String>>> =
        cx.new(|cx| SelectState::new(adapter_options, Some(IndexPath::default()), window, cx));

    let template_items =
        match template_inputs_by_adapter(&template.adapter, cx.global::<AiChatConfig>()) {
            Ok(items) => items,
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title("Load template schema failed")
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
                return;
            }
        };
    let template_value = template.template.clone();
    let template_form_input =
        cx.new(|cx| TemplateEditForm::new(template_items, &template_value, window, cx));
    let template_form_container =
        cx.new(|_cx| TemplateFormContainer::new(template_form_input.clone()));
    let prompt_form_input = cx.new(|cx| PromptListForm::new(&template, window, cx));
    let adapter_subscription = window.subscribe(&adapter_input, cx, {
        let template_form_container = template_form_container.clone();
        move |_state, event: &SelectEvent<Vec<String>>, window, cx| {
            let SelectEvent::Confirm(adapter) = event;
            let Some(adapter) = adapter.as_deref() else {
                return;
            };
            let current_template = template_form_container
                .read(cx)
                .collect_template(cx)
                .unwrap_or_else(|_| serde_json::json!({}));
            let template_items =
                match template_inputs_by_adapter(adapter, cx.global::<AiChatConfig>()) {
                    Ok(items) => items,
                    Err(err) => {
                        window.push_notification(
                            Notification::new()
                                .title("Load template schema failed")
                                .message(err.to_string())
                                .with_type(NotificationType::Error),
                            cx,
                        );
                        return;
                    }
                };
            let next_form =
                cx.new(|cx| TemplateEditForm::new(template_items, &current_template, window, cx));
            template_form_container.update(cx, |container, cx| {
                container.set_form(next_form, cx);
            });
        }
    });
    let adapter_subscription = Rc::new(RefCell::new(Some(adapter_subscription)));

    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(dialog_title)
            .w(px(900.))
            .h(px(600.))
            .child(
                v_form()
                    .child(
                        field()
                            .required(true)
                            .label("Name")
                            .child(Input::new(&name_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label("Icon")
                            .child(Input::new(&icon_input)),
                    )
                    .child(
                        field()
                            .label("Description")
                            .child(Input::new(&description_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label("Mode")
                            .child(Select::new(&mode_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label("Adapter")
                            .child(Select::new(&adapter_input)),
                    )
                    .child(
                        field()
                            .label("Template")
                            .child(template_form_container.clone()),
                    )
                    .child(field().label("Prompts").child(prompt_form_input.clone())),
            )
            .footer({
                let name_input = name_input.clone();
                let icon_input = icon_input.clone();
                let description_input = description_input.clone();
                let mode_input = mode_input.clone();
                let adapter_input = adapter_input.clone();
                let template_form_container = template_form_container.clone();
                let prompt_form_input = prompt_form_input.clone();
                let adapter_subscription = adapter_subscription.clone();
                let on_saved = on_saved.clone();
                move |_dialog, _state, _window, _cx| {
                    let _keep_subscription_alive = adapter_subscription.borrow();
                    vec![
                        Button::new("cancel")
                            .label("Cancel")
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                        Button::new("submit")
                            .primary()
                            .label(submit_label)
                            .on_click({
                                let name_input = name_input.clone();
                                let icon_input = icon_input.clone();
                                let description_input = description_input.clone();
                                let mode_input = mode_input.clone();
                                let adapter_input = adapter_input.clone();
                                let template_form_container = template_form_container.clone();
                                let prompt_form_input = prompt_form_input.clone();
                                let on_saved = on_saved.clone();
                                move |_, window, cx| {
                                    let name = name_input.read(cx).value().trim().to_string();
                                    let icon = icon_input.read(cx).value().trim().to_string();
                                    let description = {
                                        let value =
                                            description_input.read(cx).value().trim().to_string();
                                        if value.is_empty() {
                                            None
                                        } else {
                                            Some(value)
                                        }
                                    };
                                    let mode = match mode_input.read(cx).selected_value().copied() {
                                        Some(mode) => mode,
                                        None => {
                                            window.push_notification(
                                                Notification::new()
                                                    .title("Please select a mode")
                                                    .with_type(NotificationType::Error),
                                                cx,
                                            );
                                            return;
                                        }
                                    };
                                    let adapter = match adapter_input.read(cx).selected_value() {
                                        Some(adapter) => adapter.clone(),
                                        None => {
                                            window.push_notification(
                                                Notification::new()
                                                    .title("Please select an adapter")
                                                    .with_type(NotificationType::Error),
                                                cx,
                                            );
                                            return;
                                        }
                                    };
                                    let template =
                                        match template_form_container.read(cx).collect_template(cx)
                                        {
                                            Ok(template) => template,
                                            Err(err) => {
                                                window.push_notification(
                                                    Notification::new()
                                                        .title("Invalid template")
                                                        .message(err)
                                                        .with_type(NotificationType::Error),
                                                    cx,
                                                );
                                                return;
                                            }
                                        };
                                    let prompts =
                                        match prompt_form_input.read(cx).collect_prompts(cx) {
                                            Ok(prompts) => prompts,
                                            Err(err) => {
                                                window.push_notification(
                                                    Notification::new()
                                                        .title("Invalid prompts")
                                                        .message(err)
                                                        .with_type(NotificationType::Error),
                                                    cx,
                                                );
                                                return;
                                            }
                                        };

                                    let mut conn = match cx.global::<Db>().get() {
                                        Ok(conn) => conn,
                                        Err(err) => {
                                            window.push_notification(
                                                Notification::new()
                                                    .title("Open database failed")
                                                    .message(err.to_string())
                                                    .with_type(NotificationType::Error),
                                                cx,
                                            );
                                            return;
                                        }
                                    };

                                    let new_template = NewConversationTemplate {
                                        name,
                                        icon,
                                        description,
                                        mode,
                                        adapter,
                                        template,
                                        prompts,
                                    };
                                    let template_id = match template_id {
                                        Some(template_id) => {
                                            if let Err(err) = ConversationTemplate::update(
                                                new_template,
                                                template_id,
                                                &mut conn,
                                            ) {
                                                window.push_notification(
                                                    Notification::new()
                                                        .title(failure_title)
                                                        .message(err.to_string())
                                                        .with_type(NotificationType::Error),
                                                    cx,
                                                );
                                                return;
                                            }
                                            template_id
                                        }
                                        None => match new_template.insert(&mut conn) {
                                            Ok(template_id) => template_id,
                                            Err(err) => {
                                                window.push_notification(
                                                    Notification::new()
                                                        .title(failure_title)
                                                        .message(err.to_string())
                                                        .with_type(NotificationType::Error),
                                                    cx,
                                                );
                                                return;
                                            }
                                        },
                                    };

                                    let latest =
                                        match ConversationTemplate::find(template_id, &mut conn) {
                                            Ok(template) => template,
                                            Err(err) => {
                                                window.push_notification(
                                                    Notification::new()
                                                        .title("Reload template failed")
                                                        .message(err.to_string())
                                                        .with_type(NotificationType::Error),
                                                    cx,
                                                );
                                                return;
                                            }
                                        };

                                    (on_saved)(latest, window, cx);
                                    window.push_notification(
                                        Notification::new()
                                            .title(success_title)
                                            .with_type(NotificationType::Success),
                                        cx,
                                    );
                                    window.close_dialog(cx);
                                }
                            }),
                    ]
                }
            })
    });
}
