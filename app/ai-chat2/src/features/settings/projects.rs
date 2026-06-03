use crate::{
    foundation::{I18n, assets::IconName},
    state,
};
use ai_chat_db::ProjectRecord;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::Button,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};
use std::path::PathBuf;
use tracing::{Level, event};

use super::{layout::settings_empty_message, push_settings_error};

pub(super) struct ProjectsSettingsPage {
    projects: Result<Vec<ProjectRecord>, String>,
    _subscriptions: Vec<Subscription>,
}

impl ProjectsSettingsPage {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let project_catalog = state::projects::catalog(cx);
        let subscription = cx.subscribe(
            &project_catalog,
            |page, _catalog, _event: &state::projects::ProjectCatalogEvent, cx| {
                page.reload_projects_from_catalog(cx);
            },
        );
        Self {
            projects: Self::load_projects(cx).map_err(|err| err.to_string()),
            _subscriptions: vec![subscription],
        }
    }

    fn load_projects(cx: &App) -> ai_chat_db::Result<Vec<ProjectRecord>> {
        state::projects::normal_projects(cx)
    }

    fn reload_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match Self::load_projects(cx) {
            Ok(projects) => {
                self.projects = Ok(projects);
                cx.notify();
                true
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-projects-failed");
                let message = err.to_string();
                self.projects = Err(message.clone());
                push_settings_error(window, cx, title, message);
                cx.notify();
                false
            }
        }
    }

    fn reload_projects_from_catalog(&mut self, cx: &mut Context<Self>) {
        self.projects = Self::load_projects(cx).map_err(|err| err.to_string());
        cx.notify();
    }

    fn open_add_project_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = cx.global::<I18n>().t("button-add-project");
        let failed_title = cx.global::<I18n>().t("notify-add-project-failed");
        let path_prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(title.into()),
        });
        let page = cx.entity().downgrade();

        window
            .spawn(cx, async move |cx| {
                let selected_path = match path_prompt.await {
                    Ok(Ok(Some(paths))) => paths.into_iter().next(),
                    Ok(Ok(None)) => return,
                    Ok(Err(err)) => {
                        push_project_notification_async(
                            cx,
                            failed_title.clone().into(),
                            err.to_string(),
                            NotificationType::Error,
                        );
                        return;
                    }
                    Err(err) => {
                        push_project_notification_async(
                            cx,
                            failed_title.into(),
                            err.to_string(),
                            NotificationType::Error,
                        );
                        return;
                    }
                };
                let Some(path) = selected_path else {
                    return;
                };

                if let Err(err) = page.update_in(cx, |page, window, cx| {
                    page.insert_selected_project(path, window, cx);
                }) {
                    event!(Level::ERROR, error = ?err, "add project after path prompt failed");
                }
            })
            .detach();
    }

    fn insert_selected_project(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match state::projects::insert_existing_folder_project(cx, path) {
            Ok(result) => {
                let _ = self.reload_projects(window, cx);
                let (title, notification_type) = if result.was_existing {
                    (
                        cx.global::<I18n>().t("notify-project-already-exists"),
                        NotificationType::Warning,
                    )
                } else {
                    (
                        cx.global::<I18n>().t("notify-project-added-success"),
                        NotificationType::Success,
                    )
                };
                push_project_notification(
                    window,
                    cx,
                    title,
                    result.project.path,
                    notification_type,
                );
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-add-project-failed");
                push_settings_error(window, cx, title, err);
            }
        }
    }

    fn render_toolbar(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_end()
            .child(
                Button::new("project-settings-add")
                    .icon(IconName::Plus)
                    .label(cx.global::<I18n>().t("button-add-project"))
                    .small()
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_add_project_prompt(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_project_row(&self, project: ProjectRecord, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .min_w_0()
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
                    .child(Icon::new(IconName::Folder).text_color(cx.theme().muted_foreground)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        Label::new(project.display_name)
                            .text_sm()
                            .font_medium()
                            .truncate(),
                    )
                    .child(
                        Label::new(project.path)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            )
            .into_any_element()
    }

    fn render_project_list(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match &self.projects {
            Err(err) => {
                let load_failed = cx.global::<I18n>().t("notify-load-projects-failed");
                settings_empty_message(format!("{load_failed}: {err}"))
            }
            Ok(projects) if projects.is_empty() => {
                settings_empty_message(cx.global::<I18n>().t("empty-projects"))
            }
            Ok(projects) => v_flex()
                .w_full()
                .gap_2()
                .children(
                    projects
                        .iter()
                        .cloned()
                        .map(|project| self.render_project_row(project, cx))
                        .collect::<Vec<_>>(),
                )
                .into_any_element(),
        }
    }
}

impl Render for ProjectsSettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap_3()
            .child(self.render_toolbar(window, cx))
            .child(self.render_project_list(window, cx))
    }
}

fn push_project_notification(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    message: impl Into<SharedString>,
    notification_type: NotificationType,
) {
    window.push_notification(
        Notification::new()
            .title(title.into())
            .message(message.into())
            .with_type(notification_type),
        cx,
    );
}

fn push_project_notification_async(
    cx: &mut AsyncWindowContext,
    title: SharedString,
    message: String,
    notification_type: NotificationType,
) {
    if let Err(err) = cx.window_handle().update(cx, |_, window, cx| {
        push_project_notification(window, cx, title, message, notification_type);
    }) {
        event!(Level::ERROR, error = ?err, "push project settings notification failed");
    }
}
