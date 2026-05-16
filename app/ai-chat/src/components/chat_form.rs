pub(crate) mod ext_settings;
mod mode_select;
mod model_select;
mod picker;
mod template_picker;

use crate::{
    database::{ConversationTemplate, ConversationTemplatePrompt, Db, Mode},
    errors::AiChatResult,
    foundation::assets::IconName,
    foundation::i18n::I18n,
    llm::{apply_ext_setting, build_request_template, preset_ext_settings, provider_by_name},
    state::ConversationDraft,
};
use ext_settings::{ExtSettings, ExtSettingsEvent, ExtSettingsLayout};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState, Position},
    list::ListState,
    notification::{Notification, NotificationType},
    tag::Tag,
    v_flex,
};
use mode_select::{ModeSelect, ModeSelectEvent};
use model_select::{ModelSelect, ModelSelectEvent};
use picker::{PickerListDelegate, PickerPopoverOptions, render_picker_popover};
use std::rc::Rc;
use template_picker::{TemplateOption, template_sections};

#[derive(Default)]
pub(crate) struct TemplateChangeState {
    _version: u64,
}

impl Global for TemplateChangeState {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(TemplateChangeState::default());
}

pub(crate) fn notify_templates_changed(cx: &mut App) {
    cx.update_global::<TemplateChangeState, _>(|state, _cx| {
        state._version = state._version.wrapping_add(1);
    });
}

#[derive(Clone)]
pub(crate) enum ChatFormEvent {
    SendRequested,
    PauseRequested,
    StateChanged,
}

impl EventEmitter<ChatFormEvent> for ChatForm {}

#[derive(Clone)]
pub(crate) struct ChatFormSnapshot {
    pub(crate) text: String,
    pub(crate) provider_name: String,
    pub(crate) request_template: serde_json::Value,
    pub(crate) prompts: Vec<ConversationTemplatePrompt>,
    pub(crate) mode: Mode,
}

pub(crate) struct ChatForm {
    input_state: Entity<InputState>,
    mode_select: Entity<ModeSelect>,
    model_select: Entity<ModelSelect>,
    ext_settings: Entity<ExtSettings>,
    templates: Vec<ConversationTemplate>,
    selected_template: Option<ConversationTemplate>,
    request_template: Option<serde_json::Value>,
    template_picker: Option<Entity<ListState<PickerListDelegate<TemplateOption>>>>,
    template_picker_bounds: Bounds<Pixels>,
    running: bool,
    template_picker_open: bool,
    slash_restore_position: Option<Position>,
    suppress_template_trigger_once: bool,
    last_input_text: String,
    pending_restore_draft: Option<ConversationDraft>,
    _subscriptions: Vec<Subscription>,
}

