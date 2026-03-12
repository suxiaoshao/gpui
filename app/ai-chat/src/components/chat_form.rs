mod extension_select;
mod mode_select;
mod model_select;
mod picker;
mod provider_chat_form;
mod provider_template_form;
mod template_picker;

use crate::{
    database::{ConversationTemplate, ConversationTemplatePrompt, Db, Mode},
    errors::{AiChatError, AiChatResult},
    i18n::I18n,
    llm::{ProviderModel, chat_form_layout_by_provider, provider_by_name, template_inputs_by_provider},
};
use extension_select::ExtensionSelect;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState, Position},
    list::ListState,
    tag::Tag,
    v_flex,
};
use mode_select::ModeSelect;
use model_select::{ModelSelect, ModelSelectEvent};
use picker::{PickerListDelegate, PickerPopoverOptions, render_picker_popover};
use provider_chat_form::ProviderChatFormView;
use provider_template_form::ProviderTemplateFormState;
use std::rc::Rc;
use template_picker::{TemplateOption, template_sections};

pub(crate) fn init(_cx: &mut App) {}

#[derive(Clone)]
pub(crate) enum ChatFormEvent {
    SendRequested,
    PauseRequested,
}

impl EventEmitter<ChatFormEvent> for ChatForm {}

#[derive(Clone)]
pub(crate) struct ChatFormSnapshot {
    pub(crate) text: String,
    pub(crate) extension_name: Option<String>,
    pub(crate) provider_name: String,
    pub(crate) request_template: serde_json::Value,
    pub(crate) prompts: Vec<ConversationTemplatePrompt>,
    pub(crate) mode: Mode,
}

pub(crate) struct ChatForm {
    input_state: Entity<InputState>,
    extension_select: Entity<ExtensionSelect>,
    mode_select: Entity<ModeSelect>,
    model_select: Entity<ModelSelect>,
    templates: Vec<ConversationTemplate>,
    selected_template: Option<ConversationTemplate>,
    provider_chat_form: Option<Entity<ProviderChatFormView>>,
    template_picker: Option<Entity<ListState<PickerListDelegate<TemplateOption>>>>,
    template_picker_bounds: Bounds<Pixels>,
    running: bool,
    template_picker_open: bool,
    slash_restore_position: Option<Position>,
    suppress_template_trigger_once: bool,
    last_input_text: String,
    _subscriptions: Vec<Subscription>,
}

