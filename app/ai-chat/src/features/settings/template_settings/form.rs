use crate::{
    database::{
        ConversationTemplate, ConversationTemplatePrompt, Db, NewConversationTemplate, Role,
    },
    foundation::{assets::IconName, i18n::I18n},
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
    scroll::ScrollableElement,
    select::{Select, SelectState},
    v_flex,
};
use std::rc::Rc;
use time::OffsetDateTime;

type OnSaved = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Clone)]
struct TemplateDialogLabels {
    dialog_title: SharedString,
    submit_label: SharedString,
    success_title: SharedString,
    failure_title: SharedString,
    name_label: SharedString,
    icon_label: SharedString,
    description_label: SharedString,
    cancel_label: SharedString,
    required_template_message: SharedString,
    required_prompt_role_message: SharedString,
    required_prompt_content_message: SharedString,
}

#[derive(Clone)]
struct TemplateDialogFields {
    name_input: Entity<InputState>,
    icon_input: Entity<InputState>,
    description_input: Entity<InputState>,
    prompt_form_input: Entity<PromptListForm>,
}

struct PromptEditorRow {
    role_input: Entity<SelectState<Vec<Role>>>,
    prompt_input: Entity<InputState>,
}

struct PromptListForm {
    prompt_rows: Vec<PromptEditorRow>,
}

#[derive(Clone, Debug)]
struct PromptFormValue {
    role: Option<Role>,
    prompt: String,
}

pub(super) fn open_add_template_dialog(on_saved: OnSaved, window: &mut Window, cx: &mut App) {
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
    open_template_form_dialog(None, template, DialogMode::Add, on_saved, window, cx);
}

pub(super) fn open_edit_template_dialog(
    template_id: i32,
    template: ConversationTemplate,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    open_template_form_dialog(
        Some(template_id),
        template,
        DialogMode::Edit,
        on_saved,
        window,
        cx,
    );
}

#[derive(Clone, Copy)]
enum DialogMode {
    Add,
    Edit,
}

impl DialogMode {
    fn labels(self, cx: &App) -> TemplateDialogLabels {
        let i18n = cx.global::<I18n>();
        TemplateDialogLabels {
            dialog_title: i18n
                .t(match self {
                    Self::Add => "dialog-add-template-title",
                    Self::Edit => "dialog-edit-template-title",
                })
                .into(),
            submit_label: i18n
                .t(match self {
                    Self::Add => "button-create",
                    Self::Edit => "button-save-changes",
                })
                .into(),
            success_title: i18n
                .t(match self {
                    Self::Add => "notify-template-created-success",
                    Self::Edit => "notify-template-updated-success",
                })
                .into(),
            failure_title: i18n
                .t(match self {
                    Self::Add => "notify-create-template-failed",
                    Self::Edit => "notify-update-template-failed",
                })
                .into(),
            name_label: i18n.t("field-name").into(),
            icon_label: i18n.t("field-icon").into(),
            description_label: i18n.t("field-description").into(),
            cancel_label: i18n.t("button-cancel").into(),
            required_template_message: i18n.t("template-error-name-icon-required").into(),
            required_prompt_role_message: i18n.t("template-error-select-role").into(),
            required_prompt_content_message: i18n.t("template-error-prompt-empty").into(),
        }
    }

    fn submit_icon(self) -> IconName {
        match self {
            Self::Add => IconName::Upload,
            Self::Edit => IconName::Save,
        }
    }
}

