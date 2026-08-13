#![allow(dead_code, unused_imports)]

mod controls;
mod project_control;

use std::path::PathBuf;

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, ElementExt, Icon, Sizable, box_shadow,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    menu::{DropdownMenu, PopupMenuItem},
    v_flex,
};

use crate::{
    components::{
        chat::input::{ChatFormSkillCompletionPlacement, ComposerEditor, attachments},
        chat::run_settings,
        picker::{PickerPopover, PickerPopoverConfig},
    },
    features::conversation::attachments::{
        ComposerAttachment, ComposerAttachmentKind, ComposerAttachmentSource,
    },
    foundation::assets::IconName,
};

pub(crate) const SKILL_COMPLETION_GAP: f32 = 6.;
const SKILL_COMPLETION_WINDOW_MARGIN: f32 = 8.;
pub(crate) const SKILL_COMPLETION_MAX_HEIGHT: f32 = 360.;
const PROJECT_BAR_VISIBLE_HEIGHT: f32 = 42.;
const PROJECT_BAR_OVERLAP: f32 = 16.;
const COMPOSER_INPUT_HORIZONTAL_PADDING: f32 = 12.;
const COMPOSER_INPUT_TOP_PADDING: f32 = 12.;
const COMPOSER_INPUT_BOTTOM_MARGIN: f32 = 4.;
const COMPOSER_FOOTER_HORIZONTAL_PADDING: f32 = 8.;
const COMPOSER_FOOTER_BOTTOM_MARGIN: f32 = 8.;

pub(crate) use controls::{
    AddAttachmentControl, AgentRunControlStatus, AgentRunStatusSource, AttachmentControlState,
    ChatFormControls, ControlSlot, PrimaryActionControlState, RunSettingsControls,
};
pub(crate) use project_control::{
    ProjectControlState, ProjectPickerOption, ProjectPickerOptionKind, ProjectPickerValue,
    project_picker_trigger, project_picker_value, project_sections,
};

#[derive(Clone, Debug)]
pub(crate) enum ChatFormUiEvent {
    AddProjectRequested,
    AddAttachmentFilesRequested,
    AddAttachmentFromClipboardRequested,
    ExternalPathsDropped(Vec<PathBuf>),
    OpenAttachmentRequested(ComposerAttachment),
    RemoveAttachmentRequested(u64),
    PrimaryActionRequested,
}

impl EventEmitter<ChatFormUiEvent> for ChatFormState {}

/// Stable interaction and lifecycle state for [`ChatForm`].
///
/// Render-time controls and placement are supplied by the parent whenever it
/// rebuilds `ChatForm`; this entity only retains measured geometry and the
/// subscriptions that keep the view responsive to nested control entities.
pub(crate) struct ChatFormState {
    bounds: Option<Bounds<Pixels>>,
    _subscriptions: Vec<Subscription>,
}

impl ChatFormState {
    /// Creates the backing state for one stable graph of nested control
    /// entities. Owners may change slot availability and other render-time
    /// properties while reusing this state, but replacing a nested entity
    /// handle requires a new `ChatFormState` so its subscriptions are rebuilt
    /// outside rendering.
    pub(crate) fn new(controls: &ChatFormControls, cx: &mut Context<Self>) -> Self {
        if let ControlSlot::Disabled(composer) = &controls.composer {
            composer.update(cx, |composer, cx| composer.set_disabled(true, cx));
        }
        let mut subscriptions = Vec::new();
        if let Some(state) = controls.run_settings.model.value().cloned() {
            subscriptions.push(cx.observe(&state, |_, _, cx| cx.notify()));
        }
        if let Some(state) = controls.run_settings.reasoning.value().cloned() {
            subscriptions.push(cx.observe(&state, |_, _, cx| cx.notify()));
        }
        if let Some(state) = controls.run_settings.approval.value().cloned() {
            subscriptions.push(cx.observe(&state, |_, _, cx| cx.notify()));
        }
        if let Some(composer) = controls.composer.value() {
            subscriptions.push(cx.observe(composer, |_, _, cx| cx.notify()));
        }
        if let Some(attachments) = controls.attachments.value() {
            subscriptions.push(cx.observe(attachments, |_, _, cx| cx.notify()));
        }
        if let Some(primary_action) = controls.primary_action.value() {
            subscriptions.push(cx.observe(primary_action, |_, _, cx| cx.notify()));
        }
        if let Some(project) = controls.project.value() {
            subscriptions.push(cx.observe(project, |_, _, cx| cx.notify()));
        }
        Self {
            bounds: None,
            _subscriptions: subscriptions,
        }
    }
}

