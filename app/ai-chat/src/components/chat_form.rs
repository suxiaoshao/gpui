mod provider_chat_form;
mod provider_template_form;
mod template_picker;

use crate::{
    config::AiChatConfig,
    database::{ConversationTemplate, ConversationTemplatePrompt, Db, Mode},
    errors::{AiChatError, AiChatResult},
    gpui_ext::WeakEntityResultExt,
    i18n::I18n,
    llm::{
        ProviderModel, available_models, chat_form_layout_by_provider, provider_by_name,
        template_inputs_by_provider,
    },
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable, Size, StyledExt as _,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState, Position},
    list::{List, ListState},
    select::{SearchableVec, Select, SelectEvent, SelectGroup, SelectItem, SelectState},
    tag::Tag,
    v_flex,
};
use provider_chat_form::ProviderChatFormView;
use provider_template_form::ProviderTemplateFormState;
use std::sync::Arc;
use template_picker::TemplatePickerDelegate;

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

#[derive(Clone, PartialEq, Eq)]
struct ModelSelectionValue {
    provider_name: String,
    model_id: String,
}

#[derive(Clone)]
struct ModelSelectItem {
    title: SharedString,
    value: ModelSelectionValue,
}

impl SelectItem for ModelSelectItem {
    type Value = ModelSelectionValue;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

fn mode_options(mode: Mode) -> Vec<Mode> {
    match mode {
        Mode::Contextual => vec![Mode::Contextual, Mode::Single, Mode::AssistantOnly],
        Mode::Single => vec![Mode::Single, Mode::Contextual, Mode::AssistantOnly],
        Mode::AssistantOnly => vec![Mode::AssistantOnly, Mode::Contextual, Mode::Single],
    }
}

pub(crate) struct ChatForm {
    input_state: Entity<InputState>,
    extension_state: Entity<SelectState<SearchableVec<String>>>,
    mode_state: Entity<SelectState<Vec<Mode>>>,
    model_state: Entity<SelectState<SearchableVec<SelectGroup<ModelSelectItem>>>>,
    models: Vec<ProviderModel>,
    templates: Vec<ConversationTemplate>,
    selected_model: Option<ProviderModel>,
    selected_template: Option<ConversationTemplate>,
    provider_chat_form: Option<Entity<ProviderChatFormView>>,
    template_picker: Option<Entity<ListState<TemplatePickerDelegate>>>,
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
        let extension_state = cx.new(|cx| {
            let extension_container = cx.global::<crate::extensions::ExtensionContainer>();
            let names = extension_container
                .get_all_config()
                .into_iter()
                .map(|config| config.name)
                .collect::<Vec<_>>();
            SelectState::new(SearchableVec::new(names), None, window, cx).searchable(true)
        });
        let mode_state = cx.new(|cx| {
            SelectState::new(
                mode_options(Mode::Contextual),
                Some(gpui_component::IndexPath::default()),
                window,
                cx,
            )
        });
        let model_state = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(Vec::<SelectGroup<ModelSelectItem>>::new()),
                None,
                window,
                cx,
            )
            .searchable(true)
        });
        let templates = cx
            .global::<Db>()
            .get()
            .ok()
            .and_then(|mut conn| ConversationTemplate::all(&mut conn).ok())
            .unwrap_or_default();

        let mut this = Self {
            input_state,
            extension_state,
            mode_state,
            model_state,
            models: Vec::new(),
            templates,
            selected_model: None,
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
        this.bind_model_events(window, cx);
        this.load_models(window, cx);
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

    fn bind_model_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model_subscription = cx.subscribe_in(
            &self.model_state,
            window,
            |this,
             _state,
             event: &SelectEvent<SearchableVec<SelectGroup<ModelSelectItem>>>,
             window,
             cx| {
                let SelectEvent::Confirm(value) = event;
                let Some(value) = value else {
                    this.selected_model = None;
                    this.provider_chat_form = None;
                    cx.notify();
                    return;
                };
                let Some(model) = this
                    .models
                    .iter()
                    .find(|model| {
                        model.provider_name == value.provider_name && model.id == value.model_id
                    })
                    .cloned()
                else {
                    return;
                };
                let provider = match provider_by_name(&model.provider_name) {
                    Ok(provider) => provider,
                    Err(_) => return,
                };
                let template = match provider.default_template_for_model(&model) {
                    Ok(template) => template,
                    Err(_) => return,
                };
                this.rebuild_provider_chat_form(&model, template, window, cx);
            },
        );
        self._subscriptions.push(model_subscription);
    }

    fn load_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = cx.entity().downgrade();
        let config = cx.global::<AiChatConfig>().clone();
        cx.spawn_in(window, async move |_, cx| {
            let result = available_models(config).await;
            let _ = state.update_in_result(cx, |this, window, cx| {
                if let Ok(models) = result {
                    this.set_models(models, window, cx);
                }
            });
        })
        .detach();
    }

    fn set_models(
        &mut self,
        models: Vec<ProviderModel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.models = models.clone();
        let mut provider_groups = std::collections::BTreeMap::<String, Vec<ModelSelectItem>>::new();
        for model in models {
            provider_groups
                .entry(model.provider_name.clone())
                .or_default()
                .push(ModelSelectItem {
                    title: model.id.clone().into(),
                    value: ModelSelectionValue {
                        provider_name: model.provider_name.clone(),
                        model_id: model.id.clone(),
                    },
                });
        }
        let groups = provider_groups
            .into_iter()
            .map(|(provider, mut items)| {
                items.sort_by(|left, right| left.title.cmp(&right.title));
                SelectGroup::new(provider).items(items)
            })
            .collect::<Vec<_>>();
        let selected = self.selected_model.as_ref().map(|model| {
            gpui_component::IndexPath::default()
                .section(
                    groups
                        .iter()
                        .position(|group| group.title == model.provider_name)
                        .unwrap_or_default(),
                )
                .row(
                    groups
                        .iter()
                        .find(|group| group.title == model.provider_name)
                        .and_then(|group| {
                            group
                                .items
                                .iter()
                                .position(|item| item.value.model_id == model.id)
                        })
                        .unwrap_or_default(),
                )
        });
        self.model_state = cx.new(|cx| {
            SelectState::new(SearchableVec::new(groups), selected, window, cx).searchable(true)
        });
        self.bind_model_events(window, cx);
        cx.notify();
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
                self.selected_model = Some(model.clone());
                self.provider_chat_form = None;
                cx.notify();
                return;
            }
        };
        let layout = match chat_form_layout_by_provider(&model.provider_name) {
            Ok(layout) => layout,
            Err(_) => {
                self.selected_model = Some(model.clone());
                self.provider_chat_form = None;
                cx.notify();
                return;
            }
        };
        let form =
            cx.new(|cx| ProviderTemplateFormState::new(template_items, &template, window, cx));
        let base_template = template.clone();
        self.selected_model = Some(model.clone());
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
        let Some(selected_model) = self.selected_model.clone() else {
            return Ok(None);
        };
        let Some(provider_chat_form) = &self.provider_chat_form else {
            return Ok(None);
        };
        let request_template = provider_chat_form
            .read(cx)
            .effective_template(cx)
            .map_err(AiChatError::StreamError)?;
        let mode = self
            .mode_state
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(Mode::Contextual);
        Ok(Some(ChatFormSnapshot {
            text: self.input_state.read(cx).value().to_string(),
            extension_name: self.extension_state.read(cx).selected_value().cloned(),
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
            && self.selected_model.is_some()
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
                            .child(
                                Select::new(&self.extension_state)
                                    .cleanable(true)
                                    .w(px(150.)),
                            )
                            .child(Select::new(&self.model_state).cleanable(true).w(px(220.)))
                            .child(Select::new(&self.mode_state).w(px(160.))),
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
        let templates = self.templates.clone();
        let selected_template = self.selected_template.clone();
        let initial_ix =
            TemplatePickerDelegate::selected_index_for(&templates, selected_template.as_ref());
        let state = cx.entity().downgrade();
        let on_confirm = Arc::new(
            move |template: ConversationTemplate, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |form, cx| {
                    form.choose_template(template, window, cx);
                });
            },
        );
        let state = cx.entity().downgrade();
        let on_cancel = Arc::new(move |window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |form, cx| {
                form.close_template_picker(true, true, window, cx);
            });
        });
        self.template_picker = Some(cx.new(move |cx| {
            let mut state = ListState::new(
                TemplatePickerDelegate::new(templates, on_confirm, on_cancel),
                window,
                cx,
            );
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
        let bounds = self.template_picker_bounds;
        let popup_radius = cx.theme().radius.min(px(8.));
        let template_picker = self
            .template_picker
            .clone()
            .expect("template picker exists while open");
        deferred(
            anchored()
                .anchor(Corner::BottomLeft)
                .snap_to_window_with_margin(px(8.))
                .position(point(bounds.left(), bounds.top()))
                .child(
                    div()
                        .w(bounds.size.width + px(2.))
                        .on_mouse_down_out(cx.listener(|form, _, window, cx| {
                            form.close_template_picker(true, false, window, cx);
                        }))
                        .child(
                            v_flex()
                                .occlude()
                                .mb_1p5()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().border)
                                .rounded(popup_radius)
                                .shadow_md()
                                .child(
                                    List::new(&template_picker)
                                        .with_size(Size::Medium)
                                        .max_h(rems(20.))
                                        .paddings(Edges::all(px(4.))),
                                ),
                        ),
                ),
        )
        .with_priority(1)
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
