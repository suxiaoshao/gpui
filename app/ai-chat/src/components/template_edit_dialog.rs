use crate::{
    database::{
        ConversationTemplate, ConversationTemplatePrompt, Db, NewConversationTemplate, Role,
    },
    foundation::assets::IconName,
    foundation::i18n::{I18n, t_static},
};
use gpui::*;
use gpui_component::{
    Sizable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::{DialogClose, DialogFooter},
    divider::Divider,
    form::{field, v_form},
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    select::Select,
    select::SelectState,
    v_flex,
};
use std::rc::Rc;
use time::OffsetDateTime;

type OnSaved = Rc<dyn Fn(ConversationTemplate, &mut Window, &mut App) + 'static>;

struct PromptEditorRow {
    role_input: Entity<SelectState<Vec<Role>>>,
    prompt_input: Entity<InputState>,
}

struct PromptListForm {
    prompt_rows: Vec<PromptEditorRow>,
}

#[derive(Clone)]
struct TemplateDialogI18n {
    dialog_title: SharedString,
    submit_label: SharedString,
    success_title: SharedString,
    failure_title: SharedString,
    name_label: SharedString,
    icon_label: SharedString,
    description_label: SharedString,
    prompts_label: SharedString,
    cancel_label: SharedString,
}

#[derive(Clone)]
struct TemplateDialogFields {
    name_input: Entity<InputState>,
    icon_input: Entity<InputState>,
    description_input: Entity<InputState>,
    prompt_form_input: Entity<PromptListForm>,
}

struct TemplateFormSubmission {
    new_template: NewConversationTemplate,
    failure_title: SharedString,
    success_title: SharedString,
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
                Some(gpui_component::IndexPath::default()),
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
                                    .icon(IconName::X)
                                    .ghost()
                                    .small()
                                    .tooltip(delete_label.clone())
                                    .on_click(move |_, _window, cx| {
                                        let _ = this.update(cx, |form, cx| {
                                            form.remove_prompt_row(index, cx)
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
                    .child(
                        Button::new("prompt-add")
                            .icon(IconName::Plus)
                            .label(add_prompt_label)
                            .on_click(move |_, window, cx| {
                                let _ = this.update(cx, |form, cx| form.add_prompt_row(window, cx));
                            }),
                    ),
            )
            .child(v_flex().gap_3().children(prompt_fields))
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
    let now = OffsetDateTime::now_utc();
    let template = ConversationTemplate {
        id: 0,
        name: String::new(),
        icon: "🧩".to_string(),
        description: None,
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
    let labels = dialog_labels(
        params.dialog_title_key,
        params.submit_label_key,
        params.success_title_key,
        params.failure_title_key,
        cx,
    );
    let submit_icon = if params.template_id.is_some() {
        IconName::Save
    } else {
        IconName::Upload
    };
    let fields = dialog_fields(&template, window, cx);
    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(labels.dialog_title.clone())
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
                            .label(labels.prompts_label.clone())
                            .child(fields.prompt_form_input.clone()),
                    ),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new()
                            .child(Button::new("cancel").label(labels.cancel_label.clone())),
                    )
                    .child(
                        Button::new("submit")
                            .primary()
                            .icon(submit_icon)
                            .label(labels.submit_label.clone())
                            .on_click({
                                let labels = labels.clone();
                                let fields = fields.clone();
                                let on_saved = on_saved.clone();
                                move |_, window, cx| {
                                    let submission = match collect_submission(&fields, &labels, cx)
                                    {
                                        Ok(submission) => submission,
                                        Err(err) => {
                                            window.push_notification(
                                                Notification::new()
                                                    .title(labels.failure_title.clone())
                                                    .message(err)
                                                    .with_type(NotificationType::Error),
                                                cx,
                                            );
                                            return;
                                        }
                                    };
                                    save_template(
                                        params.template_id,
                                        submission,
                                        on_saved.clone(),
                                        window,
                                        cx,
                                    );
                                }
                            }),
                    ),
            )
    });
}

fn dialog_labels(
    dialog_title_key: &'static str,
    submit_label_key: &'static str,
    success_title_key: &'static str,
    failure_title_key: &'static str,
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
        prompts_label: i18n.t("field-prompts").into(),
        cancel_label: i18n.t("button-cancel").into(),
    }
}

fn dialog_fields(
    template: &ConversationTemplate,
    window: &mut Window,
    cx: &mut App,
) -> TemplateDialogFields {
    let name_input = cx.new(|cx| InputState::new(window, cx).placeholder(t_static("field-name")));
    let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder(t_static("field-icon")));
    let description_input =
        cx.new(|cx| InputState::new(window, cx).placeholder(t_static("field-description")));
    name_input.update(cx, |input, cx| {
        input.set_value(template.name.clone(), window, cx)
    });
    icon_input.update(cx, |input, cx| {
        input.set_value(template.icon.clone(), window, cx)
    });
    description_input.update(cx, |input, cx| {
        input.set_value(template.description.clone().unwrap_or_default(), window, cx)
    });
    let prompt_form_input = cx.new(|cx| PromptListForm::new(template, window, cx));
    TemplateDialogFields {
        name_input,
        icon_input,
        description_input,
        prompt_form_input,
    }
}

fn collect_submission(
    fields: &TemplateDialogFields,
    labels: &TemplateDialogI18n,
    cx: &App,
) -> Result<TemplateFormSubmission, String> {
    let name = fields.name_input.read(cx).value().trim().to_string();
    let icon = fields.icon_input.read(cx).value().trim().to_string();
    if name.is_empty() || icon.is_empty() {
        return Err(format!("{} / {}", labels.name_label, labels.icon_label));
    }
    let description = fields.description_input.read(cx).value().trim().to_string();
    let prompts = fields.prompt_form_input.read(cx).collect_prompts(cx)?;
    Ok(TemplateFormSubmission {
        new_template: NewConversationTemplate {
            name,
            icon,
            description: if description.is_empty() {
                None
            } else {
                Some(description)
            },
            prompts,
        },
        failure_title: labels.failure_title.clone(),
        success_title: labels.success_title.clone(),
    })
}

fn save_template(
    template_id: Option<i32>,
    submission: TemplateFormSubmission,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let mut conn = match cx.global::<Db>().get() {
        Ok(conn) => conn,
        Err(err) => {
            window.push_notification(
                Notification::new()
                    .title(submission.failure_title)
                    .message(err.to_string())
                    .with_type(NotificationType::Error),
                cx,
            );
            return;
        }
    };
    let template_id = match template_id {
        Some(template_id) => {
            if let Err(err) =
                ConversationTemplate::update(submission.new_template, template_id, &mut conn)
            {
                window.push_notification(
                    Notification::new()
                        .title(submission.failure_title)
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
                return;
            }
            template_id
        }
        None => match submission.new_template.insert(&mut conn) {
            Ok(template_id) => template_id,
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title(submission.failure_title)
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
                return;
            }
        },
    };
    let latest = match ConversationTemplate::find(template_id, &mut conn) {
        Ok(template) => template,
        Err(err) => {
            window.push_notification(
                Notification::new()
                    .title(submission.failure_title)
                    .message(err.to_string())
                    .with_type(NotificationType::Error),
                cx,
            );
            return;
        }
    };
    window.close_dialog(cx);
    window.push_notification(
        Notification::new()
            .title(submission.success_title)
            .with_type(NotificationType::Success),
        cx,
    );
    on_saved(latest, window, cx);
}
