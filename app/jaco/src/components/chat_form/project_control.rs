use crate::{
    components::picker::{PickerListDelegate, PickerSection},
    foundation::assets::IconName,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::ListState,
    select::SelectItem,
};
use jaco_core::ProjectId;
use jaco_db::ProjectRecord;

const PROJECT_PICKER_TRIGGER_SIZE: f32 = 28.;
const PROJECT_PICKER_TRIGGER_RADIUS: f32 = 999.;

#[derive(Clone, Debug)]
pub(crate) enum ProjectPickerOptionKind {
    NoProject { label: SharedString },
    Project(ProjectRecord),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProjectPickerValue {
    NoProject,
    Project(ProjectId),
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectPickerOption {
    pub(crate) value: ProjectPickerValue,
    pub(crate) kind: ProjectPickerOptionKind,
}

impl ProjectPickerOption {
    pub(crate) fn no_project(label: impl Into<SharedString>) -> Self {
        Self {
            value: ProjectPickerValue::NoProject,
            kind: ProjectPickerOptionKind::NoProject {
                label: label.into(),
            },
        }
    }

    pub(crate) fn project(project: ProjectRecord) -> Self {
        Self {
            value: ProjectPickerValue::Project(project.id.clone()),
            kind: ProjectPickerOptionKind::Project(project),
        }
    }

    pub(crate) fn trigger_presentation(&self) -> (SharedString, IconName) {
        match &self.kind {
            ProjectPickerOptionKind::NoProject { label } => (label.clone(), IconName::FolderX),
            ProjectPickerOptionKind::Project(project) => {
                (project.display_name.clone().into(), IconName::FolderOpen)
            }
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

pub(crate) struct ProjectControlState {
    pub(crate) open: bool,
    pub(crate) picker: Entity<ListState<PickerListDelegate<ProjectPickerOption>>>,
}

pub(crate) fn project_picker_trigger(
    id: &'static str,
    icon: IconName,
    label: impl Into<SharedString>,
    open: bool,
    cx: &App,
) -> Button {
    let foreground = cx.theme().muted_foreground;
    let hover_foreground = cx.theme().foreground.opacity(0.78);
    let active_background = cx.theme().foreground.opacity(0.08);

    Button::new(id)
        .ghost()
        .with_size(px(PROJECT_PICKER_TRIGGER_SIZE))
        .h(px(PROJECT_PICKER_TRIGGER_SIZE))
        .px(px(8.))
        .py(px(0.))
        .rounded(px(PROJECT_PICKER_TRIGGER_RADIUS))
        .text_color(foreground)
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

pub(crate) fn project_sections(
    projects: &[ProjectRecord],
    none_label: impl Into<SharedString>,
) -> Vec<PickerSection<ProjectPickerOption>> {
    let mut items = projects
        .iter()
        .cloned()
        .map(ProjectPickerOption::project)
        .collect::<Vec<_>>();
    items.push(ProjectPickerOption::no_project(none_label));
    vec![PickerSection::untitled(items)]
}

pub(crate) fn project_picker_value(project_id: Option<&ProjectId>) -> ProjectPickerValue {
    project_id
        .cloned()
        .map(ProjectPickerValue::Project)
        .unwrap_or(ProjectPickerValue::NoProject)
}
