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
        chat_input::{ChatFormSkillCompletionPlacement, ComposerEditor, attachments},
        picker::{PickerPopoverConfig, picker_popover},
        run_settings,
    },
    foundation::assets::IconName,
    state::attachments::{ComposerAttachment, ComposerAttachmentKind, ComposerAttachmentSource},
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
    AddAttachmentControl, AttachmentControlState, ChatFormControls, ControlSlot,
    PrimaryActionControlState, RunSettingsControls,
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

impl EventEmitter<ChatFormUiEvent> for ChatForm {}

/// Pure visual shell shared by conversation input and shortcut editing.
/// Business state and form stores live in the caller/controller that supplies
/// `ChatFormControls`.
pub(crate) struct ChatForm {
    controls: ChatFormControls,
    bounds: Option<Bounds<Pixels>>,
    skill_completion_placement: ChatFormSkillCompletionPlacement,
    _subscriptions: Vec<Subscription>,
}

impl ChatForm {
    pub(crate) fn new(
        controls: ChatFormControls,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if let ControlSlot::Disabled(composer) = &controls.composer {
            composer.update(cx, |composer, cx| composer.set_disabled(true, cx));
        }
        let mut subscriptions = Vec::new();
        subscriptions.push(cx.observe(&controls.run_settings.form, |_, _, cx| cx.notify()));
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
            if let Some(form) = attachments.read(cx).form.clone() {
                subscriptions.push(cx.observe(&form, |_, _, cx| cx.notify()));
            }
        }
        if let Some(primary_action) = controls.primary_action.value() {
            subscriptions.push(cx.observe(primary_action, |_, _, cx| cx.notify()));
        }
        if let Some(project) = controls.project.value() {
            subscriptions.push(cx.observe(project, |_, _, cx| cx.notify()));
        }
        Self {
            controls,
            bounds: None,
            skill_completion_placement: ChatFormSkillCompletionPlacement::BelowForm,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn controls(&self) -> &ChatFormControls {
        &self.controls
    }

    pub(crate) fn focus_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let ControlSlot::Enabled(composer) = &self.controls.composer else {
            return false;
        };
        composer.update(cx, |composer, cx| composer.focus(window, cx));
        true
    }

    pub(crate) fn set_skill_completion_placement(
        &mut self,
        placement: ChatFormSkillCompletionPlacement,
    ) {
        self.skill_completion_placement = placement;
    }

    fn composer(&self) -> Option<&Entity<ComposerEditor>> {
        self.controls.composer.value()
    }