impl ChatForm {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            let input = InputState::new(window, cx)
                .multi_line(true)
                .auto_grow(3, 8)
                .placeholder(cx.global::<I18n>().t("field-chat-input-placeholder"));
            input.focus(window, cx);
            input
        });
        let mode_select = cx.new(|cx| ModeSelect::new(window, cx));
        let model_select = cx.new(|cx| ModelSelect::new(window, cx));
        let ext_settings = cx.new(|cx| ExtSettings::new(ExtSettingsLayout::Compact, window, cx));
        let templates = cx
            .global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| ConversationTemplate::all(&mut conn).ok())
            .unwrap_or_default();

        let mut this = Self {
            input_state,
            mode_select,
            model_select,
            ext_settings,
            templates,
            selected_template: None,
            request_template: None,
            template_picker: None,
            template_picker_bounds: Bounds::default(),
            running: false,
            template_picker_open: false,
            slash_restore_position: None,
            suppress_template_trigger_once: false,
            last_input_text: String::new(),
            pending_restore_draft: None,
            _subscriptions: Vec::new(),
        };
        this.bind_input_events(window, cx);
        this.bind_model_select_events(window, cx);
        this.bind_mode_select_events(window, cx);
        this.bind_ext_settings_events(window, cx);
        this.bind_template_change_events(window, cx);
        this
    }

    fn bind_input_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_subscription = cx.subscribe_in(
            &self.input_state,
            window,
            |this, _input, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    this.on_input_change(window, cx);
                    cx.emit(ChatFormEvent::StateChanged);
                }
                InputEvent::PressEnter { secondary }
                    if !secondary && !this.running && this.can_send(cx) =>
                {
                    cx.emit(ChatFormEvent::SendRequested);
                }
                _ => {}
            },
        );
        self._subscriptions.push(input_subscription);
    }

    fn bind_model_select_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model_select_subscription = cx.subscribe_in(
            &self.model_select,
            window,
            |this, _state, event: &ModelSelectEvent, window, cx| match event {
                ModelSelectEvent::Change(Some(model)) => {
                    let provider = match provider_by_name(&model.provider_name) {
                        Ok(provider) => provider,
                        Err(_) => {
                            this.request_template = None;
                            cx.emit(ChatFormEvent::StateChanged);
                            cx.notify();
                            return;
                        }
                    };
                    let template = match provider.default_template_for_model(model) {
                        Ok(template) => template,
                        Err(_) => {
                            this.request_template = None;
                            this.ext_settings
                                .update(cx, |settings, cx| settings.clear(cx));
                            cx.emit(ChatFormEvent::StateChanged);
                            cx.notify();
                            return;
                        }
                    };
                    this.request_template = Some(template);
                    this.sync_ext_settings(model, window, cx);
                    this.try_restore_pending_draft(window, cx);
                    cx.emit(ChatFormEvent::StateChanged);
                    cx.notify();
                }
                ModelSelectEvent::Change(None) => {
                    this.request_template = None;
                    this.ext_settings
                        .update(cx, |settings, cx| settings.clear(cx));
                    cx.emit(ChatFormEvent::StateChanged);
                    cx.notify();
                }
                ModelSelectEvent::ModelsChanged => {
                    this.try_restore_pending_draft(window, cx);
                }
            },
        );
        self._subscriptions.push(model_select_subscription);
    }

    fn bind_mode_select_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let subscription = cx.subscribe_in(
            &self.mode_select,
            window,
            |_this, _, event: &ModeSelectEvent, _window, cx| {
                if matches!(event, ModeSelectEvent::Change(_)) {
                    cx.emit(ChatFormEvent::StateChanged);
                }
            },
        );
        self._subscriptions.push(subscription);
    }

    fn bind_ext_settings_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let subscription = cx.subscribe_in(
            &self.ext_settings,
            window,
            |this, _settings, event: &ExtSettingsEvent, window, cx| {
                let ExtSettingsEvent::Change(setting) = event;
                let Some(model) = this.model_select.read(cx).selected_model() else {
                    return;
                };
                let Some(template) = this.request_template.as_mut() else {
                    return;
                };
                if apply_ext_setting(&model, template, setting).is_err() {
                    return;
                }
                this.sync_ext_settings(&model, window, cx);
                cx.emit(ChatFormEvent::StateChanged);
                cx.notify();
            },
        );
        self._subscriptions.push(subscription);
    }

    fn bind_template_change_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let subscription =
            cx.observe_global_in::<TemplateChangeState>(window, |this, window, cx| {
                this.reload_templates(window, cx);
                cx.notify();
            });
        self._subscriptions.push(subscription);
    }

    fn reload_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut conn = match cx.global::<Db>().get() {
            Ok(conn) => conn,
            Err(err) => {
                self.notify_template_load_error(err.to_string(), window, cx);
                return;
            }
        };
        let templates = match ConversationTemplate::all(&mut conn) {
            Ok(templates) => templates,
            Err(err) => {
                self.notify_template_load_error(err.to_string(), window, cx);
                return;
            }
        };

        let (selected_template, selected_template_removed) =
            reconcile_selected_template(self.selected_template.take(), &templates);
        self.selected_template = selected_template;
        self.templates = templates;
        if selected_template_removed {
            cx.emit(ChatFormEvent::StateChanged);
        }
    }

    fn notify_template_load_error(
        &self,
        message: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.push_notification(
            Notification::new()
                .title(cx.global::<I18n>().t("notify-load-templates-failed"))
                .message(message)
                .with_type(NotificationType::Error),
            cx,
        );
    }

    fn sync_ext_settings(
        &mut self,
        model: &crate::llm::ProviderModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(template) = self.request_template.clone() else {
            self.ext_settings
                .update(cx, |settings, cx| settings.clear(cx));
            return;
        };
        let settings = match preset_ext_settings(model, &template) {
            Ok(settings) => settings,
            Err(_) => {
                self.ext_settings
                    .update(cx, |settings, cx| settings.clear(cx));
                return;
            }
        };
        self.ext_settings.update(cx, |ext_settings, cx| {
            ext_settings.set_items(settings, window, cx)
        });
    }

    fn on_input_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, cursor, cursor_position) = {
            let input = self.input_state.read(cx);
            (
                input.value().to_string(),
                input.cursor(),
                input.cursor_position(),
            )
        };
        if self.suppress_template_trigger_once {
            self.suppress_template_trigger_once = false;
            self.last_input_text = text;
            cx.notify();
            return;
        }
        if self.template_picker_open {
            self.last_input_text = text;
            cx.notify();
            return;
        }
        let Some((next, slash_restore_position)) =
            detect_template_trigger(&self.last_input_text, &text, cursor, cursor_position)
        else {
            self.last_input_text = text;
            cx.notify();
            return;
        };
        self.last_input_text = next.clone();
        self.reload_templates(window, cx);
        self.template_picker_open = true;
        self.slash_restore_position = Some(slash_restore_position);
        self.rebuild_template_picker(window, cx);
        self.input_state.update(cx, |input, cx| {
            input.set_value(next, window, cx);
            input.set_cursor_position(slash_restore_position, window, cx);
        });
        self.focus_template_picker(window, cx);
        cx.notify();
    }

    fn open_template_picker(
        &mut self,
        slash_restore_position: Option<Position>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reload_templates(window, cx);
        self.template_picker_open = true;
        self.slash_restore_position = slash_restore_position;
        self.rebuild_template_picker(window, cx);
        self.focus_template_picker(window, cx);
        cx.notify();
    }

    fn close_template_picker(
        &mut self,
        restore_slash: bool,
        focus_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.template_picker_open = false;
        self.template_picker = None;
        if restore_slash && let Some(slash_restore_position) = self.slash_restore_position.take() {
            self.suppress_template_trigger_once = true;
            self.input_state.update(cx, |input, cx| {
                input.set_cursor_position(slash_restore_position, window, cx);
                input.insert("/", window, cx);
            });
            self.last_input_text = self.input_state.read(cx).value().to_string();
        } else {
            self.slash_restore_position = None;
        }
        if focus_input {
            self.input_state
                .update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    fn choose_template(
        &mut self,
        template: ConversationTemplate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_template = Some(template);
        self.close_template_picker(false, true, window, cx);
        cx.emit(ChatFormEvent::StateChanged);
    }

    pub(crate) fn clear_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.last_input_text.clear();
        cx.emit(ChatFormEvent::StateChanged);
    }

    pub(crate) fn focus_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state
            .update(cx, |input, cx| input.focus(window, cx));
    }

    pub(crate) fn set_running(&mut self, running: bool, cx: &mut Context<Self>) {
        self.running = running;
        cx.notify();
    }

    pub(crate) fn snapshot(&self, cx: &App) -> AiChatResult<Option<ChatFormSnapshot>> {
        let Some(selected_model) = self.model_select.read(cx).selected_model() else {
            return Ok(None);
        };
        let Some(request_template) = self.request_template.clone() else {
            return Ok(None);
        };
        let mode = self.mode_select.read(cx).selected_value();
        Ok(Some(ChatFormSnapshot {
            text: self.input_state.read(cx).value().to_string(),
            provider_name: selected_model.provider_name.clone(),
            request_template,
            prompts: self
                .selected_template
                .as_ref()
                .map(|template| template.prompts.clone())
                .unwrap_or_default(),
            mode,
        }))
    }

    pub(crate) fn draft_snapshot(&self, cx: &App) -> Option<ConversationDraft> {
        let text = self.input_state.read(cx).value().to_string();
        let mode = self.mode_select.read(cx).selected_value();
        let selected_template_id = self.selected_template.as_ref().map(|template| template.id);
        let selected_model = self.model_select.read(cx).selected_model();
        if text.trim().is_empty()
            && selected_template_id.is_none()
            && selected_model.is_none()
            && mode == Mode::Contextual
        {
            return None;
        }
        let request_template = self
            .request_template
            .clone()
            .unwrap_or_else(|| serde_json::Value::Object(Default::default()));
        Some(ConversationDraft {
            text,
            provider_name: selected_model
                .as_ref()
                .map(|model| model.provider_name.clone())
                .unwrap_or_default(),
            model_id: selected_model
                .as_ref()
                .map(|model| model.id.clone())
                .unwrap_or_default(),
            mode,
            selected_template_id,
            request_template,
        })
    }

    pub(crate) fn restore_draft(
        &mut self,
        draft: ConversationDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_restore_draft = Some(draft.clone());
        self.input_state.update(cx, |input, cx| {
            input.set_value(draft.text.clone(), window, cx);
        });
        self.last_input_text = draft.text;
        self.mode_select.update(cx, |select, cx| {
            select.set_selected_mode(draft.mode, window, cx);
        });
        self.selected_template = draft
            .selected_template_id
            .and_then(|template_id| {
                self.templates
                    .iter()
                    .find(|template| template.id == template_id)
            })
            .cloned();
        self.try_restore_pending_draft(window, cx);
        cx.emit(ChatFormEvent::StateChanged);
        cx.notify();
    }

    fn try_restore_pending_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.pending_restore_draft.clone() else {
            return;
        };
        if draft.provider_name.is_empty() || draft.model_id.is_empty() {
            self.pending_restore_draft = None;
            return;
        }
        let Some(model) = self.model_select.update(cx, |select, cx| {
            select.restore_selected_model(&draft.provider_name, &draft.model_id, window, cx)
        }) else {
            return;
        };
        let base_template = match build_request_template(&model, Some(&draft.request_template)) {
            Ok(template) => template,
            Err(_) => return,
        };
        self.request_template = Some(base_template);
        self.sync_ext_settings(&model, window, cx);
        self.pending_restore_draft = None;
    }

    fn can_send(&self, cx: &App) -> bool {
        !self.running
            && !self.input_state.read(cx).value().trim().is_empty()
            && self.model_select.read(cx).selected_model().is_some()
            && self.request_template.is_some()
    }
}

