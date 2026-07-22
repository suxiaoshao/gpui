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
use gpui_store::StoreSelection;
use jaco_core::ProjectId;
use jaco_db::ProjectRecord;
use tracing::{Level, event};

use crate::{
    components::{
        chat_form::{
            ProjectControlState, ProjectPickerOption, ProjectPickerValue, project_picker_value,
            project_sections,
        },
        chat_input::{
            ChatFormSkillCompletionPlacement, ChatInputController, ChatInputEvent, ChatInputSubmit,
        },
        picker::PickerListDelegate,
    },
    foundation::I18n,
    state,
};

pub(crate) struct NewConversationPage {
    chat_form: Entity<ChatInputController>,
    projects: StoreSelection<Vec<ProjectRecord>>,
    selected_project_id: Option<ProjectId>,
    project: Entity<ProjectControlState>,
    _subscriptions: Vec<Subscription>,
}

impl NewConversationPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let projects =
            state::projects::catalog(cx).select_cloned(cx, |snapshot| snapshot.projects());
        let selected_project_id =
            projects.read(|projects| initial_project_id(projects, default_project_id(cx).as_ref()));
        let selected_project = projects.read(|projects| {
            selected_project_id
                .as_ref()
                .and_then(|id| project_by_id(projects, id))
                .cloned()
        });
        let state = cx.entity().downgrade();
        let empty_label = cx.global::<I18n>().t("new-conversation-project-empty");
        let none_label = cx.global::<I18n>().t("new-conversation-project-none");
        let sections = projects.read(|projects| project_sections(projects, none_label));
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
        let project = cx.new(|_| ProjectControlState {
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
        let project_subscription = cx.observe_in(
            &project_catalog.entity(),
            window,
            |_, _catalog, window, cx| {
                cx.defer_in(window, move |page, window, cx| {
                    page.reload_projects_from_catalog(window, cx);
                });
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
            projects,
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self
            .projects
            .read(|projects| project_by_id(projects, &project_id).cloned())
        {
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
            self.sync_project_picker(window, cx);
            self.project.update(cx, |project, cx| {
                project.open = false;
                cx.notify();
            });
        }
        cx.notify();
    }

    fn selected_project(&self) -> Option<ProjectRecord> {
        let selected_project_id = self.selected_project_id.as_ref()?;
        self.projects
            .read(|projects| project_by_id(projects, selected_project_id).cloned())
    }

    fn reload_projects(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.reload_projects_from_catalog(window, cx);
        true
    }

    fn reload_projects_from_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected_project_id = self.projects.read(|projects| {
            selected_or_initial_project_id(
                projects,
                self.selected_project_id.as_ref(),
                default_project_id(cx).as_ref(),
            )
        });
        self.selected_project_id = selected_project_id;
        self.refresh_skill_catalog_for_selected(cx);
        self.sync_project_picker(window, cx);
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
        match option.value {
            ProjectPickerValue::NoProject => self.select_no_project(window, cx),
            ProjectPickerValue::Project(project_id) => {
                if let Some(project) = self
                    .projects
                    .read(|projects| project_by_id(projects, &project_id).cloned())
                {
                    self.select_project(project, window, cx);
                } else {
                    self.sync_project_picker(window, cx);
                }
            }
        }
    }

    fn select_no_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match state::config::update_app_settings(cx, |payload| {
            payload.default_project_id = None;
        }) {
            Ok(_) => {
                self.selected_project_id = None;
                self.chat_form.update(cx, |chat_form, cx| {
                    chat_form.refresh_skill_catalog(None, cx);
                });
                self.sync_project_picker(window, cx);
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
                self.sync_project_picker(window, cx);
            }
        }
    }

    fn select_project(
        &mut self,
        project: ProjectRecord,
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
                self.sync_project_picker(window, cx);
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
                self.sync_project_picker(window, cx);
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
            attachments: submit.attachments.clone(),
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
                self.select_project(project.clone(), window, cx);
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
        let sections = self
            .projects
            .read(|projects| project_sections(projects, none_label.clone()));
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
