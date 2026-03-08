use crate::{
    components::provider_template_form::ProviderTemplateFormState as TemplateEditForm,
    config::AiChatConfig,
    database::{
        ConversationTemplate, ConversationTemplatePrompt, Db, Mode, NewConversationTemplate, Role,
    },
    i18n::{I18n, t_static},
    llm::{adapter_names, template_inputs_by_adapter},
};
use gpui::*;
use gpui_component::{
    IndexPath, WindowExt,
    button::{Button, ButtonVariants},
    divider::Divider,
    form::{field, v_form},
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use std::{cell::RefCell, rc::Rc};
use time::OffsetDateTime;

struct PromptEditorRow {
    role_input: Entity<SelectState<Vec<Role>>>,
    prompt_input: Entity<InputState>,
}

struct PromptListForm {
    prompt_rows: Vec<PromptEditorRow>,
}

struct TemplateFormContainer {
    form: Entity<TemplateEditForm>,
}

type OnSaved = Rc<dyn Fn(ConversationTemplate, &mut Window, &mut App) + 'static>;

#[derive(Clone)]
struct TemplateDialogI18n {
    dialog_title: SharedString,
    submit_label: SharedString,
    success_title: SharedString,
    failure_title: SharedString,
    name_label: SharedString,
    icon_label: SharedString,
    description_label: SharedString,
    mode_label: SharedString,
    adapter_label: SharedString,
    template_label: SharedString,
    prompts_label: SharedString,
    cancel_label: SharedString,
    load_schema_failed_title: SharedString,
}

#[derive(Clone)]
struct TemplateDialogFields {
    name_input: Entity<InputState>,
    icon_input: Entity<InputState>,
    description_input: Entity<InputState>,
    mode_input: Entity<SelectState<Vec<Mode>>>,
    adapter_input: Entity<SelectState<Vec<String>>>,
    template_form_container: Entity<TemplateFormContainer>,
    prompt_form_input: Entity<PromptListForm>,
}

struct TemplateFormSubmission {
    new_template: NewConversationTemplate,
    failure_title: SharedString,
    success_title: SharedString,
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
                .ok_or_else(|| {
                    format!("{} {}", t_static("template-error-select-role"), index + 1)
                })?;
            let prompt = row.prompt_input.read(cx).value().trim().to_string();
            if prompt.is_empty() {
                return Err(format!(
                    "{} {}",
                    t_static("template-error-prompt-empty"),
                    index + 1
                ));
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
                .placeholder(t_static("field-prompt"))
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

impl Render for PromptListForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (prompts_label, delete_label, role_label, prompt_label, add_prompt_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-prompts"),
                i18n.t("button-delete"),
                i18n.t("field-role"),
                i18n.t("field-prompt"),
                i18n.t("button-add-prompt"),
            )
        };
        let this = cx.entity().downgrade();
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
                            .child(Label::new(format!("{prompt_label} {}", index + 1)))
                            .child(
                                Button::new(("prompt-delete", index))
                                    .label(delete_label.clone())
                                    .on_click(move |_, _window, cx| {
                                        let _ = this.update(cx, |form, cx| {
                                            form.remove_prompt_row(index, cx);
                                        });
                                    }),
                            ),
                    )
                    .child(
                        field()
                            .required(true)
                            .label(role_label.clone())
                            .child(Select::new(&row.role_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label(prompt_label.clone())
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
                    .child(Label::new(prompts_label))
                    .child(Button::new("prompt-add").label(add_prompt_label).on_click(
                        move |_, window, cx| {
                            let _ = this.update(cx, |form, cx| {
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
        TemplateDialogParams {
            template_id: Some(template_id),
            dialog_title_key: "dialog-edit-template-title",
            submit_label_key: "button-submit",
            success_title_key: "notify-template-updated-success",
            failure_title_key: "notify-update-template-failed",
        },
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
                .title(cx.global::<I18n>().t("notify-no-adapter-available"))
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
        TemplateDialogParams {
            template_id: None,
            dialog_title_key: "dialog-add-template-title",
            submit_label_key: "button-create",
            success_title_key: "notify-template-created-success",
            failure_title_key: "notify-create-template-failed",
        },
        template,
        on_saved,
        window,
        cx,
    );
}

struct TemplateDialogParams {
    template_id: Option<i32>,
    dialog_title_key: &'static str,
    submit_label_key: &'static str,
    success_title_key: &'static str,
    failure_title_key: &'static str,
}

fn open_template_dialog(
    params: TemplateDialogParams,
    template: ConversationTemplate,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let TemplateDialogParams {
        template_id,
        dialog_title_key,
        submit_label_key,
        success_title_key,
        failure_title_key,
    } = params;
    let labels = template_dialog_i18n(
        dialog_title_key,
        submit_label_key,
        success_title_key,
        failure_title_key,
        cx,
    );
    let Some(fields) = build_template_dialog_fields(&template, &labels, window, cx) else {
        return;
    };
    let adapter_subscription = window.subscribe(&fields.adapter_input, cx, {
        let template_form_container = fields.template_form_container.clone();
        move |_state, event: &SelectEvent<Vec<String>>, window, cx| {
            let SelectEvent::Confirm(adapter) = event;
            let Some(adapter) = adapter.as_deref() else {
                return;
            };
            reload_template_form_for_adapter(adapter, &template_form_container, window, cx);
        }
    });
    let adapter_subscription = Rc::new(RefCell::new(Some(adapter_subscription)));

    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(labels.dialog_title.clone())
            .w(px(900.))
            .h(px(600.))
            .child(
                v_form()
                    .child(
                        field()
                            .required(true)
                            .label(labels.name_label.clone())
                            .child(Input::new(&fields.name_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label(labels.icon_label.clone())
                            .child(Input::new(&fields.icon_input)),
                    )
                    .child(
                        field()
                            .label(labels.description_label.clone())
                            .child(Input::new(&fields.description_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label(labels.mode_label.clone())
                            .child(Select::new(&fields.mode_input)),
                    )
                    .child(
                        field()
                            .required(true)
                            .label(labels.adapter_label.clone())
                            .child(Select::new(&fields.adapter_input)),
                    )
                    .child(
                        field()
                            .label(labels.template_label.clone())
                            .child(fields.template_form_container.clone()),
                    )
                    .child(
                        field()
                            .label(labels.prompts_label.clone())
                            .child(fields.prompt_form_input.clone()),
                    ),
            )
            .footer({
                let fields = fields.clone();
                let adapter_subscription = adapter_subscription.clone();
                let on_saved = on_saved.clone();
                let labels = labels.clone();
                move |_dialog, _state, _window, _cx| {
                    let _keep_subscription_alive = adapter_subscription.borrow();
                    vec![
                        Button::new("cancel")
                            .label(labels.cancel_label.clone())
                            .on_click(|_, window, cx| {
                                window.close_dialog(cx);
                            }),
                        Button::new("submit")
                            .primary()
                            .label(labels.submit_label.clone())
                            .on_click({
                                let fields = fields.clone();
                                let on_saved = on_saved.clone();
                                let labels = labels.clone();
                                move |_, window, cx| {
                                    let Some(submission) =
                                        collect_template_submission(&fields, &labels, window, cx)
                                    else {
                                        return;
                                    };
                                    let Some(latest) = save_template_submission(
                                        template_id,
                                        submission,
                                        window,
                                        cx,
                                    ) else {
                                        return;
                                    };
                                    (on_saved)(latest, window, cx);
                                    window.close_dialog(cx);
                                }
                            }),
                    ]
                }
            })
    });
}

fn template_dialog_i18n(
    dialog_title_key: &str,
    submit_label_key: &str,
    success_title_key: &str,
    failure_title_key: &str,
    cx: &App,
) -> TemplateDialogI18n {
    let i18n = cx.global::<I18n>();
    TemplateDialogI18n {
        dialog_title: i18n.t(dialog_title_key).into(),
        submit_label: i18n.t(submit_label_key).into(),
        success_title: i18n.t(success_title_key).into(),
        failure_title: i18n.t(failure_title_key).into(),
        name_label: i18n.t("field-name").into(),
        icon_label: i18n.t("field-icon").into(),
        description_label: i18n.t("field-description").into(),
        mode_label: i18n.t("field-mode").into(),
        adapter_label: i18n.t("field-adapter").into(),
        template_label: i18n.t("field-template").into(),
        prompts_label: i18n.t("field-prompts").into(),
        cancel_label: i18n.t("button-cancel").into(),
        load_schema_failed_title: i18n.t("notify-load-template-schema-failed").into(),
    }
}

fn build_template_dialog_fields(
    template: &ConversationTemplate,
    labels: &TemplateDialogI18n,
    window: &mut Window,
    cx: &mut App,
) -> Option<TemplateDialogFields> {
    let name_input = create_dialog_input(&template.name, &labels.name_label, window, cx);
    let icon_input = create_dialog_input(&template.icon, &labels.icon_label, window, cx);
    let description_input = create_dialog_input(
        template.description.clone().unwrap_or_default(),
        &labels.description_label,
        window,
        cx,
    );
    let mode_input = cx.new(|cx| {
        SelectState::new(
            ordered_mode_options(template.mode),
            Some(IndexPath::default()),
            window,
            cx,
        )
    });
    let adapter_input = cx.new(|cx| {
        SelectState::new(
            ordered_adapter_options(&template.adapter),
            Some(IndexPath::default()),
            window,
            cx,
        )
    });
    let template_form =
        load_template_form(&template.adapter, &template.template, labels, window, cx)?;
    let template_form_container = cx.new(|_cx| TemplateFormContainer::new(template_form));
    let prompt_form_input = cx.new(|cx| PromptListForm::new(template, window, cx));
    Some(TemplateDialogFields {
        name_input,
        icon_input,
        description_input,
        mode_input,
        adapter_input,
        template_form_container,
        prompt_form_input,
    })
}

fn create_dialog_input(
    value: impl Into<SharedString>,
    placeholder: &SharedString,
    window: &mut Window,
    cx: &mut App,
) -> Entity<InputState> {
    let value: SharedString = value.into();
    let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder.clone()));
    input.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
    input
}

fn ordered_mode_options(mode: Mode) -> Vec<Mode> {
    match mode {
        Mode::Contextual => vec![Mode::Contextual, Mode::Single, Mode::AssistantOnly],
        Mode::Single => vec![Mode::Single, Mode::Contextual, Mode::AssistantOnly],
        Mode::AssistantOnly => vec![Mode::AssistantOnly, Mode::Contextual, Mode::Single],
    }
}

fn ordered_adapter_options(current: &str) -> Vec<String> {
    let mut adapter_options = vec![current.to_string()];
    adapter_options.extend(
        adapter_names()
            .into_iter()
            .filter(|adapter| *adapter != current)
            .map(ToString::to_string),
    );
    adapter_options
}

fn load_template_form(
    adapter: &str,
    template_value: &serde_json::Value,
    labels: &TemplateDialogI18n,
    window: &mut Window,
    cx: &mut App,
) -> Option<Entity<TemplateEditForm>> {
    let template_items = match template_inputs_by_adapter(adapter, cx.global::<AiChatConfig>()) {
        Ok(items) => items,
        Err(err) => {
            push_error_notification(
                window,
                labels.load_schema_failed_title.clone(),
                err.to_string(),
                cx,
            );
            return None;
        }
    };
    Some(cx.new(|cx| TemplateEditForm::new(template_items, template_value, window, cx)))
}

fn reload_template_form_for_adapter(
    adapter: &str,
    template_form_container: &Entity<TemplateFormContainer>,
    window: &mut Window,
    cx: &mut App,
) {
    let current_template = template_form_container
        .read(cx)
        .collect_template(cx)
        .unwrap_or_else(|_| serde_json::json!({}));
    let labels = TemplateDialogI18n {
        dialog_title: SharedString::new_static(""),
        submit_label: SharedString::new_static(""),
        success_title: SharedString::new_static(""),
        failure_title: SharedString::new_static(""),
        name_label: SharedString::new_static(""),
        icon_label: SharedString::new_static(""),
        description_label: SharedString::new_static(""),
        mode_label: SharedString::new_static(""),
        adapter_label: SharedString::new_static(""),
        template_label: SharedString::new_static(""),
        prompts_label: SharedString::new_static(""),
        cancel_label: SharedString::new_static(""),
        load_schema_failed_title: cx
            .global::<I18n>()
            .t("notify-load-template-schema-failed")
            .into(),
    };
    let Some(next_form) = load_template_form(adapter, &current_template, &labels, window, cx)
    else {
        return;
    };
    template_form_container.update(cx, |container, cx| {
        container.set_form(next_form, cx);
    });
}

fn collect_template_submission(
    fields: &TemplateDialogFields,
    labels: &TemplateDialogI18n,
    window: &mut Window,
    cx: &mut App,
) -> Option<TemplateFormSubmission> {
    let name = fields.name_input.read(cx).value().trim().to_string();
    let icon = fields.icon_input.read(cx).value().trim().to_string();
    let description = optional_input_value(&fields.description_input, cx);
    let mode = fields
        .mode_input
        .read(cx)
        .selected_value()
        .copied()
        .or_else(|| {
            push_error_notification(
                window,
                cx.global::<I18n>().t("notify-select-mode").into(),
                String::new(),
                cx,
            );
            None
        })?;
    let adapter = fields
        .adapter_input
        .read(cx)
        .selected_value()
        .cloned()
        .or_else(|| {
            push_error_notification(
                window,
                cx.global::<I18n>().t("notify-select-adapter").into(),
                String::new(),
                cx,
            );
            None
        })?;
    let template = match fields.template_form_container.read(cx).collect_template(cx) {
        Ok(template) => template,
        Err(err) => {
            push_error_notification(
                window,
                cx.global::<I18n>().t("notify-invalid-template").into(),
                err,
                cx,
            );
            return None;
        }
    };
    let prompts = match fields.prompt_form_input.read(cx).collect_prompts(cx) {
        Ok(prompts) => prompts,
        Err(err) => {
            push_error_notification(
                window,
                cx.global::<I18n>().t("notify-invalid-prompts").into(),
                err,
                cx,
            );
            return None;
        }
    };
    Some(TemplateFormSubmission {
        new_template: NewConversationTemplate {
            name,
            icon,
            description,
            mode,
            adapter,
            template,
            prompts,
        },
        failure_title: labels.failure_title.clone(),
        success_title: labels.success_title.clone(),
    })
}

fn optional_input_value(input: &Entity<InputState>, cx: &App) -> Option<String> {
    let value = input.read(cx).value().trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn save_template_submission(
    template_id: Option<i32>,
    submission: TemplateFormSubmission,
    window: &mut Window,
    cx: &mut App,
) -> Option<ConversationTemplate> {
    let TemplateFormSubmission {
        new_template,
        failure_title,
        success_title,
    } = submission;
    let mut conn = match cx.global::<Db>().get() {
        Ok(conn) => conn,
        Err(err) => {
            push_error_notification(
                window,
                cx.global::<I18n>().t("notify-open-database-failed").into(),
                err.to_string(),
                cx,
            );
            return None;
        }
    };
    let template_id = match template_id {
        Some(template_id) => {
            if let Err(err) = ConversationTemplate::update(new_template, template_id, &mut conn) {
                push_error_notification(window, failure_title.clone(), err.to_string(), cx);
                return None;
            }
            template_id
        }
        None => match new_template.insert(&mut conn) {
            Ok(template_id) => template_id,
            Err(err) => {
                push_error_notification(window, failure_title.clone(), err.to_string(), cx);
                return None;
            }
        },
    };
    let latest = match ConversationTemplate::find(template_id, &mut conn) {
        Ok(template) => template,
        Err(err) => {
            push_error_notification(
                window,
                cx.global::<I18n>()
                    .t("notify-reload-template-failed")
                    .into(),
                err.to_string(),
                cx,
            );
            return None;
        }
    };
    window.push_notification(
        Notification::new()
            .title(success_title)
            .with_type(NotificationType::Success),
        cx,
    );
    Some(latest)
}

fn push_error_notification(
    window: &mut Window,
    title: SharedString,
    message: impl Into<SharedString>,
    cx: &mut App,
) {
    let message: SharedString = message.into();
    let notification = if message.is_empty() {
        Notification::new().title(title)
    } else {
        Notification::new().title(title).message(message)
    };
    window.push_notification(notification.with_type(NotificationType::Error), cx);
}