impl Render for ChatForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let send_tooltip = cx.global::<I18n>().t("tooltip-send-message");
        let pause_tooltip = cx.global::<I18n>().t("tooltip-pause-message");
        let template_tooltip = cx.global::<I18n>().t("tooltip-select-template");
        let mut root = v_flex();
        root.style().align_items = Some(AlignItems::Stretch);

        root.w_full()
            .gap_2()
            .occlude()
            .bg(cx.theme().group_box)
            .rounded(px(18.))
            .border_1()
            .border_color(cx.theme().border.opacity(0.04))
            .shadow_2xl()
            .p_2()
            .relative()
            .child(
                div()
                    .flex()
                    .w_full()
                    .child(Input::new(&self.input_state).flex_1().appearance(false)),
            )
            .when_some(self.selected_template.clone(), |this, template| {
                let tag = Tag::primary()
                    .outline()
                    .child(format!("{} {}", template.icon, template.name));
                this.child(h_flex().gap_2().child(div().child(tag).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|form, _event, _window, cx| {
                        form.selected_template = None;
                        cx.emit(ChatFormEvent::StateChanged);
                        cx.notify();
                    }),
                )))
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(self.model_select.clone())
                            .child(self.ext_settings.clone())
                            .child(self.mode_select.clone())
                            .child(
                                Button::new("chat-form-template-picker")
                                    .icon(IconName::LayoutTemplate)
                                    .small()
                                    .ghost()
                                    .tooltip(template_tooltip)
                                    .on_click(cx.listener(|form, _, window, cx| {
                                        form.open_template_picker(None, window, cx);
                                    })),
                            ),
                    )
                    .child(div().flex_1())
                    .child(h_flex().items_center().gap_1().child(if self.running {
                        Button::new("pause")
                            .icon(IconName::X)
                            .small()
                            .tooltip(pause_tooltip)
                            .on_click(cx.listener(|_form, _event, _window, cx| {
                                cx.emit(ChatFormEvent::PauseRequested);
                            }))
                            .into_any_element()
                    } else {
                        Button::new("send")
                            .icon(IconName::Send)
                            .small()
                            .disabled(!self.can_send(cx))
                            .tooltip(send_tooltip)
                            .on_click(cx.listener(|form, _event, _window, cx| {
                                if form.can_send(cx) {
                                    cx.emit(ChatFormEvent::SendRequested);
                                }
                            }))
                            .into_any_element()
                    })),
            )
            .when(self.template_picker_open, |this| {
                this.child(self.render_template_picker(window, cx))
            })
            .child(
                canvas(
                    {
                        let state = cx.entity();
                        move |bounds, _, cx| {
                            state.update(cx, |form, _| {
                                form.template_picker_bounds = bounds;
                            })
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
    }
}

impl ChatForm {
    fn rebuild_template_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sections = template_sections(self.templates.clone());
        let selected_template_id = self.selected_template.as_ref().map(|template| template.id);
        let initial_ix =
            PickerListDelegate::selected_index_for(&sections, selected_template_id.as_ref());
        let state = cx.entity().downgrade();
        let on_confirm = Rc::new(
            move |template: TemplateOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.choose_template(template.into_template(), window, cx);
                });
            },
        );
        let state = cx.entity().downgrade();
        let on_cancel = Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |form, cx| {
                form.close_template_picker(true, true, window, cx);
            });
        });
        let empty_label = cx.global::<I18n>().t("empty-template-picker");
        self.template_picker = Some(cx.new(move |cx| {
            let mut state = ListState::new(
                PickerListDelegate::new(
                    sections.clone(),
                    false,
                    empty_label.into(),
                    on_confirm.clone(),
                    on_cancel.clone(),
                ),
                window,
                cx,
            )
            .searchable(true);
            state.set_selected_index(initial_ix, window, cx);
            state
        }));
    }

    fn focus_template_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(template_picker) = &self.template_picker {
            template_picker.update(cx, |picker, cx| {
                picker.focus(window, cx);
            });
        }
    }

    fn render_template_picker(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let template_picker = self
            .template_picker
            .clone()
            .expect("template picker exists while open");
        let on_mouse_down_out = cx.listener(|form, _event: &MouseDownEvent, window, cx| {
            form.close_template_picker(true, false, window, cx);
        });
        render_picker_popover(
            self.template_picker_bounds,
            template_picker,
            PickerPopoverOptions {
                max_width: Some(px(320.)),
                search_placeholder: Some(cx.global::<I18n>().t("field-search-template").into()),
                ..Default::default()
            },
            on_mouse_down_out,
            cx,
        )
    }
}

