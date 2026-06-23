use std::{
    path::{Path, PathBuf},
    rc::Rc,
};

use ai_chat_core::ProjectId;
use ai_chat_db::ProjectRecord;
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt as NotificationWindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::ListState,
    notification::{Notification, NotificationType},
    select::SelectItem,
    v_flex,
};
use tracing::{Level, event};

use crate::{
    components::{
        chat_form::{ChatForm, ChatFormEvent, ChatFormSkillCompletionPlacement, ChatFormSubmit},
        picker::{PickerListDelegate, PickerPopoverConfig, PickerSection, picker_popover},
    },
    foundation::{I18n, assets::IconName},
    state,
};

const PROJECT_BAR_VISIBLE_HEIGHT: f32 = 42.;
const PROJECT_BAR_OVERLAP: f32 = 16.;
const PROJECT_PICKER_TRIGGER_SIZE: f32 = 28.;
const PROJECT_PICKER_TRIGGER_RADIUS: f32 = 999.;

pub(crate) struct NewConversationPage {
    chat_form: Entity<ChatForm>,
    projects: Result<Vec<ProjectRecord>, String>,
    selected_project_id: Option<ProjectId>,
    project_picker_open: bool,
    project_picker: Entity<ListState<PickerListDelegate<ProjectPickerOption>>>,
    _subscriptions: Vec<Subscription>,
}

impl NewConversationPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let chat_form = cx.new(|cx| {
            let mut chat_form = ChatForm::new(window, cx);
            chat_form.set_skill_completion_placement(ChatFormSkillCompletionPlacement::BelowForm);
            chat_form
        });
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
        let sections = project_sections(projects.as_ref().map(Vec::as_slice).unwrap_or(&[]), cx);
        let selected_value = project_picker_value(selected_project_id.as_ref());
        let selected_ix = PickerListDelegate::selected_index_for(&sections, Some(&selected_value));
        let empty_label = cx.global::<I18n>().t("new-conversation-project-empty");
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
                    empty_label.into(),
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
            |page, _chat_form, event: &ChatFormEvent, window, cx| match event {
                ChatFormEvent::SendRequested(submit) => {
                    page.submit_new_conversation((**submit).clone(), window, cx);
                }
                ChatFormEvent::StopRequested => {}
                ChatFormEvent::AddRequested => {}
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
            project_picker_open: false,
            project_picker,
            _subscriptions: vec![project_subscription, chat_form_subscription],
        }
    }

    pub(crate) fn focus_primary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project_picker_open {
            self.project_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
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
                    self.project_picker_open = false;
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
        self.project_picker_open = open;
        if open {
            self.sync_project_picker(window, cx);
            self.project_picker
                .update(cx, |picker, cx| picker.focus(window, cx));
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
        submit: ChatFormSubmit,
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
            trigger_kind: ai_chat_core::AgentRunTriggerKind::User,
        };
        match state::conversations::create_conversation(request, cx) {
            Ok(created) => {
                let conversation_id = created.record.conversation.id.clone();
                self.chat_form.update(cx, |chat_form, cx| {
                    chat_form.clear_after_submit(cx);
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
        let sections =
            project_sections(self.projects.as_ref().map(Vec::as_slice).unwrap_or(&[]), cx);
        let selected_value = project_picker_value(self.selected_project_id.as_ref());

        self.project_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker
                .delegate_mut()
                .set_selected_value(Some(selected_value));
            let selected_ix = picker.delegate().selected_index();
            picker.set_selected_index(selected_ix, window, cx);
        });

        cx.notify();
    }

    fn render_project_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let label = {
            let i18n = cx.global::<I18n>();
            match &self.projects {
                Err(_) => i18n.t("new-conversation-project-load-failed"),
                Ok(_) => self
                    .selected_project()
                    .map(|project| project.display_name.clone())
                    .unwrap_or_else(|| i18n.t("new-conversation-project-none")),
            }
        };
        let icon = if self.selected_project_id.is_some() {
            IconName::FolderOpen
        } else {
            IconName::FolderX
        };
        let search_placeholder = cx.global::<I18n>().t("new-conversation-project-search");
        let footer = self.render_project_picker_footer(cx);

        picker_popover(
            cx,
            PickerPopoverConfig {
                id: "new-conversation-project-popover",
                open: self.project_picker_open,
                trigger: project_picker_trigger(
                    "new-conversation-project-trigger",
                    icon,
                    label,
                    self.project_picker_open,
                    cx,
                ),
                list: self.project_picker.clone(),
                width: px(320.),
                max_height: rems(18.).into(),
                search_placeholder: Some(search_placeholder.into()),
                footer: Some(footer),
                on_open_change: cx.listener(|page, open: &bool, window, cx| {
                    page.set_project_picker_open(*open, window, cx);
                }),
            },
        )
    }

    fn render_project_picker_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .border_t_1()
            .border_color(cx.theme().border)
            .p_1()
            .child(
                Button::new("new-conversation-add-project")
                    .ghost()
                    .icon(IconName::FolderPlus)
                    .label(cx.global::<I18n>().t("button-add-project"))
                    .small()
                    .w_full()
                    .on_click(cx.listener(|page, _, window, cx| {
                        page.open_add_project_prompt(window, cx);
                    })),
            )
            .into_any_element()
    }

    fn render_project_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("new-conversation-project-bar")
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .w_full()
            .h(px(PROJECT_BAR_VISIBLE_HEIGHT + PROJECT_BAR_OVERLAP))
            .pt(px(PROJECT_BAR_OVERLAP))
            .px_3()
            .items_center()
            .rounded_tl(px(0.))
            .rounded_tr(px(0.))
            .rounded_bl(px(25.))
            .rounded_br(px(25.))
            .bg(cx.theme().muted)
            .text_color(cx.theme().muted_foreground)
            .border_1()
            .border_color(cx.theme().border.opacity(0.35))
            .child(self.render_project_selector(cx))
    }

    fn render_chat_form_layer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("new-conversation-chat-form-layer")
            .w_full()
            .rounded(px(25.))
            .bg(cx.theme().background.blend(cx.theme().input_background()))
            .child(self.chat_form.clone())
    }

    fn render_composer_stack(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("new-conversation-composer-stack")
            .w_full()
            .relative()
            .pb(px(PROJECT_BAR_VISIBLE_HEIGHT))
            .child(self.render_project_bar(cx))
            .child(self.render_chat_form_layer(cx))
    }
}