impl ChatForm {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| InputState::new(window, cx).multi_line(true).auto_grow(3, 8));
        let extension_select = cx.new(|cx| ExtensionSelect::new(window, cx));
        let mode_select = cx.new(|cx| ModeSelect::new(window, cx));
        let model_select = cx.new(|cx| ModelSelect::new(window, cx));
        let templates = cx
            .global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| ConversationTemplate::all(&mut conn).ok())
            .unwrap_or_default();

        let mut this = Self {
            input_state,
            extension_select,
            mode_select,
            model_select,
            templates,
            selected_template: None,
            provider_chat_form: None,
            template_picker: None,
            template_picker_bounds: Bounds::default(),
            running: false,
            template_picker_open: false,
            slash_restore_position: None,
            suppress_template_trigger_once: false,
            last_input_text: String::new(),
            _subscriptions: Vec::new(),
        };
        this.bind_input_events(window, cx);
        this.bind_model_select_events(window, cx);
        this
    }

    fn bind_input_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_subscription = cx.subscribe_in(
            &self.input_state,
            window,
            |this, _input, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.on_input_change(window, cx),
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
                        Err(_) => return,
                    };
                    let template = match provider.default_template_for_model(model) {
                        Ok(template) => template,
                        Err(_) => return,
                    };
                    this.rebuild_provider_chat_form(model, template, window, cx);
                }
                ModelSelectEvent::Change(None) => {
                    this.provider_chat_form = None;
                    cx.notify();
                }
            },
        );
        self._subscriptions.push(model_select_subscription);
    }

    fn rebuild_provider_chat_form(
        &mut self,
        model: &ProviderModel,
        template: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let template_items = match template_inputs_by_provider(&model.provider_name) {
            Ok(items) => items,
            Err(_) => {
                self.provider_chat_form = None;
                cx.notify();
                return;
            }
        };
        let layout = match chat_form_layout_by_provider(&model.provider_name) {
            Ok(layout) => layout,
            Err(_) => {
                self.provider_chat_form = None;
                cx.notify();
                return;
            }
        };
        let form =
            cx.new(|cx| ProviderTemplateFormState::new(template_items, &template, window, cx));
        let base_template = template.clone();
        self.provider_chat_form =
            Some(cx.new(move |_cx| ProviderChatFormView::new(form, base_template, layout)));
        cx.notify();
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
        self.template_picker_open = true;
        self.slash_restore_position = Some(slash_restore_position);
        self.last_input_text = next.clone();
        self.rebuild_template_picker(window, cx);
        self.input_state.update(cx, |input, cx| {
            input.set_value(next, window, cx);
            input.set_cursor_position(slash_restore_position, window, cx);
        });
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
    }

    pub(crate) fn clear_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.last_input_text.clear();
    }

    pub(crate) fn set_running(&mut self, running: bool, cx: &mut Context<Self>) {
        self.running = running;
        cx.notify();
    }

    pub(crate) fn snapshot(&self, cx: &App) -> AiChatResult<Option<ChatFormSnapshot>> {
        let Some(selected_model) = self.model_select.read(cx).selected_model() else {
            return Ok(None);
        };
        let Some(provider_chat_form) = &self.provider_chat_form else {
            return Ok(None);
        };
        let request_template = provider_chat_form
            .read(cx)
            .effective_template(cx)
            .map_err(AiChatError::StreamError)?;
        let mode = self.mode_select.read(cx).selected_value();
        Ok(Some(ChatFormSnapshot {
            text: self.input_state.read(cx).value().to_string(),
            extension_name: self.extension_select.read(cx).selected_value().cloned(),
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

    fn can_send(&self, cx: &App) -> bool {
        !self.running
            && !self.input_state.read(cx).value().is_empty()
            && self.model_select.read(cx).selected_model().is_some()
            && self.provider_chat_form.is_some()
    }
}

impl Render for ChatForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let send_tooltip = cx.global::<I18n>().t("tooltip-send-message");
        let pause_tooltip = cx.global::<I18n>().t("tooltip-pause-message");
        let settings_tooltip = cx.global::<I18n>().t("tooltip-chat-form-settings");
        let mut root = v_flex();
        root.style().align_items = Some(AlignItems::Stretch);

        root.w_full()
            .gap_2()
            .bg(cx.theme().input)
            .rounded(cx.theme().radius)
            .p_1()
            .child(
                h_flex()
                    .relative()
                    .w_full()
                    .child(Input::new(&self.input_state).flex_1().appearance(false))
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
                    ),
            )
            .when_some(self.selected_template.clone(), |this, template| {
                let tag = Tag::primary()
                    .outline()
                    .child(format!("{} {}", template.icon, template.name));
                this.child(h_flex().gap_2().child(div().child(tag).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|form, _event, _window, cx| {
                        form.selected_template = None;
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
                            .child(self.extension_select.clone())
                            .child(self.model_select.clone())
                            .child(self.mode_select.clone()),
                    )
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .when_some(
                                self.provider_chat_form.clone(),
                                |this, provider_chat_form| this.child(provider_chat_form),
                            )
                            .when(self.provider_chat_form.is_none(), |this| {
                                this.child(
                                    Button::new("provider-chat-form-settings-disabled")
                                        .icon(IconName::Settings)
                                        .ghost()
                                        .small()
                                        .disabled(true)
                                        .tooltip(settings_tooltip.clone()),
                                )
                            })
                            .child(if self.running {
                                Button::new("pause")
                                    .icon(IconName::Close)
                                    .small()
                                    .tooltip(pause_tooltip)
                                    .on_click(cx.listener(|_form, _event, _window, cx| {
                                        cx.emit(ChatFormEvent::PauseRequested);
                                    }))
                                    .into_any_element()
                            } else {
                                Button::new("send")
                                    .icon(IconName::ArrowUp)
                                    .small()
                                    .disabled(!self.can_send(cx))
                                    .tooltip(send_tooltip)
                                    .on_click(cx.listener(|form, _event, _window, cx| {
                                        if form.can_send(cx) {
                                            cx.emit(ChatFormEvent::SendRequested);
                                        }
                                    }))
                                    .into_any_element()
                            }),
                    ),
            )
            .when(self.template_picker_open, |this| {
                this.child(self.render_template_picker(window, cx))
            })
    }
}

impl ChatForm {
    fn rebuild_template_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sections = template_sections(self.templates.clone());
        let selected_template_id = self.selected_template.as_ref().map(|template| template.id);
        let initial_ix =
            PickerListDelegate::selected_index_for(&sections, selected_template_id.as_ref());
        let state = cx.entity().downgrade();
        let on_confirm = Rc::new(move |template: TemplateOption, window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |form, cx| {
                form.choose_template(template.into_template(), window, cx);
            });
        });
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

#[cfg(test)]
mod tests {
    use super::detect_template_trigger;
    use gpui_component::input::Position;

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
}