/// Pure visual shell shared by conversation input and shortcut editing.
/// Business state and form stores live in the caller/controller that supplies
/// `ChatFormControls`.
#[derive(IntoElement)]
pub(crate) struct ChatForm {
    state: Entity<ChatFormState>,
    controls: ChatFormControls,
    skill_completion_placement: ChatFormSkillCompletionPlacement,
    primary_action_can_submit: bool,
    primary_action_disabled_reason: Option<SharedString>,
}

impl ChatForm {
    /// Rebuilds the visual shell for the control entities used to construct
    /// `state`. See [`ChatFormState::new`] for the handle-stability contract.
    pub(crate) fn new(state: &Entity<ChatFormState>, controls: ChatFormControls) -> Self {
        Self {
            state: state.clone(),
            controls,
            skill_completion_placement: ChatFormSkillCompletionPlacement::BelowForm,
            primary_action_can_submit: false,
            primary_action_disabled_reason: None,
        }
    }

    pub(crate) fn skill_completion_placement(
        mut self,
        placement: ChatFormSkillCompletionPlacement,
    ) -> Self {
        self.skill_completion_placement = placement;
        self
    }

    pub(crate) fn primary_action_projection(
        mut self,
        can_submit: bool,
        disabled_reason: Option<SharedString>,
    ) -> Self {
        self.primary_action_can_submit = can_submit;
        self.primary_action_disabled_reason = disabled_reason;
        self
    }

    fn composer(&self) -> Option<&Entity<ComposerEditor>> {
        self.controls.composer.value()
    }

    fn render_project(&self, cx: &mut App) -> Option<AnyElement> {
        let (state, enabled) = match &self.controls.project {
            ControlSlot::Hidden => return None,
            ControlSlot::Disabled(state) => (state.clone(), false),
            ControlSlot::Enabled(state) => (state.clone(), true),
        };
        let (label, icon, open, picker) = {
            let state = state.read(cx);
            let (label, icon) = state
                .picker
                .read(cx)
                .delegate()
                .selected_item()
                .map(|item| item.trigger_presentation())
                .unwrap_or_else(|| {
                    (
                        cx.global::<crate::foundation::I18n>()
                            .t("new-conversation-project-none")
                            .into(),
                        IconName::FolderX,
                    )
                });
            (label, icon, enabled && state.open, state.picker.clone())
        };
        let project_state = state.clone();
        let event_target = self.state.downgrade();
        let add_project = Button::new("jaco-chat-form-add-project")
            .ghost()
            .icon(IconName::FolderPlus)
            .label(
                cx.global::<crate::foundation::I18n>()
                    .t("button-add-project"),
            )
            .small()
            .w_full()
            .disabled(!enabled)
            .on_click(move |_, _window, cx| {
                let _ = event_target.update(cx, |_, cx| {
                    cx.emit(ChatFormUiEvent::AddProjectRequested);
                });
            });

        let picker = PickerPopover::new(PickerPopoverConfig {
            id: "jaco-chat-form-project-popover",
            open,
            trigger: project_picker_trigger(
                "jaco-chat-form-project-trigger",
                icon,
                label,
                open,
                cx,
            )
            .disabled(!enabled),
            list: picker,
            width: px(320.),
            max_height: rems(18.).into(),
            search_placeholder: Some(
                cx.global::<crate::foundation::I18n>()
                    .t("new-conversation-project-search")
                    .into(),
            ),
            footer: enabled.then_some(add_project.into_any_element()),
            on_open_change: move |open, _window, cx| {
                project_state.update(cx, |state, cx| {
                    state.open = *open;
                    cx.notify();
                });
            },
        });

        Some(
            h_flex()
                .id("jaco-chat-form-project-bar")
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
                .bg(cx.theme().tokens.muted.background)
                .text_color(cx.theme().muted_foreground)
                .border_1()
                .border_color(cx.theme().border.opacity(0.35))
                .child(picker)
                .into_any_element(),
        )
    }