impl Render for NewConversationPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = new_conversation_title(cx.global::<I18n>());

        v_flex()
            .id("ai-chat2-new-conversation-page")
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
                    .child(self.render_composer_stack(cx)),
            )
    }
}

#[derive(Clone, Debug)]
struct ProjectPickerOption {
    value: ProjectPickerValue,
    kind: ProjectPickerOptionKind,
}

#[derive(Clone, Debug)]
enum ProjectPickerOptionKind {
    NoProject { label: SharedString },
    Project(ProjectRecord),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ProjectPickerValue {
    NoProject,
    Project(ProjectId),
}

impl ProjectPickerOption {
    fn no_project(label: impl Into<SharedString>) -> Self {
        Self {
            value: ProjectPickerValue::NoProject,
            kind: ProjectPickerOptionKind::NoProject {
                label: label.into(),
            },
        }
    }

    fn project(project: ProjectRecord) -> Self {
        Self {
            value: ProjectPickerValue::Project(project.id.clone()),
            kind: ProjectPickerOptionKind::Project(project),
        }
    }
}

impl SelectItem for ProjectPickerOption {
    type Value = ProjectPickerValue;

    fn title(&self) -> SharedString {
        match &self.kind {
            ProjectPickerOptionKind::NoProject { label } => label.clone(),
            ProjectPickerOptionKind::Project(project) => project.display_name.clone().into(),
        }
    }

    fn render(&self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        match &self.kind {
            ProjectPickerOptionKind::NoProject { label } => h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::FolderX)
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(Label::new(label.clone()).text_sm().truncate())
                .into_any_element(),
            ProjectPickerOptionKind::Project(project) => h_flex()
                .w_full()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::FolderOpen)
                        .size_4()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    Label::new(project.display_name.clone())
                        .text_sm()
                        .truncate(),
                )
                .into_any_element(),
        }
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        match &self.kind {
            ProjectPickerOptionKind::NoProject { label } => {
                label.as_ref().to_lowercase().contains(&query)
            }
            ProjectPickerOptionKind::Project(project) => {
                project.display_name.to_lowercase().contains(&query)
                    || project.path.to_lowercase().contains(&query)
            }
        }
    }
}

fn load_projects(cx: &App) -> ai_chat_db::Result<Vec<ProjectRecord>> {
    state::projects::normal_projects(cx)
}

fn project_sections(
    projects: &[ProjectRecord],
    cx: &App,
) -> Vec<PickerSection<ProjectPickerOption>> {
    let i18n = cx.global::<I18n>();
    let mut items = projects
        .iter()
        .cloned()
        .map(ProjectPickerOption::project)
        .collect::<Vec<_>>();
    items.push(ProjectPickerOption::no_project(
        i18n.t("new-conversation-project-none"),
    ));

    vec![PickerSection::untitled(items)]
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

fn project_picker_value(project_id: Option<&ProjectId>) -> ProjectPickerValue {
    project_id
        .cloned()
        .map(ProjectPickerValue::Project)
        .unwrap_or(ProjectPickerValue::NoProject)
}

fn project_by_id<'a>(projects: &'a [ProjectRecord], id: &ProjectId) -> Option<&'a ProjectRecord> {
    projects.iter().find(|project| &project.id == id)
}

fn new_conversation_title(i18n: &I18n) -> String {
    i18n.t("new-conversation-title")
}

fn project_picker_trigger(
    id: &'static str,
    icon: IconName,
    label: impl Into<SharedString>,
    open: bool,
    cx: &App,
) -> Button {
    let foreground = cx.theme().muted_foreground;
    let hover_foreground = cx.theme().foreground.opacity(0.78);
    let hover_background = cx.theme().foreground.opacity(0.06);
    let active_background = cx.theme().foreground.opacity(0.08);

    Button::new(id)
        .text()
        .with_size(px(PROJECT_PICKER_TRIGGER_SIZE))
        .h(px(PROJECT_PICKER_TRIGGER_SIZE))
        .px(px(8.))
        .py(px(0.))
        .rounded(px(PROJECT_PICKER_TRIGGER_RADIUS))
        .text_color(foreground)
        .hover(move |this| this.bg(hover_background).text_color(hover_foreground))
        .when(open, |this| {
            this.bg(active_background).text_color(hover_foreground)
        })
        .child(
            h_flex()
                .items_center()
                .min_w_0()
                .gap_1p5()
                .child(Icon::new(icon).size_4())
                .child(
                    Label::new(label.into())
                        .text_sm()
                        .font_medium()
                        .whitespace_nowrap()
                        .truncate(),
                )
                .child(
                    Icon::new(if open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .size_3(),
                ),
        )
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
            "What would you like AI Chat to help with?"
        );
        assert_eq!(
            new_conversation_title(&I18n::for_locale_tag("zh-CN")),
            "今天想让 AI Chat 帮你做什么？"
        );
    }
}