fn open_template_form_dialog(
    template_id: Option<i32>,
    template: ConversationTemplate,
    mode: DialogMode,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let labels = mode.labels(cx);
    let submit_icon = mode.submit_icon();
    let fields = dialog_fields(&template, window, cx);

    window.open_dialog(cx, move |dialog, _window, _cx| {
        dialog
            .title(labels.dialog_title.clone())
            .child(
                v_flex().w(px(680.)).max_h(px(640.)).child(
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
                        .child(field().child(fields.prompt_form_input.clone())),
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
                                            notify_error(
                                                labels.failure_title.clone(),
                                                err,
                                                window,
                                                cx,
                                            );
                                            return;
                                        }
                                    };
                                    save_template(
                                        template_id,
                                        submission,
                                        labels.success_title.clone(),
                                        labels.failure_title.clone(),
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

fn dialog_fields(
    template: &ConversationTemplate,
    window: &mut Window,
    cx: &mut App,
) -> TemplateDialogFields {
    let name_input =
        cx.new(|cx| InputState::new(window, cx).placeholder(cx.global::<I18n>().t("field-name")));
    let icon_input =
        cx.new(|cx| InputState::new(window, cx).placeholder(cx.global::<I18n>().t("field-icon")));
    let description_input = cx.new(|cx| {
        InputState::new(window, cx).placeholder(cx.global::<I18n>().t("field-description"))
    });
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
    labels: &TemplateDialogLabels,
    cx: &App,
) -> Result<NewConversationTemplate, String> {
    let prompts = fields.prompt_form_input.read(cx).collect_prompt_values(cx);
    let name = fields.name_input.read(cx).value();
    let icon = fields.icon_input.read(cx).value();
    let description = fields.description_input.read(cx).value();
    build_template_submission(
        name.as_ref(),
        icon.as_ref(),
        description.as_ref(),
        prompts,
        labels.required_template_message.as_ref(),
        labels.required_prompt_role_message.as_ref(),
        labels.required_prompt_content_message.as_ref(),
    )
}

fn build_template_submission(
    name: &str,
    icon: &str,
    description: &str,
    prompts: Vec<PromptFormValue>,
    required_template_message: &str,
    required_prompt_role_message: &str,
    required_prompt_content_message: &str,
) -> Result<NewConversationTemplate, String> {
    let name = name.trim().to_string();
    let icon = icon.trim().to_string();
    if name.is_empty() || icon.is_empty() {
        return Err(required_template_message.to_string());
    }

    let mut collected_prompts = Vec::with_capacity(prompts.len());
    for (index, prompt) in prompts.into_iter().enumerate() {
        let role = prompt
            .role
            .ok_or_else(|| format!("{} {}", required_prompt_role_message, index + 1))?;
        let prompt = prompt.prompt.trim().to_string();
        if prompt.is_empty() {
            return Err(format!("{} {}", required_prompt_content_message, index + 1));
        }
        collected_prompts.push(ConversationTemplatePrompt { role, prompt });
    }

    let description = description.trim().to_string();
    Ok(NewConversationTemplate {
        name,
        icon,
        description: if description.is_empty() {
            None
        } else {
            Some(description)
        },
        prompts: collected_prompts,
    })
}

fn save_template(
    template_id: Option<i32>,
    new_template: NewConversationTemplate,
    success_title: SharedString,
    failure_title: SharedString,
    on_saved: OnSaved,
    window: &mut Window,
    cx: &mut App,
) {
    let mut conn = match cx.global::<Db>().get() {
        Ok(conn) => conn,
        Err(err) => {
            notify_error(failure_title, err.to_string(), window, cx);
            return;
        }
    };

    let result = match template_id {
        Some(template_id) => ConversationTemplate::update(new_template, template_id, &mut conn),
        None => new_template.insert(&mut conn).map(|_| ()),
    };

    match result {
        Ok(()) => {
            window.close_dialog(cx);
            window.push_notification(
                Notification::new()
                    .title(success_title)
                    .with_type(NotificationType::Success),
                cx,
            );
            on_saved(window, cx);
        }
        Err(err) => {
            notify_error(failure_title, err.to_string(), window, cx);
        }
    }
}

fn notify_error(
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    window.push_notification(
        Notification::new()
            .title(title)
            .message(message)
            .with_type(NotificationType::Error),
        cx,
    );
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

    fn collect_prompt_values(&self, cx: &App) -> Vec<PromptFormValue> {
        self.prompt_rows
            .iter()
            .map(|row| PromptFormValue {
                role: row.role_input.read(cx).selected_value().copied(),
                prompt: row.prompt_input.read(cx).value().to_string(),
            })
            .collect()
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
                .placeholder(cx.global::<I18n>().t("field-prompt"))
                .multi_line(true)
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
        let (prompts_label, delete_label, role_label, content_label, add_prompt_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-prompts"),
                i18n.t("button-delete"),
                i18n.t("field-role"),
                i18n.t("section-content"),
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
                            .child(Label::new(format!("#{}", index + 1)))
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
                        field().required(true).label(content_label.clone()).child(
                            Input::new(&row.prompt_input)
                                .min_h(px(144.))
                                .max_h(px(240.)),
                        ),
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
            .child(
                v_flex()
                    .max_h(px(360.))
                    .overflow_y_scrollbar()
                    .gap_3()
                    .children(prompt_fields),
            )
    }
}

#[cfg(test)]
mod tests;
