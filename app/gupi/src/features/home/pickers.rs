mod models;
#[cfg(test)]
mod tests;
use super::*;
use crate::state::conversation::Session;
use gpui_kit::component::{
    Icon, StyledExt,
    list::List,
    popover::Popover,
    slider::{Slider, SliderEvent, SliderState},
    spinner::Spinner,
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder;
use models::{ModelKey, ModelList, ModelOption};

#[derive(Clone)]
pub(super) enum PickerEvent {
    Model(ModelKey),
    Thinking(String),
    Load,
    Refresh,
}

#[derive(Clone, Default, PartialEq, Eq)]
/// Computed for the current call; the picker never stores or mutates these facts.
pub(super) struct Projection {
    models: Vec<ModelOption>,
    selected: Option<ModelKey>,
    name: String,
    reasoning: bool,
    levels: Vec<String>,
    level: String,
    disabled: bool,
    unloaded: bool,
    model_error: Option<String>,
    thinking_error: Option<String>,
    change_error: Option<String>,
    models_loading: bool,
    models_unloaded: bool,
    thinking_loading: bool,
    changing: bool,
    unconfirmed: bool,
}
impl Projection {
    pub fn from_session(session: &Session) -> Self {
        let model = session.state.as_ref().and_then(|s| s.model.as_ref());
        Self {
            models: session
                .model_options()
                .iter()
                .map(ModelOption::from)
                .collect(),
            selected: model.map(ModelKey::from),
            name: model.map(|m| m.name.clone()).unwrap_or_default(),
            reasoning: model.is_some_and(|m| m.reasoning),
            levels: session.levels().to_vec(),
            level: session
                .state
                .as_ref()
                .map(|s| s.thinking_level.clone())
                .unwrap_or_default(),
            // Keep old model capabilities inert until the post-command snapshot arrives.
            disabled: session.settings_busy()
                || session.state.is_none()
                    && (session.core_read.running()
                        || session.instance.is_some() && session.core_read.error().is_none()),
            unloaded: session.state.is_none(),
            model_error: session.models.error().map(str::to_owned),
            thinking_error: session.thinking_levels.error().map(str::to_owned),
            change_error: session.model_change.error().map(str::to_owned),
            models_loading: session.models_loading(),
            models_unloaded: matches!(
                session.models,
                crate::state::conversation::loading::ReadState::Idle
            ),
            thinking_loading: session.thinking_levels.running(),
            changing: session.model_change.running(),
            unconfirmed: session.model_change.unconfirmed(),
        }
    }
}

impl Projection {
    fn can_select_model(&self) -> bool {
        !self.disabled && !self.models_loading && !self.unconfirmed && self.model_error.is_none()
    }
    fn can_think(&self) -> bool {
        !self.disabled
            && !self.thinking_loading
            && !self.unconfirmed
            && self.thinking_error.is_none()
            && self.reasoning
            && self.levels.len() > 1
            && self.levels.contains(&self.level)
    }
    fn has_feedback(&self) -> bool {
        self.models_loading
            || self.thinking_loading
            || self.changing
            || self.model_error.is_some()
            || self.thinking_error.is_some()
            || self.change_error.is_some()
    }
}

/// Only the canonical slider binding is cached, to preserve an in-progress drag.
#[derive(PartialEq, Eq)]
struct SliderBinding {
    model: Option<ModelKey>,
    levels: Vec<String>,
    level: String,
}
impl From<&Projection> for SliderBinding {
    fn from(data: &Projection) -> Self {
        Self {
            model: data.selected.clone(),
            levels: data.levels.clone(),
            level: data.level.clone(),
        }
    }
}

pub(super) struct Picker {
    query: Box<dyn Fn(&App) -> Projection>,
    slider_binding: Option<SliderBinding>,
    list: Entity<ListState<ModelList>>,
    slider: Entity<SliderState>,
    focus: FocusHandle,
    open: bool,
    models_page: bool,
    draft_level: Option<String>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<PickerEvent> for Picker {}
impl Picker {
    pub fn new(
        query: impl Fn(&App) -> Projection + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let list = cx.new(|cx| ListState::new(ModelList::default(), window, cx).searchable(true));
        let slider = cx.new(|_| SliderState::new().min(0.).max(1.).step(1.));
        let subscriptions = vec![
            cx.observe(&list, |_, _, cx| cx.notify()),
            cx.subscribe_in(&list, window, |this, list, event, window, cx| {
                let data = this.query(cx);
                match event {
                    ListEvent::Confirm(ix) if data.can_select_model() => {
                        let key = list.read(cx).delegate().item(*ix).map(|m| m.key.clone());
                        if let Some(key) = key {
                            this.models_page = false;
                            this.focus.focus(window, cx);
                            if data.selected.as_ref() != Some(&key) {
                                cx.emit(PickerEvent::Model(key));
                            }
                            cx.notify();
                        }
                    }
                    ListEvent::Cancel => {
                        this.models_page = false;
                        this.focus.focus(window, cx);
                        cx.notify();
                    }
                    _ => {}
                }
            }),
            cx.subscribe_in(&slider, window, |this, slider, event, window, cx| {
                this.slider_event(slider.clone(), event, window, cx)
            }),
        ];
        Self {
            query: Box::new(query),
            slider_binding: None,
            list,
            slider,
            focus: cx.focus_handle(),
            open: false,
            models_page: false,
            draft_level: None,
            _subscriptions: subscriptions,
        }
    }
    fn query(&self, cx: &App) -> Projection {
        (self.query)(cx)
    }
    pub fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let data = self.query(cx);
        let binding = SliderBinding::from(&data);
        let reset_slider = self.slider_binding.as_ref() != Some(&binding) || !data.can_think();
        self.list.update(cx, |list, cx| {
            let cursor = list
                .selected_index()
                .and_then(|ix| list.delegate().item(ix))
                .map(|m| m.key.clone());
            let delegate = list.delegate_mut();
            delegate.replace(
                data.models.clone(),
                data.selected.clone(),
                !data.can_select_model(),
            );
            let index = cursor
                .as_ref()
                .or(data.selected.as_ref())
                .and_then(|key| delegate.position(key));
            list.set_selected_index(index, window, cx);
            cx.notify();
        });
        self.slider_binding = Some(binding);
        if reset_slider {
            self.reset_slider(window, cx);
        }
        cx.notify();
    }
    fn reset_slider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let data = self.query(cx);
        self.draft_level = None;
        let max = data.levels.len().saturating_sub(1).max(1) as f32;
        let value = data
            .levels
            .iter()
            .position(|v| v == &data.level)
            .unwrap_or(0) as f32;
        self.slider.update(cx, |slider, cx| {
            *slider = SliderState::new().min(0.).max(max).step(1.);
            slider.set_value(value, window, cx);
        });
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = false;
        self.models_page = false;
        self.reset_slider(window, cx);
        cx.notify();
    }
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.open = open;
        self.models_page = false;
        self.reset_slider(window, cx);
        let data = self.query(cx);
        if open && data.models_unloaded && !data.disabled && !data.models_loading {
            cx.emit(PickerEvent::Load);
        }
        cx.notify();
    }
    fn slider_event(
        &mut self,
        _: Entity<SliderState>,
        event: &SliderEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let data = self.query(cx);
        if !self.open || !data.can_think() {
            return;
        }
        match event {
            SliderEvent::Change(value) => {
                self.draft_level = data.levels.get(value.start().round() as usize).cloned();
                self.focus.focus(window, cx);
                cx.notify();
            }
            SliderEvent::Release(value) => {
                if let Some(level) = data.levels.get(value.start().round() as usize).cloned() {
                    self.commit_level(level, cx);
                }
            }
        }
    }
    fn commit_level(&mut self, level: String, cx: &mut Context<Self>) {
        let data = self.query(cx);
        if !data.can_think() || !data.levels.contains(&level) {
            return;
        }
        self.draft_level = None;
        if level != data.level {
            cx.emit(PickerEvent::Thinking(level));
        }
        cx.notify();
    }
    fn show_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let data = self.query(cx);
        if !data.can_select_model() {
            return;
        }
        if data.unloaded {
            cx.emit(PickerEvent::Load);
            cx.notify();
            return;
        }
        self.models_page = true;
        self.list.update(cx, |list, cx| {
            list.set_query("", window, cx);
            let selected = data
                .selected
                .as_ref()
                .and_then(|key| list.delegate().position(key));
            list.set_selected_index(selected, window, cx);
            list.focus(window, cx);
        });
        cx.notify();
    }
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let data = self.query(cx);
        if data.disabled || data.models_loading || data.thinking_loading {
            return;
        }
        self.reset_slider(window, cx);
        cx.emit(if data.unloaded {
            PickerEvent::Load
        } else {
            PickerEvent::Refresh
        });
        cx.notify();
    }
    fn content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let data = self.query(cx);
        let mut panel = v_flex()
            .w(px(if self.models_page { 336. } else { 256. })
                .min(window.viewport_size().width - px(40.)))
            .gap_3();
        if self.models_page {
            panel = panel.child(
                Button::new("model-back")
                    .ghost()
                    .small()
                    .icon(IconName::ChevronLeft)
                    .label(t(cx, "composer-model-thinking"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.models_page = false;
                        this.focus.focus(window, cx);
                        cx.notify();
                    })),
            );
            return panel
                .when(data.has_feedback(), |panel| {
                    panel.child(self.load_feedback(cx))
                })
                .child(
                    List::new(&self.list)
                        .max_h(rems(18.))
                        .search_placeholder(t(cx, "conversation-model-search")),
                )
                .into_any_element();
        }
        let name = if data.name.is_empty() {
            t(cx, "conversation-model")
        } else {
            data.name.clone()
        };
        let model_row = Button::new("choose-model")
            .ghost()
            .h_8()
            .min_w_0()
            .max_w_full()
            .px_1()
            .disabled(
                data.disabled
                    || data.models_loading
                    || data.unconfirmed
                    || data.model_error.is_some(),
            )
            .accessibility_label(t(cx, "conversation-model"))
            .tooltip(name.clone())
            .child(
                h_flex()
                    .min_w_0()
                    .max_w_full()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        div()
                            .min_w_0()
                            .text_sm()
                            .text_center()
                            .truncate()
                            .child(name),
                    )
                    .child(Icon::new(IconName::ChevronRight).size_4().flex_none()),
            )
            .on_click(cx.listener(|this, _, window, cx| this.show_models(window, cx)));
        let mut provider = h_flex()
            .id("model-provider")
            .size_8()
            .flex_none()
            .justify_center();
        if let Some(model) = &data.selected {
            let name = model.provider.clone();
            provider = provider
                .role(Role::Image)
                .aria_label(name.clone())
                .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
                .child(
                    if let Some(icon) =
                        crate::foundation::assets::provider_logo_icon(&model.provider)
                    {
                        icon.size_4().into_any_element()
                    } else {
                        div()
                            .min_w_0()
                            .text_xs()
                            .truncate()
                            .child(model.provider.clone())
                            .into_any_element()
                    },
                );
        }
        panel = panel.child(
            h_flex()
                .gap_1()
                .items_center()
                .child(provider)
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .justify_center()
                        .child(model_row),
                )
                .child(
                    Button::new("refresh-model-options")
                        .ghost()
                        .small()
                        .size_8()
                        .flex_none()
                        .icon(IconName::RefreshCw)
                        .tooltip(t(cx, "composer-model-refresh"))
                        .accessibility_label(t(cx, "composer-model-refresh"))
                        .disabled(data.disabled || data.models_loading || data.thinking_loading)
                        .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx))),
                ),
        );
        if data.unloaded {
            return panel
                .when(data.has_feedback(), |panel| {
                    panel.child(self.load_feedback(cx))
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(cx, "conversation-load-options")),
                )
                .into_any_element();
        }
        if data.has_feedback() {
            panel = panel.child(self.load_feedback(cx));
        }
        let level = self.draft_level.as_ref().unwrap_or(&data.level);
        let label = if !data.reasoning {
            t(cx, "composer-thinking-unavailable")
        } else if level.is_empty() {
            t(cx, "conversation-unknown")
        } else {
            thinking_label(level, cx)
        };
        let mut effort = v_flex().gap_1();
        if data.reasoning && data.levels.len() > 1 {
            let disabled = !data.can_think();
            effort = effort
                .child(
                    div()
                        .id("thinking-slider")
                        .track_focus(&self.focus)
                        .tab_stop(true)
                        .aria_label(t(cx, "conversation-thinking"))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            let data = this.query(cx);
                            if !data.can_think() || event.is_held {
                                return;
                            }
                            let current = data
                                .levels
                                .iter()
                                .position(|l| l == &data.level)
                                .unwrap_or(0);
                            let index = match event.keystroke.key.as_str() {
                                "left" => current.saturating_sub(1),
                                "right" => (current + 1).min(data.levels.len() - 1),
                                "home" => 0,
                                "end" => data.levels.len() - 1,
                                _ => return,
                            };
                            let level = data.levels[index].clone();
                            this.slider
                                .update(cx, |s, cx| s.set_value(index as f32, window, cx));
                            this.commit_level(level, cx);
                            cx.stop_propagation();
                        }))
                        .child(Slider::new(&self.slider).disabled(disabled)),
                )
                .child(h_flex().justify_between().gap_1().children(
                    data.levels.iter().enumerate().map(|(i, level)| {
                        let selected = self.draft_level.as_ref().unwrap_or(&data.level) == level;
                        let level = level.clone();
                        Button::new(("thinking-level", i))
                            .ghost()
                            .xsmall()
                            .px_1()
                            .label(thinking_label(&level, cx))
                            .when(selected, |button| {
                                button.text_color(cx.theme().primary).font_medium()
                            })
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.slider
                                    .update(cx, |s, cx| s.set_value(i as f32, window, cx));
                                this.commit_level(level.clone(), cx);
                            }))
                    }),
                ));
        } else {
            effort = effort.child(
                div()
                    .text_sm()
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            );
        }
        panel.child(effort).into_any_element()
    }
    fn load_feedback(&self, cx: &App) -> AnyElement {
        let data = self.query(cx);
        let mut feedback = v_flex().gap_1();
        for (loading, key) in [
            (data.models_loading, "composer-model-loading"),
            (data.thinking_loading, "composer-thinking-loading"),
            (data.changing, "composer-model-confirming"),
        ] {
            if loading {
                feedback = feedback.child(
                    h_flex()
                        .gap_2()
                        .child(Spinner::new().small())
                        .child(div().text_xs().child(t(cx, key))),
                );
            }
        }
        for error in [&data.model_error, &data.thinking_error, &data.change_error]
            .into_iter()
            .flatten()
        {
            feedback = feedback.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }
        feedback.into_any_element()
    }
}
impl Render for Picker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.query(cx);
        let name = if data.name.is_empty() {
            t(cx, "conversation-model")
        } else {
            data.name.clone()
        };
        let level = thinking_label(&data.level, cx);
        let trigger = Button::new("model-thinking")
            .ghost()
            .small()
            .h_8()
            .min_w_0()
            .max_w_full()
            .px_2()
            .accessibility_label(t(cx, "composer-model-thinking"))
            .when(!self.open, |button| {
                button.tooltip(format!("{name}\n{level}"))
            })
            .child(
                h_flex()
                    .min_w_0()
                    .gap_1p5()
                    .child(div().min_w_0().truncate().child(name))
                    .when(data.reasoning && !level.is_empty(), |row| {
                        row.child(
                            div()
                                .flex_none()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("· {level}")),
                        )
                    })
                    .child(Icon::new(IconName::ChevronDown).size_3().flex_none()),
            );
        let owner = cx.entity();
        Popover::new("model-thinking-popover")
            .anchor(Anchor::BottomRight)
            .open(self.open)
            .trigger(trigger)
            .on_open_change(cx.listener(|this, open, window, cx| {
                this.set_open(*open, window, cx);
            }))
            .content(move |_, window, cx| owner.update(cx, |this, cx| this.content(window, cx)))
    }
}

pub(super) fn thinking_label(level: &str, cx: &App) -> String {
    let key = match level {
        "off" => "conversation-thinking-off",
        "minimal" => "conversation-thinking-minimal",
        "low" => "conversation-thinking-low",
        "medium" => "conversation-thinking-medium",
        "high" => "conversation-thinking-high",
        "xhigh" => "conversation-thinking-xhigh",
        "max" => "conversation-thinking-max",
        _ => return level.to_owned(),
    };
    t(cx, key)
}
