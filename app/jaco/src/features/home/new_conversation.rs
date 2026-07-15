use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use gpui::*;
use gpui_component::{
    ActiveTheme, StyledExt, WindowExt as NotificationWindowExt,
    label::Label,
    list::ListState,
    notification::{Notification, NotificationType},
    v_flex,
};
use jaco_core::ProjectId;
use jaco_db::ProjectRecord;
use tracing::{Level, event};

use crate::{
    components::{
        chat_form::{
            ProjectControlState, ProjectPickerOption, ProjectPickerOptionKind,
            project_picker_value, project_sections,
        },
        chat_input::{
            ChatFormSkillCompletionPlacement, ChatInputController, ChatInputEvent, ChatInputSubmit,
        },
        picker::PickerListDelegate,
    },
    foundation::{I18n, assets::IconName},
    state,
};

pub(crate) struct NewConversationPage {
    chat_form: Entity<ChatInputController>,
    projects: Result<Vec<ProjectRecord>, String>,
    selected_project_id: Option<ProjectId>,
    project: Entity<ProjectControlState>,
    _subscriptions: Vec<Subscription>,
}

impl NewConversationPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let projects = load_projects(cx);
        let selected_project_id = projects
            .as_ref()
            .ok()
            .and_then(|projects| initial_project_id(projects, default_project_id(cx).as_ref()));
        let selected_project = projects.as_ref().ok().and_then(|projects| {
            selected_project_id
                .as_ref()
                .and_then(|id| project_by_id(projects, id))
        });
        let state = cx.entity().downgrade();
        let empty_label = cx.global::<I18n>().t("new-conversation-project-empty");
        let none_label = cx.global::<I18n>().t("new-conversation-project-none");
        let sections = project_sections(
            projects.as_ref().map(Vec::as_slice).unwrap_or(&[]),
            none_label,
        );
        let selected_value = project_picker_value(selected_project_id.as_ref());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, Some(&selected_value));
        let confirm = Rc::new({
            let state = state.clone();
            move |option: ProjectPickerOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |page, cx| {
                    page.select_project_option(option, window, cx);
                });
            }
        });
        let cancel = Rc::new({
            let state = state.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |page, cx| {
                    page.set_project_picker_open(false, window, cx);
                });
            }
        });
        let project_picker = cx.new(|cx| {
            let mut picker = ListState::new(
                PickerListDelegate::new(
                    sections,
                    Some(selected_value),
                    empty_label.clone().into(),
                    confirm,
                    cancel,
                ),
                window,
                cx,
            )
            .searchable(true);
            picker.set_selected_index(selected_ix, window, cx);
            picker
        });
        let (selected_label, icon) =
            project_control_presentation(&projects, selected_project_id.as_ref(), cx);
        let project = cx.new(|_| ProjectControlState {
            selected_label,
            placeholder: empty_label.into(),
            icon,
            open: false,
            picker: project_picker,
        });
        let chat_form = cx.new(|cx| {
            let mut chat_form = ChatInputController::new_with_project(project.clone(), window, cx);
            chat_form
                .set_skill_completion_placement(ChatFormSkillCompletionPlacement::BelowForm, cx);
            chat_form
        });
        let project_catalog = state::projects::catalog(cx);
        let project_subscription = cx.subscribe(
            &project_catalog,
            |page, _catalog, _event: &state::projects::ProjectCatalogEvent, cx| {
                page.reload_projects_from_catalog(cx);
            },
        );
        let chat_form_subscription = cx.subscribe_in(
            &chat_form,
            window,
            |page, _chat_form, event: &ChatInputEvent, window, cx| match event {
                ChatInputEvent::SendRequested(submit) => {
                    page.submit_new_conversation((**submit).clone(), window, cx);
                }
                ChatInputEvent::StopRequested => {}
                ChatInputEvent::AddRequested => {}
                ChatInputEvent::AddProjectRequested => {
                    page.open_add_project_prompt(window, cx);
                }
            },
        );

        if let Some(project) = selected_project {
            chat_form.update(cx, |chat_form, cx| {
                chat_form.refresh_skill_catalog(Some(Path::new(&project.path)), cx);
            });
        }

        Self {
            chat_form,
            projects: projects.map_err(|err| err.to_string()),
            selected_project_id,
            project,
            _subscriptions: vec![project_subscription, chat_form_subscription],
        }
    }

    pub(crate) fn focus_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project.read(cx).open {
            let picker = self.project.read(cx).picker.clone();
            picker.update(cx, |picker, cx| picker.focus(window, cx));
            return;
        }

        self.chat_form
            .update(cx, |chat_form, cx| chat_form.focus_composer(window, cx));
    }

    pub(crate) fn select_project_id_from_sidebar(
        &mut self,
        project_id: ProjectId,
        cx: &mut Context<Self>,
    ) {
        match load_projects(cx) {
            Ok(projects) => {
                let project = project_by_id(&projects, &project_id).cloned();
                self.projects = Ok(projects);
                if let Some(project) = project {
                    let save_result = state::config::update_app_settings(cx, |payload| {
                        payload.default_project_id = Some(project.id.clone());
                    });
                    if let Err(err) = save_result {
                        event!(Level::ERROR, error = ?err, "save sidebar selected project failed");
                    }
                    self.selected_project_id = Some(project.id.clone());
                    self.chat_form.update(cx, |chat_form, cx| {
                        chat_form.refresh_skill_catalog(Some(Path::new(&project.path)), cx);
                    });
                    self.project.update(cx, |project, cx| {
                        project.open = false;
                        cx.notify();
                    });
                }
            }
            Err(err) => {
                self.projects = Err(err.to_string());
            }
        }
        cx.notify();
    }

    fn selected_project(&self) -> Option<&ProjectRecord> {
        let projects = self.projects.as_ref().ok()?;
        let selected_project_id = self.selected_project_id.as_ref()?;
        project_by_id(projects, selected_project_id)
    }

    fn reload_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match load_projects(cx) {
            Ok(projects) => {
                let selected_project_id = selected_or_initial_project_id(
                    &projects,
                    self.selected_project_id.as_ref(),
                    default_project_id(cx).as_ref(),
                );
                self.projects = Ok(projects);
                self.selected_project_id = selected_project_id;
                self.refresh_skill_catalog_for_selected(cx);
                self.sync_project_picker(window, cx);
                true
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-load-projects-failed");
                let message = err.to_string();
                self.projects = Err(message.clone());
                push_project_notification(window, cx, title, message, NotificationType::Error);
                self.sync_project_picker(window, cx);
                false
            }
        }
    }

    fn reload_projects_from_catalog(&mut self, cx: &mut Context<Self>) {
        match load_projects(cx) {
            Ok(projects) => {
                let selected_project_id = selected_or_initial_project_id(
                    &projects,
                    self.selected_project_id.as_ref(),
                    default_project_id(cx).as_ref(),
                );
                self.projects = Ok(projects);
                self.selected_project_id = selected_project_id;
                self.refresh_skill_catalog_for_selected(cx);
            }
            Err(err) => {
                self.projects = Err(err.to_string());
            }
        }
        cx.notify();
    }

    fn refresh_skill_catalog_for_selected(&mut self, cx: &mut Context<Self>) {
        let selected_path = self.selected_project().map(|project| project.path.clone());
        self.chat_form.update(cx, |chat_form, cx| {
            chat_form.refresh_skill_catalog(selected_path.as_deref().map(Path::new), cx);
        });
    }

    fn set_project_picker_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.project.update(cx, |project, cx| {
            project.open = open;
            cx.notify();
        });
        if open {
            self.sync_project_picker(window, cx);
            let picker = self.project.read(cx).picker.clone();
            picker.update(cx, |picker, cx| picker.focus(window, cx));
        }
        cx.notify();
    }

    fn select_project_option(
        &mut self,
        option: ProjectPickerOption,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match option.kind {
            ProjectPickerOptionKind::NoProject { .. } => self.select_no_project(false, window, cx),
            ProjectPickerOptionKind::Project(project) => {
                self.select_project(project, false, window, cx)
            }
        }
    }

    fn select_no_project(
        &mut self,
        sync_picker: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match state::config::update_app_settings(cx, |payload| {
            payload.default_project_id = None;
        }) {
            Ok(_) => {
                self.selected_project_id = None;
                self.chat_form.update(cx, |chat_form, cx| {
                    chat_form.refresh_skill_catalog(None, cx);
                });
                if sync_picker {
                    self.sync_project_picker(window, cx);
                }
                self.set_project_picker_open(false, window, cx);
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-save-settings-failed");
                push_project_notification(
                    window,
                    cx,
                    title,
                    err.to_string(),
                    NotificationType::Error,
                );
                if sync_picker {
                    self.sync_project_picker(window, cx);
                }
            }
        }
    }

    fn select_project(
        &mut self,
        project: ProjectRecord,
        sync_picker: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project_id = project.id.clone();
        match state::config::update_app_settings(cx, |payload| {
            payload.default_project_id = Some(project_id.clone());
        }) {
            Ok(_) => {
                self.selected_project_id = Some(project_id);
                self.chat_form.update(cx, |chat_form, cx| {
                    chat_form.refresh_skill_catalog(Some(Path::new(&project.path)), cx);
                });
                if sync_picker {
                    self.sync_project_picker(window, cx);
                }
                self.set_project_picker_open(false, window, cx);
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-save-settings-failed");
                push_project_notification(
                    window,
                    cx,
                    title,
                    err.to_string(),
                    NotificationType::Error,
                );
                if sync_picker {
                    self.sync_project_picker(window, cx);
                }
            }
        }
    }

    fn open_add_project_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_project_picker_open(false, window, cx);
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
                    event!(Level::ERROR, error = ?err, "add project from new conversation failed");
                }
            })
            .detach();
    }

    fn submit_new_conversation(
        &mut self,
        submit: ChatInputSubmit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let request = state::conversations::CreateConversationRequest {
            project_id: self.selected_project_id.clone(),
            content_parts: submit.composer.content_parts.clone(),
            attachments: submit.composer.attachments.clone(),
            title_seed: submit.composer.text.clone(),
            skill_requests: submit.composer.skill_requests.clone(),
            provider_model: submit.provider_model,
            reasoning_selection: submit.reasoning_selection,
            approval_mode: submit.approval_mode,
            prompt_id: None,
            prompt_snapshot: None,
            trigger_kind: jaco_core::AgentRunTriggerKind::User,
        };
        match state::conversations::create_conversation(request, cx) {
            Ok(created) => {
                let conversation_id = created.record.conversation.id.clone();
                self.chat_form.update(cx, |chat_form, cx| {
                    chat_form.clear_after_submit(window, cx);
                });
                state::workspace::workspace(cx).update(cx, |workspace, cx| {
                    workspace.reload_sidebar(cx);
                    workspace.open_conversation(conversation_id.clone(), cx);
                });
                state::conversation_runtime::runtime(cx).update(cx, |runtime, cx| {
                    runtime.start_run(created.run_request, window, cx);
                });
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("new-conversation-submit-failed");
                push_project_notification(
                    window,
                    cx,
                    title,
                    err.to_string(),
                    NotificationType::Error,
                );
            }
        }
    }

    fn insert_selected_project(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match state::projects::insert_existing_folder_project(cx, path) {
            Ok(result) => {
                let project = result.project.clone();
                let _ = self.reload_projects(window, cx);
                self.select_project(project.clone(), true, window, cx);
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
                push_project_notification(window, cx, title, project.path, notification_type);
            }
            Err(err) => {
                let title = cx.global::<I18n>().t("notify-add-project-failed");
                push_project_notification(
                    window,
                    cx,
                    title,
                    err.to_string(),
                    NotificationType::Error,
                );
            }
        }
    }

    fn sync_project_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let none_label = cx.global::<I18n>().t("new-conversation-project-none");
        let sections = project_sections(
            self.projects.as_ref().map(Vec::as_slice).unwrap_or(&[]),
            none_label.clone(),
        );
        let selected_value = project_picker_value(self.selected_project_id.as_ref());

        let picker = self.project.read(cx).picker.clone();
        picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker
                .delegate_mut()
                .set_selected_value(Some(selected_value));
            let selected_ix = picker.delegate().selected_index();
            picker.set_selected_index(selected_ix, window, cx);
        });

        let (selected_label, icon) =
            project_control_presentation(&self.projects, self.selected_project_id.as_ref(), cx);
        self.project.update(cx, |project, cx| {
            project.selected_label = selected_label;
            project.placeholder = none_label.clone().into();
            project.icon = icon;
            cx.notify();
        });

        cx.notify();
    }
}

