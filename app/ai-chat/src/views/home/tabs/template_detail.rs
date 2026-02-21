use crate::{
    adapter::{description_items_by_adapter, description_items_default},
    components::template_edit_dialog::open_template_edit_dialog,
    database::{ConversationTemplate, Db, Mode, Role},
    errors::AiChatResult,
    i18n::I18n,
    store::{ChatData, ChatDataEvent},
};
use gpui::*;
use gpui_component::description_list::{DescriptionItem, DescriptionList};
use gpui_component::{
    ActiveTheme, Sizable, WindowExt,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    tag::Tag,
    text::TextView,
    v_flex,
};
use std::{ops::Deref, rc::Rc};

pub(crate) struct TemplateDetailView {
    template_id: i32,
    template: AiChatResult<ConversationTemplate>,
}

impl TemplateDetailView {
    pub fn new(template_id: i32, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            template_id,
            template: Self::get_template(template_id, cx),
        }
    }

    fn get_template(
        template_id: i32,
        cx: &mut Context<Self>,
    ) -> AiChatResult<ConversationTemplate> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::find(template_id, conn)
    }

    fn open_edit_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let load_template_failed = cx.global::<I18n>().t("notify-load-template-failed");
        let template = match &self.template {
            Ok(template) => template.clone(),
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title(load_template_failed)
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
                return;
            }
        };
        let this = cx.entity().downgrade();
        open_template_edit_dialog(
            self.template_id,
            template,
            Rc::new(move |latest, _window, cx| {
                let _ = this.update(cx, |view, _cx| {
                    view.template = Ok(latest);
                });
            }),
            window,
            cx,
        );
    }

    fn template_tab_key(template_id: i32) -> i32 {
        template_id.saturating_add(1).saturating_neg()
    }

    fn delete_template(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (open_db_failed, delete_failed, delete_success) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("notify-open-database-failed"),
                i18n.t("notify-delete-template-failed"),
                i18n.t("notify-template-deleted-success"),
            )
        };
        let mut conn = match cx.global::<Db>().get() {
            Ok(conn) => conn,
            Err(err) => {
                window.push_notification(
                    Notification::new()
                        .title(open_db_failed)
                        .message(err.to_string())
                        .with_type(NotificationType::Error),
                    cx,
                );
                return;
            }
        };
        if let Err(err) = ConversationTemplate::delete(self.template_id, &mut conn) {
            window.push_notification(
                Notification::new()
                    .title(delete_failed)
                    .message(err.to_string())
                    .with_type(NotificationType::Error),
                cx,
            );
            return;
        }
        let chat_data = cx.global::<ChatData>().deref().clone();
        let tab_key = Self::template_tab_key(self.template_id);
        chat_data.update(cx, |_this, cx| {
            cx.emit(ChatDataEvent::RemoveTab(tab_key));
            cx.emit(ChatDataEvent::OpenTemplateList);
        });
        window.push_notification(
            Notification::new()
                .title(delete_success)
                .with_type(NotificationType::Success),
            cx,
        );
    }

    fn open_delete_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (delete_title, delete_message, cancel_label, delete_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("dialog-delete-template-title"),
                i18n.t("dialog-delete-template-message"),
                i18n.t("button-cancel"),
                i18n.t("button-delete"),
            )
        };
        let this = cx.entity().downgrade();
        window.open_dialog(cx, {
            let this = this.clone();
            move |dialog, _window, _cx| {
                let this = this.clone();
                dialog
                    .title(delete_title.clone())
                    .child(Label::new(delete_message.clone()))
                    .footer({
                        let cancel_label = cancel_label.clone();
                        let delete_label = delete_label.clone();
                        move |_dialog, _state, _window, _cx| {
                            vec![
                                Button::new("cancel").label(cancel_label.clone()).on_click(
                                    |_, window, cx| {
                                        window.close_dialog(cx);
                                    },
                                ),
                                Button::new("confirm-delete")
                                    .danger()
                                    .label(delete_label.clone())
                                    .on_click({
                                        let this = this.clone();
                                        move |_, window, cx| {
                                            window.close_dialog(cx);
                                            let _ = this.update(cx, |view, cx| {
                                                view.delete_template(window, cx);
                                            });
                                        }
                                    }),
                            ]
                        }
                    })
            }
        });
    }
}