    fn render_attachments(&self, cx: &mut App) -> Option<AnyElement> {
        let (ControlSlot::Enabled(attachments) | ControlSlot::Disabled(attachments)) =
            &self.controls.attachments
        else {
            return None;
        };
        let enabled = self.controls.attachments.is_enabled();
        let attachments = attachments.read(cx).attachments.clone();
        (!attachments.is_empty()).then(|| {
            div()
                .id("chat-form-attachments-strip")
                .w_full()
                .overflow_x_scroll()
                .child(
                    h_flex()
                        .items_end()
                        .gap(px(attachments::STRIP_GAP))
                        .children(attachments.into_iter().map(|attachment| {
                            self.render_attachment_card(attachment, enabled, cx)
                        })),
                )
                .into_any_element()
        })
    }

    fn render_attachment_card(
        &self,
        attachment: ComposerAttachment,
        enabled: bool,
        cx: &mut App,
    ) -> AnyElement {
        match attachment.kind {
            ComposerAttachmentKind::Image => {
                self.render_image_attachment_card(attachment, enabled, cx)
            }
            ComposerAttachmentKind::File => {
                self.render_file_attachment_card(attachment, enabled, cx)
            }
        }
    }

    fn render_image_attachment_card(
        &self,
        attachment: ComposerAttachment,
        enabled: bool,
        cx: &mut App,
    ) -> AnyElement {
        let local_id = attachment.local_id;
        let radius = attachments::CARD_RADIUS;
        let mut card = div()
            .id(format!("chat-form-attachment-image-{local_id}"))
            .relative()
            .flex_none()
            .size(px(attachments::IMAGE_THUMBNAIL_SIZE))
            .rounded(px(radius))
            .child(
                div()
                    .absolute()
                    .top(px(0.))
                    .right(px(0.))
                    .bottom(px(0.))
                    .left(px(0.))
                    .rounded(px(radius))
                    .overflow_hidden()
                    .child(render_attachment_image(&attachment, radius)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(0.))
                    .right(px(0.))
                    .bottom(px(0.))
                    .left(px(0.))
                    .rounded(px(radius))
                    .border_1()
                    .border_color(cx.theme().border),
            );
        if enabled {
            let open_attachment = attachment.clone();
            let event_target = self.state.downgrade();
            card = card
                .cursor(CursorStyle::PointingHand)
                .on_click(move |_, _, cx| {
                    let _ = event_target.update(cx, |_, cx| {
                        cx.emit(ChatFormUiEvent::OpenAttachmentRequested(
                            open_attachment.clone(),
                        ));
                    });
                })
                .child(self.render_remove_attachment_button(
                    local_id,
                    "chat-form-remove-image",
                    cx,
                ));
        }
        card.into_any_element()
    }

    fn render_file_attachment_card(
        &self,
        attachment: ComposerAttachment,
        enabled: bool,
        cx: &mut App,
    ) -> AnyElement {
        let local_id = attachment.local_id;
        let mut card = h_flex()
            .id(format!("chat-form-attachment-file-{local_id}"))
            .relative()
            .flex_none()
            .w(px(attachments::FILE_CARD_WIDTH))
            .h(px(attachments::FILE_CARD_HEIGHT))
            .gap_2()
            .p_2()
            .rounded(px(attachments::CARD_RADIUS))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().tokens.muted.background.opacity(0.22))
            .child(
                div()
                    .flex_none()
                    .size(px(32.))
                    .rounded(px(6.))
                    .bg(cx.theme().tokens.background.background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(IconName::File)
                            .with_size(px(18.))
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .flex_1()
                    .gap(px(2.))
                    .child(Label::new(attachment.name.clone()).text_sm().truncate())
                    .child(
                        Label::new(attachments::format_file_size(attachment.size_bytes))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate(),
                    ),
            );
        if enabled {
            let open_attachment = attachment.clone();
            let event_target = self.state.downgrade();
            card = card
                .cursor(CursorStyle::PointingHand)
                .hover(|this| this.border_color(cx.theme().primary.opacity(0.55)))
                .on_click(move |_, _, cx| {
                    let _ = event_target.update(cx, |_, cx| {
                        cx.emit(ChatFormUiEvent::OpenAttachmentRequested(
                            open_attachment.clone(),
                        ));
                    });
                })
                .child(self.render_remove_attachment_button(local_id, "chat-form-remove-file", cx));
        }
        card.into_any_element()
    }

