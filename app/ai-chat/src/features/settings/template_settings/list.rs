use crate::{
    database::{ConversationTemplate, Db},
    errors::AiChatResult,
    foundation::{assets::IconName, i18n::I18n},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};
use std::rc::Rc;

use super::{
    dialogs::{open_delete_template_dialog, open_template_view_dialog},
    form::{open_add_template_dialog, open_edit_template_dialog},
};

pub(crate) struct TemplateSettingsPage {
    search_input: Entity<InputState>,
    templates: Result<Vec<ConversationTemplate>, String>,
    _search_input_subscription: Subscription,
}

impl TemplateSettingsPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(cx.global::<I18n>().t("field-search-template"))
        });
        let _search_input_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);

        Self {
            templates: Self::load_templates(cx).map_err(|err| err.to_string()),
            search_input,
            _search_input_subscription,
        }
    }

    fn load_templates(cx: &mut Context<Self>) -> AiChatResult<Vec<ConversationTemplate>> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::all(conn)
    }

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn filtered_templates(&self, cx: &App) -> Vec<ConversationTemplate> {
        let query = self.current_query(cx);
        filter_templates(self.templates.as_deref().unwrap_or_default(), &query)
    }

    fn on_search_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            cx.notify();
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

    fn reload_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match Self::load_templates(cx) {
            Ok(templates) => {
                self.templates = Ok(templates);
                cx.notify();
                true
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-templates-failed");
                Self::notify_error(title, err.to_string(), window, cx);
                self.templates = Err(err.to_string());
                cx.notify();
                false
            }
        }
    }

    fn open_add_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.entity().downgrade();
        open_add_template_dialog(
            Rc::new(move |window, cx| {
                let _ = page.update(cx, |page, cx| {
                    page.reload_templates(window, cx);
                });
            }),
            window,
            cx,
        );
    }

    fn open_view_dialog(&mut self, template_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        let template = match self.find_template(template_id, cx) {
            Ok(template) => template,
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-template-failed");
                Self::notify_error(title, err.to_string(), window, cx);
                return;
            }
        };
        let page = cx.entity().downgrade();
        let edit_page = page.clone();
        open_template_view_dialog(
            template,
            Rc::new(move |template_id, window, cx| {
                let _ = edit_page.update(cx, |page, cx| {
                    page.open_edit_dialog(template_id, window, cx);
                });
            }),
            Rc::new(move |template_id, window, cx| {
                let _ = page.update(cx, |page, cx| {
                    page.open_delete_dialog(template_id, window, cx);
                });
            }),
            window,
            cx,
        );
    }

    fn open_edit_dialog(&mut self, template_id: i32, window: &mut Window, cx: &mut Context<Self>) {
        let template = match self.find_template(template_id, cx) {
            Ok(template) => template,
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-template-failed");
                Self::notify_error(title, err.to_string(), window, cx);
                return;
            }
        };
        let page = cx.entity().downgrade();
        open_edit_template_dialog(
            template_id,
            template,
            Rc::new(move |window, cx| {
                let _ = page.update(cx, |page, cx| {
                    page.reload_templates(window, cx);
                });
            }),
            window,
            cx,
        );
    }

    fn open_delete_dialog(
        &mut self,
        template_id: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let template = match self.find_template(template_id, cx) {
            Ok(template) => template,
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-template-failed");
                Self::notify_error(title, err.to_string(), window, cx);
                return;
            }
        };
        let page = cx.entity().downgrade();
        open_delete_template_dialog(
            template,
            Rc::new(move |template_id, window, cx| {
                page.update(cx, |page, cx| page.delete_template(template_id, window, cx))
                    .unwrap_or(false)
            }),
            window,
            cx,
        );
    }

    fn find_template(
        &mut self,
        template_id: i32,
        cx: &mut Context<Self>,
    ) -> AiChatResult<ConversationTemplate> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::find(template_id, conn)
    }

    fn delete_template(
        &mut self,
        template_id: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut conn = match cx.global::<Db>().get() {
            Ok(conn) => conn,
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-open-database-failed");
                Self::notify_error(title, err.to_string(), window, cx);
                return false;
            }
        };

        if let Err(err) = ConversationTemplate::delete(template_id, &mut conn) {
            let title = cx.global::<I18n>().t("notify-delete-template-failed");
            Self::notify_error(title, err.to_string(), window, cx);
            return false;
        }

        let _ = self.reload_templates(window, cx);
        window.push_notification(
            Notification::new()
                .title(cx.global::<I18n>().t("notify-template-deleted-success"))
                .with_type(NotificationType::Success),
            cx,
        );
        true
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (reload_label, add_label) = {
            let i18n = cx.global::<I18n>();
            (i18n.t("button-reload"), i18n.t("dialog-add-template-title"))
        };

        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                Input::new(&self.search_input)
                    .flex_1()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground))
                    .cleanable(true),
            )
            .child(
                Button::new("template-settings-reload")
                    .icon(IconName::RefreshCcw)
                    .ghost()
                    .tooltip(reload_label)
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.reload_templates(window, cx);
                    })),
            )
            .child(
                Button::new("template-settings-add")
                    .icon(IconName::Plus)
                    .label(add_label)
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_add_dialog(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_template_row(
        &self,
        template: ConversationTemplate,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (prompts_label, view_label, edit_label, delete_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-prompts"),
                i18n.t("button-view"),
                i18n.t("button-edit"),
                i18n.t("button-delete"),
            )
        };
        let template_id = template.id;
        let prompt_count = format!("{} {}", template.prompts.len(), prompts_label);

        h_flex()
            .id(("template-settings-row", template_id as u64))
            .w_full()
            .items_center()
            .gap_3()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .px_3()
            .py_2()
            .hover(|this| this.bg(cx.theme().accent.opacity(0.45)))
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().border.opacity(0.35))
                    .child(Label::new(template.icon).text_base()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(Label::new(template.name).text_sm().truncate())
                    .when_some(template.description, |this, description| {
                        this.child(
                            Label::new(description)
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .truncate(),
                        )
                    }),
            )
            .child(
                Label::new(prompt_count)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                h_flex()
                    .flex_none()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(("template-settings-view", template_id as u64))
                            .icon(IconName::Eye)
                            .ghost()
                            .tooltip(view_label)
                            .on_click(cx.listener(move |page, _, window, cx| {
                                page.open_view_dialog(template_id, window, cx);
                            })),
                    )
                    .child(
                        Button::new(("template-settings-edit", template_id as u64))
                            .icon(IconName::Edit)
                            .ghost()
                            .tooltip(edit_label)
                            .on_click(cx.listener(move |page, _, window, cx| {
                                page.open_edit_dialog(template_id, window, cx);
                            })),
                    )
                    .child(
                        Button::new(("template-settings-delete", template_id as u64))
                            .icon(IconName::Trash)
                            .danger()
                            .tooltip(delete_label)
                            .on_click(cx.listener(move |page, _, window, cx| {
                                page.open_delete_dialog(template_id, window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_template_list(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let load_failed = cx.global::<I18n>().t("notify-load-templates-failed");
        match &self.templates {
            Err(err) => v_flex()
                .w_full()
                .min_h(px(280.))
                .items_center()
                .justify_center()
                .child(Label::new(format!("{load_failed}: {err}")).text_sm())
                .into_any_element(),
            Ok(_) => {
                let filtered = self.filtered_templates(cx);
                if filtered.is_empty() {
                    v_flex()
                        .w_full()
                        .min_h(px(220.))
                        .items_center()
                        .justify_center()
                        .child(
                            Label::new(cx.global::<I18n>().t("empty-template-search"))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .into_any_element()
                } else {
                    v_flex()
                        .w_full()
                        .gap_2()
                        .children(
                            filtered
                                .into_iter()
                                .map(|template| self.render_template_row(template, cx))
                                .collect::<Vec<_>>(),
                        )
                        .into_any_element()
                }
            }
        }
    }
}

impl Render for TemplateSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_3()
            .child(
                Label::new(cx.global::<I18n>().t("settings-templates-description"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(self.render_toolbar(window, cx))
            .child(self.render_template_list(window, cx))
    }
}

pub(super) fn filter_templates(
    items: &[ConversationTemplate],
    query: &str,
) -> Vec<ConversationTemplate> {
    let query = query.trim();
    if query.is_empty() {
        return items.to_vec();
    }

    items
        .iter()
        .filter(|template| template.matches_search_query(query))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