impl Render for TemplateDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (edit_label, delete_label, load_template_failed) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("button-edit"),
                i18n.t("button-delete"),
                i18n.t("notify-load-template-failed"),
            )
        };
        v_flex()
            .size_full()
            .child(
                h_flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("template-edit")
                            .primary()
                            .label(edit_label)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_edit_dialog(window, cx);
                            })),
                    )
                    .child(
                        Button::new("template-delete")
                            .danger()
                            .label(delete_label)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.open_delete_dialog(window, cx);
                            })),
                    ),
            )
            .child(match &self.template {
                Ok(template) => render_template_detail(template, window, cx),
                Err(err) => v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(Label::new(format!("{load_template_failed}: {err}")).text_sm())
                    .into_any_element(),
            })
    }
}

fn render_template_detail(
    template: &ConversationTemplate,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let i18n = cx.global::<I18n>();
    let mode_tag = match template.mode {
        Mode::Contextual => Tag::primary(),
        Mode::Single => Tag::info(),
        Mode::AssistantOnly => Tag::success(),
    }
    .outline()
    .flex_initial()
    .child(template.mode.to_string());

    let base_items = {
        let items = vec![
            DescriptionItem::new(i18n.t("field-id")).value(template.id.to_string()),
            DescriptionItem::new(i18n.t("field-name")).value(template.name.clone()),
            DescriptionItem::new(i18n.t("field-icon")).value(template.icon.clone()),
            DescriptionItem::new(i18n.t("field-mode"))
                .value(div().flex().child(mode_tag).into_any_element()),
            DescriptionItem::new(i18n.t("field-adapter")).value(template.adapter.clone()),
            DescriptionItem::new(i18n.t("field-prompts")).value(template.prompts.len().to_string()),
        ];
        items
    };

    let detail_items = description_items_by_adapter(template)
        .unwrap_or_else(|_| description_items_default(template));
    let merged_items = {
        let mut items = base_items;
        if !items.is_empty() && !detail_items.is_empty() {
            items.push(DescriptionItem::Divider);
        }
        items.extend(detail_items);
        items.push(
            DescriptionItem::new(i18n.t("field-description"))
                .value(template.description.clone().unwrap_or("-".to_string()))
                .span(3),
        );
        items
    };

    div()
        .id(template.id)
        .flex_1()
        .gap_3()
        .px_4()
        .child(Label::new(i18n.t("section-information")).text_lg())
        .child(
            div().child(
                DescriptionList::new()
                    .columns(3)
                    .children(merged_items)
                    .layout(Axis::Vertical),
            ),
        )
        .child(Label::new(i18n.t("field-prompts")).text_lg())
        .child(
            div().id("template-prompts").children(
                template
                    .prompts
                    .iter()
                    .enumerate()
                    .map(|(index, prompt)| {
                        render_prompt_message(template.id, index, prompt, window, cx)
                    })
                    .collect::<Vec<_>>(),
            ),
        )
        .child(div().h_4())
        .overflow_hidden()
        .overflow_y_scrollbar()
        .into_any_element()
}

fn render_prompt_message(
    template_id: i32,
    index: usize,
    prompt: &crate::database::ConversationTemplatePrompt,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let text_id: SharedString = format!("template-prompt-{template_id}-{index}").into();
    v_flex()
        .child(
            h_flex()
                .items_start()
                .gap_2()
                .child(
                    Avatar::new()
                        .name(prompt.role.to_string())
                        .src(prompt_avatar(prompt.role))
                        .with_size(px(32.)),
                )
                .child(
                    TextView::markdown(text_id, &prompt.prompt, window, cx)
                        .selectable(true)
                        .flex_1()
                        .overflow_x_hidden(),
                ),
        )
        .child(Divider::horizontal().my_2().ml(px(40.)))
        .into_any_element()
}

fn prompt_avatar(role: Role) -> &'static str {
    match role {
        Role::Developer => "png/system.png",
        Role::User => "jpg/user.jpg",
        Role::Assistant => "jpg/assistant.jpg",
    }
}