impl Render for NewConversationPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = new_conversation_title(cx.global::<I18n>());

        v_flex()
            .id("jaco-new-conversation-page")
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .items_center()
            .justify_center()
            .px_8()
            .py_12()
            .child(
                v_flex()
                    .w_full()
                    .max_w(px(780.))
                    .items_center()
                    .gap(px(28.))
                    .child(
                        Label::new(title)
                            .text_center()
                            .text_size(px(30.))
                            .font_medium()
                            .text_color(cx.theme().foreground),
                    )
                    .child(self.chat_form.clone()),
            )
    }
}

fn load_projects(cx: &App) -> jaco_db::Result<Vec<ProjectRecord>> {
    state::projects::normal_projects(cx)
}

fn default_project_id(cx: &App) -> Option<ProjectId> {
    state::config::app_settings(cx)
        .default_project_id()
        .cloned()
}

fn selected_or_initial_project_id(
    projects: &[ProjectRecord],
    selected_project_id: Option<&ProjectId>,
    default_project_id: Option<&ProjectId>,
) -> Option<ProjectId> {
    selected_project_id
        .filter(|id| project_by_id(projects, id).is_some())
        .cloned()
        .or_else(|| initial_project_id(projects, default_project_id))
}

fn initial_project_id(
    projects: &[ProjectRecord],
    default_project_id: Option<&ProjectId>,
) -> Option<ProjectId> {
    default_project_id
        .and_then(|id| project_by_id(projects, id))
        .map(|project| project.id.clone())
}

