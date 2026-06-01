use crate::{
    database,
    foundation::{I18n, assets::IconName},
};
use ai_chat_core::{ProjectKind, ProjectMetadata};
use ai_chat_db::{NewProject, ProjectRecord};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::Button,
    h_flex,
    label::Label,
    notification::{Notification, NotificationType},
    v_flex,
};
use std::path::{Path, PathBuf};
use tracing::{Level, event};

use super::{layout::settings_empty_message, push_settings_error};

pub(super) struct ProjectsSettingsPage {
    projects: Result<Vec<ProjectRecord>, String>,
}

impl ProjectsSettingsPage {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            projects: Self::load_projects(cx).map_err(|err| err.to_string()),
        }
    }

    fn load_projects(cx: &App) -> ai_chat_db::Result<Vec<ProjectRecord>> {
        let projects = database::repository(cx).list_projects()?;
        Ok(visible_projects(&projects))
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
        let project_path = path.display().to_string();
        let repository = database::repository(cx);

        match repository.get_project_by_path(&project_path) {
            Ok(Some(_)) => {
                let _ = self.reload_projects(window, cx);
                let title = cx.global::<I18n>().t("notify-project-already-exists");
                push_project_notification(
                    window,
                    cx,
                    title,
                    project_path,
                    NotificationType::Warning,
                );
                return;
            }
            Ok(None) => {}
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-add-project-failed");
                push_settings_error(window, cx, title, err);
                return;
            }
        }

        let input = NewProject {
            path: project_path,
            display_name: project_display_name(&path),
            kind: ProjectKind::Normal,
            metadata: empty_project_metadata(),
        };

        match repository.insert_project(input) {
            Ok(project) => {
                let _ = self.reload_projects(window, cx);
                let title = cx.global::<I18n>().t("notify-project-added-success");
                push_project_notification(
                    window,
                    cx,
                    title,
                    project.path,
                    NotificationType::Success,
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

pub(super) fn project_display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

fn visible_projects(projects: &[ProjectRecord]) -> Vec<ProjectRecord> {
    projects
        .iter()
        .filter(|project| project_kind_is_visible(project.kind))
        .cloned()
        .collect()
}

fn project_kind_is_visible(kind: ProjectKind) -> bool {
    kind == ProjectKind::Normal
}

fn empty_project_metadata() -> ProjectMetadata {
    ProjectMetadata {
        scratch_reason: None,
        git_root: None,
        last_active_conversation_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{project_display_name, project_kind_is_visible};
    use ai_chat_core::ProjectKind;
    use std::path::Path;

    #[test]
    fn project_display_name_uses_path_last_component() {
        assert_eq!(
            project_display_name(Path::new("/tmp/ai-chat-project")),
            "ai-chat-project"
        );
    }

    #[test]
    fn project_display_name_falls_back_to_full_path() {
        let path = Path::new("/");

        assert_eq!(project_display_name(path), path.display().to_string());
    }

    #[test]
    fn project_settings_show_only_normal_projects() {
        assert!(project_kind_is_visible(ProjectKind::Normal));
        assert!(!project_kind_is_visible(ProjectKind::Scratch));
    }
}