fn detect_template_trigger(
    previous_text: &str,
    current_text: &str,
    cursor: usize,
    cursor_position: Position,
) -> Option<(String, Position)> {
    if cursor == 0
        || current_text.len() != previous_text.len().saturating_add(1)
        || current_text.as_bytes().get(cursor - 1) != Some(&b'/')
    {
        return None;
    }
    let mut next = current_text.to_string();
    next.remove(cursor - 1);
    if next != previous_text {
        return None;
    }
    Some((
        next,
        Position {
            line: cursor_position.line,
            character: cursor_position.character.saturating_sub(1),
        },
    ))
}

fn reconcile_selected_template(
    selected_template: Option<ConversationTemplate>,
    templates: &[ConversationTemplate],
) -> (Option<ConversationTemplate>, bool) {
    let Some(selected_template) = selected_template else {
        return (None, false);
    };
    let Some(template) = templates
        .iter()
        .find(|template| template.id == selected_template.id)
        .cloned()
    else {
        return (None, true);
    };
    (Some(template), false)
}

#[cfg(test)]
mod tests {
    use super::{detect_template_trigger, reconcile_selected_template};
    use crate::database::{ConversationTemplate, ConversationTemplatePrompt, Role};
    use gpui_component::input::Position;
    use time::OffsetDateTime;