    fn render_project(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
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
            .on_click(cx.listener(|_form, _, _window, cx| {
                cx.emit(ChatFormUiEvent::AddProjectRequested);
            }));

        let picker = picker_popover(
            cx,
            PickerPopoverConfig {
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
            },
        );

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
                .bg(cx.theme().muted)
                .text_color(cx.theme().muted_foreground)
                .border_1()
                .border_color(cx.theme().border.opacity(0.35))
                .child(picker)
                .into_any_element(),
        )
    }

    fn render_attachments(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (ControlSlot::Enabled(attachments) | ControlSlot::Disabled(attachments)) =
            &self.controls.attachments
        else {
            return None;
        };
        let enabled = self.controls.attachments.is_enabled();
        let form = attachments.read(cx).form.clone();
        let attachments = form
            .map(|form| form.read(cx).draft().attachments.clone())
            .unwrap_or_default();
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
        cx: &mut Context<Self>,
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
        cx: &mut Context<Self>,
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
            card = card
                .cursor(CursorStyle::PointingHand)
                .on_click(cx.listener(move |_form, _, _, cx| {
                    cx.emit(ChatFormUiEvent::OpenAttachmentRequested(
                        open_attachment.clone(),
                    ));
                }))
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
        cx: &mut Context<Self>,
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
            .bg(cx.theme().muted.opacity(0.22))
            .child(
                div()
                    .flex_none()
                    .size(px(32.))
                    .rounded(px(6.))
                    .bg(cx.theme().background)
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
            card = card
                .cursor(CursorStyle::PointingHand)
                .hover(|this| this.border_color(cx.theme().primary.opacity(0.55)))
                .on_click(cx.listener(move |_form, _, _, cx| {
                    cx.emit(ChatFormUiEvent::OpenAttachmentRequested(
                        open_attachment.clone(),
                    ));
                }))
                .child(self.render_remove_attachment_button(local_id, "chat-form-remove-file", cx));
        }
        card.into_any_element()
    }

    fn render_remove_attachment_button(
        &self,
        local_id: u64,
        id_prefix: &'static str,
        cx: &mut Context<Self>,
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
            .bg(cx.theme().background.opacity(0.86))
            .child(Icon::new(IconName::X).with_size(px(14.)))
            .tooltip(
                cx.global::<crate::foundation::I18n>()
                    .t("chat-form-attachment-remove"),
            )
            .on_click(cx.listener(move |_form, _, _, cx| {
                cx.stop_propagation();
                cx.emit(ChatFormUiEvent::RemoveAttachmentRequested(local_id));
            }))
            .into_any_element()
    }

    fn render_add_attachment_menu(&self, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        let i18n = cx.global::<crate::foundation::I18n>();
        let add_files = i18n.t("chat-form-attachment-add-files");
        let add_from_clipboard = i18n.t("chat-form-attachment-add-from-clipboard");
        let form = cx.entity().downgrade();

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
        cx: &mut Context<Self>,
    ) -> (Option<AnyElement>, Option<AnyElement>, Option<AnyElement>) {
        let form = self.controls.run_settings.form.clone();
        let model = match &self.controls.run_settings.model {
            ControlSlot::Hidden => None,
            ControlSlot::Disabled(state) => Some(run_settings::render_model_selector(
                form.clone(),
                state.clone(),
                false,
                cx,
            )),
            ControlSlot::Enabled(state) => Some(run_settings::render_model_selector(
                form.clone(),
                state.clone(),
                true,
                cx,
            )),
        };
        let reasoning = match &self.controls.run_settings.reasoning {
            ControlSlot::Hidden => None,
            ControlSlot::Disabled(state) => Some(run_settings::render_reasoning_selector(
                form.clone(),
                state.clone(),
                false,
                cx,
            )),
            ControlSlot::Enabled(state) => Some(run_settings::render_reasoning_selector(
                form.clone(),
                state.clone(),
                true,
                cx,
            )),
        };
        let approval = match &self.controls.run_settings.approval {
            ControlSlot::Hidden => None,
            ControlSlot::Disabled(state) => Some(run_settings::render_approval_selector(
                form.clone(),
                state.clone(),
                false,
                cx,
            )),
            ControlSlot::Enabled(state) => Some(run_settings::render_approval_selector(
                form.clone(),
                state.clone(),
                true,
                cx,
            )),
        };
        (model, reasoning, approval)
    }

    fn render_skill_completion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(bounds) = self.bounds else {
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

impl Render for ChatForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let form = cx.entity();
        let composer = self.composer().cloned();
        let project = self.render_project(cx);
        let attachments = self.render_attachments(cx);
        let (model, reasoning, approval) = self.render_run_settings(cx);
        let add_attachment = self.controls.add_attachment.is_visible();
        let add_attachment_enabled = self.controls.add_attachment.is_enabled();
        let primary_action = self.controls.primary_action.value().cloned();
        let primary_enabled = self.controls.primary_action.is_enabled();
        let skill_completion_open = composer
            .as_ref()
            .is_some_and(|composer| composer.read(cx).skill_completion_open());

        let chat_form = v_flex()
            .id("jaco-chat-form")
            .debug_selector(|| "jaco-chat-form".into())
            .w_full()
            .relative()
            .on_prepaint(move |bounds, _, cx| {
                form.update(cx, |form, _| {
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
            .on_drop(cx.listener(|form, paths: &ExternalPaths, _window, cx| {
                if form.controls.attachments.is_enabled() {
                    cx.emit(ChatFormUiEvent::ExternalPathsDropped(
                        paths.paths().to_vec(),
                    ));
                }
            }))
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
                        h_flex()
                            .items_center()
                            .gap(px(5.))
                            .min_w_0()
                            .when_some(model, |this, model| this.child(model))
                            .when_some(primary_action, |this, action| {
                                let agent_running = action.read(cx).agent_running;
                                let can_submit = action.read(cx).can_submit;
                                this.child(
                                    Button::new(if agent_running {
                                        "chat-form-stop"
                                    } else {
                                        "chat-form-send"
                                    })
                                    .primary()
                                    .with_size(px(28.))
                                    .size(px(28.))
                                    .p(px(0.))
                                    .rounded(px(999.))
                                    .disabled(!primary_enabled || (!agent_running && !can_submit))
                                    .child(Icon::new(if agent_running {
                                        IconName::Square
                                    } else {
                                        IconName::Send
                                    }))
                                    .on_click(cx.listener(
                                        |form, _, _window, cx| {
                                            if form.controls.primary_action.is_enabled() {
                                                cx.emit(ChatFormUiEvent::PrimaryActionRequested);
                                            }
                                        },
                                    )),
                                )
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