fn project_by_id<'a>(projects: &'a [ProjectRecord], id: &ProjectId) -> Option<&'a ProjectRecord> {
    projects.iter().find(|project| &project.id == id)
}

fn project_control_presentation<E>(
    projects: &Result<Vec<ProjectRecord>, E>,
    selected_project_id: Option<&ProjectId>,
    cx: &App,
) -> (SharedString, IconName) {
    let i18n = cx.global::<I18n>();
    match projects {
        Err(_) => (
            i18n.t("new-conversation-project-load-failed").into(),
            IconName::FolderX,
        ),
        Ok(projects) => match selected_project_id.and_then(|id| project_by_id(projects, id)) {
            Some(project) => (project.display_name.clone().into(), IconName::FolderOpen),
            None => (
                i18n.t("new-conversation-project-none").into(),
                IconName::FolderX,
            ),
        },
    }
}

fn new_conversation_title(i18n: &I18n) -> String {
    i18n.t("new-conversation-title")
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
        event!(Level::ERROR, error = ?err, "push new conversation project notification failed");
    }
}

#[cfg(test)]
mod tests {
    use super::new_conversation_title;
    use crate::foundation::I18n;

    #[test]
    fn new_conversation_title_uses_app_neutral_copy() {
        assert_eq!(
            new_conversation_title(&I18n::english_for_test()),
            "What would you like Jaco to help with?"
        );
        assert_eq!(
            new_conversation_title(&I18n::for_locale_tag("zh-CN")),
            "今天想让 Jaco 帮你做什么？"
        );
    }
}