    fn template(id: i32, name: &str, prompt: &str) -> ConversationTemplate {
        ConversationTemplate {
            id,
            name: name.to_string(),
            icon: "🧩".to_string(),
            description: None,
            prompts: vec![ConversationTemplatePrompt {
                prompt: prompt.to_string(),
                role: Role::User,
            }],
            created_time: OffsetDateTime::UNIX_EPOCH,
            updated_time: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn detect_template_trigger_supports_middle_cursor_insertions() {
        let result = detect_template_trigger(
            "hello world",
            "hello /world",
            7,
            Position {
                line: 0,
                character: 7,
            },
        );

        assert_eq!(
            result,
            Some((
                "hello world".to_string(),
                Position {
                    line: 0,
                    character: 6,
                },
            ))
        );
    }

    #[test]
    fn detect_template_trigger_ignores_non_insert_changes() {
        let result = detect_template_trigger(
            "hello world",
            "hello /world!",
            7,
            Position {
                line: 0,
                character: 7,
            },
        );

        assert_eq!(result, None);
    }

    #[test]
    fn detect_template_trigger_ignores_non_slash_insertions() {
        let result = detect_template_trigger(
            "hello world",
            "hello !world",
            7,
            Position {
                line: 0,
                character: 7,
            },
        );

        assert_eq!(result, None);
    }

    #[test]
    fn reconcile_selected_template_refreshes_existing_selection() {
        let selected = template(1, "Old Template", "old prompt");
        let refreshed = template(1, "QA Single Prompt Template", "new prompt");

        let (selected_template, removed) =
            reconcile_selected_template(Some(selected), &[refreshed]);

        assert!(!removed);
        let selected_template = selected_template.expect("selected template should be refreshed");
        assert_eq!(selected_template.name, "QA Single Prompt Template");
        assert_eq!(selected_template.prompts[0].prompt, "new prompt");
    }

    #[test]
    fn reconcile_selected_template_clears_removed_selection() {
        let selected = template(1, "Deleted Template", "prompt");
        let other = template(2, "Other Template", "other prompt");

        let (selected_template, removed) = reconcile_selected_template(Some(selected), &[other]);

        assert!(removed);
        assert!(selected_template.is_none());
    }

    #[test]
    fn reconcile_selected_template_keeps_empty_selection_empty() {
        let refreshed = template(1, "QA Single Prompt Template", "prompt");

        let (selected_template, removed) = reconcile_selected_template(None, &[refreshed]);

        assert!(!removed);
        assert!(selected_template.is_none());
    }
}