    fn render_remove_attachment_button(
        &self,
        local_id: u64,
        id_prefix: &'static str,
        cx: &mut App,
    ) -> AnyElement {
        Button::new(format!("{id_prefix}-{local_id}"))
            .ghost()
            .absolute()
            .top(px(4.))
            .right(px(4.))
            .with_size(px(attachments::REMOVE_BUTTON_SIZE))
            .size(px(attachments::REMOVE_BUTTON_SIZE))
            .p_0()
            .rounded(px(999.))
            .bg(cx.theme().tokens.background.background.opacity(0.86))
            .child(Icon::new(IconName::X).with_size(px(14.)))
            .tooltip(
                cx.global::<crate::foundation::I18n>()
                    .t("chat-form-attachment-remove"),
            )
            .on_click({
                let event_target = self.state.downgrade();
                move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = event_target.update(cx, |_, cx| {
                        cx.emit(ChatFormUiEvent::RemoveAttachmentRequested(local_id));
                    });
                }
            })
            .into_any_element()
    }

    fn render_add_attachment_menu(&self, enabled: bool, cx: &mut App) -> AnyElement {
        let i18n = cx.global::<crate::foundation::I18n>();
        let add_files = i18n.t("chat-form-attachment-add-files");
        let add_from_clipboard = i18n.t("chat-form-attachment-add-from-clipboard");
        let form = self.state.downgrade();

        Button::new("chat-form-add")
            .ghost()
            .with_size(px(28.))
            .size(px(28.))
            .p(px(0.))
            .rounded(px(999.))
            .child(Icon::new(IconName::Plus).with_size(px(18.)))
            .tooltip(i18n.t("chat-form-add-tooltip"))
            .disabled(!enabled)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |menu, _window, _cx| {
                let form_for_files = form.clone();
                let form_for_clipboard = form.clone();
                menu.item(
                    PopupMenuItem::new(add_files.clone())
                        .icon(IconName::Paperclip)
                        .on_click(move |_, _, cx| {
                            let _ = form_for_files.update(cx, |_, cx| {
                                cx.emit(ChatFormUiEvent::AddAttachmentFilesRequested);
                            });
                        }),
                )
                .item(
                    PopupMenuItem::new(add_from_clipboard.clone())
                        .icon(IconName::Clipboard)
                        .on_click(move |_, _, cx| {
                            let _ = form_for_clipboard.update(cx, |_, cx| {
                                cx.emit(ChatFormUiEvent::AddAttachmentFromClipboardRequested);
                            });
                        }),
                )
            })
            .into_any_element()
    }

    fn render_run_settings(
        &self,
    ) -> (
        Option<run_settings::ModelSelector>,
        Option<run_settings::ReasoningSelector>,
        Option<run_settings::ApprovalSelector>,
    ) {
        let model = match &self.controls.run_settings.model {
            ControlSlot::Hidden => None,
            ControlSlot::Disabled(state) => {
                Some(run_settings::ModelSelector::new(state.clone(), false))
            }
            ControlSlot::Enabled(state) => {
                Some(run_settings::ModelSelector::new(state.clone(), true))
            }
        };
        let reasoning = match &self.controls.run_settings.reasoning {
            ControlSlot::Hidden => None,
            ControlSlot::Disabled(state) => {
                Some(run_settings::ReasoningSelector::new(state.clone(), false))
            }
            ControlSlot::Enabled(state) => {
                Some(run_settings::ReasoningSelector::new(state.clone(), true))
            }
        };
        let approval = match &self.controls.run_settings.approval {
            ControlSlot::Hidden => None,
            ControlSlot::Disabled(state) => {
                Some(run_settings::ApprovalSelector::new(state.clone(), false))
            }
            ControlSlot::Enabled(state) => {
                Some(run_settings::ApprovalSelector::new(state.clone(), true))
            }
        };
        (model, reasoning, approval)
    }

    fn render_skill_completion(&mut self, window: &mut Window, cx: &mut App) -> AnyElement {
        let Some(bounds) = self.state.read(cx).bounds else {
            return div().into_any_element();
        };
        let Some(composer) = self.composer().cloned() else {
            return div().into_any_element();
        };
        if !composer.read(cx).skill_completion_open() {
            return div().into_any_element();
        }
        let margin = px(SKILL_COMPLETION_WINDOW_MARGIN);
        let Some(layout) = skill_completion_popup_layout(
            bounds,
            window.viewport_size(),
            self.skill_completion_placement,
        ) else {
            return div().into_any_element();
        };
        let panel = composer.update(cx, |composer, cx| {
            composer.render_skill_completion_panel(layout.max_height, window, cx)
        });
        deferred(
            anchored()
                .anchor(layout.anchor)
                .position(layout.position)
                .offset(layout.offset)
                .snap_to_window_with_margin(margin)
                .child(
                    div()
                        .debug_selector(|| "jaco-skill-completion-popup".into())
                        .w(bounds.size.width)
                        .child(panel),
                ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SkillCompletionPopupLayout {
    pub(crate) anchor: Anchor,
    pub(crate) position: Point<Pixels>,
    pub(crate) offset: Point<Pixels>,
    pub(crate) max_height: Pixels,
}

pub(crate) fn skill_completion_popup_layout(
    form_bounds: Bounds<Pixels>,
    viewport_size: Size<Pixels>,
    placement: ChatFormSkillCompletionPlacement,
) -> Option<SkillCompletionPopupLayout> {
    let gap = px(SKILL_COMPLETION_GAP);
    let margin = px(SKILL_COMPLETION_WINDOW_MARGIN);
    let max_height = px(SKILL_COMPLETION_MAX_HEIGHT);

    let (anchor, position, offset, available_height) = match placement {
        ChatFormSkillCompletionPlacement::AboveForm => (
            Anchor::BottomLeft,
            point(form_bounds.left(), form_bounds.top()),
            point(px(0.), -gap),
            form_bounds.top() - margin - gap,
        ),
        ChatFormSkillCompletionPlacement::BelowForm => (
            Anchor::TopLeft,
            point(form_bounds.left(), form_bounds.bottom()),
            point(px(0.), gap),
            viewport_size.height - form_bounds.bottom() - margin - gap,
        ),
    };

    let max_height = available_height.max(px(0.)).min(max_height);
    (max_height > px(0.)).then_some(SkillCompletionPopupLayout {
        anchor,
        position,
        offset,
        max_height,
    })
}

fn render_attachment_image(attachment: &ComposerAttachment, radius: f32) -> AnyElement {
    match &attachment.source {
        ComposerAttachmentSource::LocalFile { path } => img(path.clone())
            .size_full()
            .rounded(px(radius))
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        ComposerAttachmentSource::GeneratedImage { image } => img(image.clone())
            .size_full()
            .rounded(px(radius))
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
    }
}

fn footer_primary_controls() -> Div {
    h_flex().items_center().gap(px(5.)).flex_none()
}

#[derive(IntoElement)]
struct PrimaryAction {
    state: Entity<PrimaryActionControlState>,
    event_target: WeakEntity<ChatFormState>,
    enabled: bool,
    can_submit: bool,
    disabled_reason: Option<SharedString>,
}

impl PrimaryAction {
    fn new(
        state: &Entity<PrimaryActionControlState>,
        event_target: WeakEntity<ChatFormState>,
        enabled: bool,
        can_submit: bool,
        disabled_reason: Option<SharedString>,
    ) -> Self {
        Self {
            state: state.clone(),
            event_target,
            enabled,
            can_submit,
            disabled_reason,
        }
    }
}

impl View for PrimaryAction {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state.read(cx);
        let agent_status = state.agent_status(cx);
        let agent_active = matches!(
            agent_status,
            AgentRunControlStatus::Running | AgentRunControlStatus::Stopping
        );
        let submitting = agent_status == AgentRunControlStatus::Submitting;
        let agent_stopping = agent_status == AgentRunControlStatus::Stopping;
        let enabled = self.enabled;
        let event_target = self.event_target;

        Button::new(if agent_active {
            "chat-form-stop"
        } else {
            "chat-form-send"
        })
        .primary()
        .with_size(px(28.))
        .size(px(28.))
        .p(px(0.))
        .rounded(px(999.))
        .disabled(!enabled || submitting || agent_stopping || (!agent_active && !self.can_submit))
        .loading(submitting)
        .when_some(
            match agent_status {
                AgentRunControlStatus::Running => Some(
                    cx.global::<crate::foundation::I18n>()
                        .t("chat-form-stop-tooltip")
                        .into(),
                ),
                AgentRunControlStatus::Stopping => Some(
                    cx.global::<crate::foundation::I18n>()
                        .t("chat-form-stopping-tooltip")
                        .into(),
                ),
                AgentRunControlStatus::Submitting => None,
                AgentRunControlStatus::Idle => self.disabled_reason,
            },
            |button, reason| button.tooltip(reason),
        )
        .child(Icon::new(if agent_active {
            IconName::Square
        } else {
            IconName::Send
        }))
        .on_click(move |_, _window, cx| {
            if enabled {
                let _ = event_target.update(cx, |_, cx| {
                    cx.emit(ChatFormUiEvent::PrimaryActionRequested);
                });
            }
        })
    }
}

impl View for ChatForm {
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let form = self.state.downgrade();
        let composer = self.composer().cloned();
        let project = self.render_project(cx);
        let attachments = self.render_attachments(cx);
        let (model, reasoning, approval) = self.render_run_settings();
        let add_attachment = self.controls.add_attachment.is_visible();
        let add_attachment_enabled = self.controls.add_attachment.is_enabled();
        let primary_action = self.controls.primary_action.value().cloned();
        let primary_enabled = self.controls.primary_action.is_enabled();
        let primary_action_can_submit = self.primary_action_can_submit;
        let primary_action_disabled_reason = self.primary_action_disabled_reason.clone();
        let attachments_enabled_for_drop = self.controls.attachments.is_enabled();
        let drop_event_target = self.state.downgrade();
        let skill_completion_open = composer
            .as_ref()
            .is_some_and(|composer| composer.read(cx).skill_completion_open());

        let chat_form = v_flex()
            .id("jaco-chat-form")
            .debug_selector(|| "jaco-chat-form".into())
            .w_full()
            .relative()
            .on_prepaint(move |bounds, _, cx| {
                let _ = form.update(cx, |form, _| {
                    form.bounds = Some(bounds);
                });
            })
            .rounded(px(25.))
            .border_1()
            .border_color(cx.theme().input)
            .bg(cx.theme().input_background())
            .text_color(cx.theme().foreground)
            .when(cx.theme().shadow, |this| {
                this.shadow(vec![box_shadow(
                    0.,
                    4.,
                    16.,
                    0.,
                    cx.theme().foreground.opacity(0.05),
                )])
            })
            .on_drop(move |paths: &ExternalPaths, _window, cx| {
                if attachments_enabled_for_drop {
                    let _ = drop_event_target.update(cx, |_, cx| {
                        cx.emit(ChatFormUiEvent::ExternalPathsDropped(
                            paths.paths().to_vec(),
                        ));
                    });
                }
            })
            .when_some(composer, |this, composer| {
                this.child(
                    v_flex()
                        .w_full()
                        .min_h(px(56.))
                        .px(px(COMPOSER_INPUT_HORIZONTAL_PADDING))
                        .pt(px(COMPOSER_INPUT_TOP_PADDING))
                        .mb(px(COMPOSER_INPUT_BOTTOM_MARGIN))
                        .gap(px(attachments::STRIP_BOTTOM_MARGIN))
                        .when_some(attachments, |this, attachments| this.child(attachments))
                        .child(composer),
                )
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .min_h(px(28.))
                    .px(px(COMPOSER_FOOTER_HORIZONTAL_PADDING))
                    .mb(px(COMPOSER_FOOTER_BOTTOM_MARGIN))
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(5.))
                            .min_w_0()
                            .when(add_attachment, |this| {
                                this.child(
                                    self.render_add_attachment_menu(add_attachment_enabled, cx),
                                )
                            })
                            .when_some(reasoning, |this, reasoning| this.child(reasoning))
                            .when_some(approval, |this, approval| this.child(approval)),
                    )
                    .child(div().flex_1().min_w_0())
                    .child(
                        footer_primary_controls()
                            .when_some(model, |this, model| this.child(model))
                            .when_some(primary_action, |this, action| {
                                this.child(PrimaryAction::new(
                                    &action,
                                    self.state.downgrade(),
                                    primary_enabled,
                                    primary_action_can_submit,
                                    primary_action_disabled_reason.clone(),
                                ))
                            }),
                    ),
            )
            .when(skill_completion_open, |this| {
                this.child(self.render_skill_completion(window, cx))
            });

        let chat_form = chat_form.into_any_element();
        if let Some(project) = project {
            v_flex()
                .id("jaco-chat-form-stack")
                .w_full()
                .relative()
                .pb(px(PROJECT_BAR_VISIBLE_HEIGHT))
                .child(project)
                .child(
                    div()
                        .id("jaco-chat-form-layer")
                        .w_full()
                        .rounded(px(25.))
                        .bg(cx.theme().background.blend(cx.theme().input_background()))
                        .child(chat_form),
                )
                .into_any_element()
        } else {
            chat_form
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChatForm, ChatFormControls, ChatFormState, ControlSlot, PrimaryAction,
        PrimaryActionControlState, RunSettingsControls, footer_primary_controls,
    };
    use crate::components::picker::{
        PickerContentPopoverConfig, picker_content_popover, picker_trigger_with_icon,
    };
    use gpui::{
        AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled as _,
        TestAppContext, View as _, Window, div, px,
    };
    use gpui_component::{h_flex, v_flex};

    struct FooterControlsLayout;

    impl Render for FooterControlsLayout {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let picker = |id| {
                picker_content_popover(
                    cx,
                    PickerContentPopoverConfig {
                        id,
                        open: false,
                        trigger: picker_trigger_with_icon(
                            id,
                            div().size_4().into_any_element(),
                            "gpt-5.5",
                            false,
                        ),
                        content: div().into_any_element(),
                        width: px(200.),
                        footer: None,
                        on_open_change: |_, _, _| {},
                    },
                )
            };

            v_flex().child(h_flex().child(picker("natural"))).child(
                h_flex()
                    .w(px(180.))
                    .child(div().w(px(100.)).flex_none())
                    .child(div().flex_1().min_w_0())
                    .child(
                        footer_primary_controls()
                            .child(picker("footer"))
                            .child(div().size(px(28.)).flex_none()),
                    ),
            )
        }
    }

    fn hidden_controls() -> ChatFormControls {
        ChatFormControls {
            project: ControlSlot::Hidden,
            composer: ControlSlot::Hidden,
            attachments: ControlSlot::Hidden,
            add_attachment: ControlSlot::Hidden,
            run_settings: RunSettingsControls {
                model: ControlSlot::Hidden,
                reasoning: ControlSlot::Hidden,
                approval: ControlSlot::Hidden,
            },
            primary_action: ControlSlot::Hidden,
        }
    }

    #[gpui::test]
    fn direct_views_use_backing_identity_across_rebuilds(cx: &mut TestAppContext) {
        let controls = hidden_controls();
        let form_state = cx.new(|cx| ChatFormState::new(&controls, cx));
        let first_form = ChatForm::new(&form_state, controls.clone());
        let refreshed_form = ChatForm::new(&form_state, controls)
            .primary_action_projection(true, Some("ready".into()));

        assert_eq!(first_form.entity_id(), Some(form_state.entity_id()));
        assert_eq!(refreshed_form.entity_id(), first_form.entity_id());

        let primary_state = cx.new(|_| PrimaryActionControlState::default());
        let first_primary =
            PrimaryAction::new(&primary_state, form_state.downgrade(), true, false, None);
        let refreshed_primary = PrimaryAction::new(
            &primary_state,
            form_state.downgrade(),
            false,
            true,
            Some("disabled".into()),
        );

        assert_eq!(first_primary.entity_id(), Some(primary_state.entity_id()));
        assert_eq!(refreshed_primary.entity_id(), first_primary.entity_id());
    }

    #[gpui::test]
    fn footer_primary_controls_preserve_model_trigger_intrinsic_width(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (_, cx) = cx.add_window_view(|_, _| FooterControlsLayout);
        cx.run_until_parked();

        let natural = cx
            .debug_bounds("picker-trigger-label:natural")
            .expect("natural picker trigger label should be rendered");
        let footer = cx
            .debug_bounds("picker-trigger-label:footer")
            .expect("footer picker trigger label should be rendered");

        assert_eq!(footer.size.width, natural.size.width);
    }
}
