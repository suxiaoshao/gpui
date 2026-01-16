use crate::{
    database::{ConversationTemplate, Db},
    errors::AiChatResult,
    hotkey::TemporaryData,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Sizable,
    alert::Alert,
    divider::Divider,
    h_flex,
    input::{Input, InputState},
    label::Label,
    tag::Tag,
    v_flex,
};
use tracing::{Level, event};

const CONTEXT: &str = "temporary-list";

actions!([Up, Down, Enter]);

pub(crate) fn init(cx: &mut App) {
    event!(Level::INFO, "Initializing temporary view");
    cx.bind_keys([
        KeyBinding::new("up", Up, Some(CONTEXT)),
        KeyBinding::new("down", Down, Some(CONTEXT)),
        KeyBinding::new("enter", Enter, Some(CONTEXT)),
    ]);
}

pub(crate) struct TemporaryView {
    _subscription: Vec<Subscription>,
    search_input: Entity<InputState>,
    templates: AiChatResult<Vec<ConversationTemplate>>,
    selected_index: Option<usize>,
    focus_handle: FocusHandle,
}

impl TemporaryView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscription = vec![cx.observe_window_activation(window, |_this, window, cx| {
            if !window.is_window_active() {
                window.remove_window();
                let data = cx.global_mut::<TemporaryData>();
                data.temporary_window = None;
            }
        })];
        let templates = Self::get_templates(cx);
        let search_input = cx.new(|cx| InputState::new(window, cx));
        search_input.focus_handle(cx).focus(window);
        Self {
            _subscription,
            search_input,
            templates,
            selected_index: None,
            focus_handle: cx.focus_handle(),
        }
    }
    fn get_templates(cx: &mut Context<Self>) -> AiChatResult<Vec<ConversationTemplate>> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::all(conn)
    }
    fn on_up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.selected_index {
            self.selected_index = Some(index.saturating_sub(1));
        } else if let Ok(templates) = &self.templates {
            self.selected_index = Some(templates.len().saturating_sub(1));
        }
    }
    fn on_down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.selected_index {
            self.selected_index = Some(index.saturating_add(1));
        } else if self.templates.is_ok() {
            self.selected_index = Some(0);
        }
    }
    fn on_enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.selected_index {}
    }
}

impl Render for TemporaryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_up))
            .on_action(cx.listener(Self::on_down))
            .on_action(cx.listener(Self::on_enter))
            .size_full()
            .child(Input::new(&self.search_input).bordered(false).large())
            .child(Divider::horizontal())
            .map(|this| match &self.templates {
                Ok(templates) => this.children(templates.iter().enumerate().map(
                    |(
                        index,
                        ConversationTemplate {
                            id,
                            name,
                            icon,
                            description,
                            mode,
                            ..
                        },
                    )| {
                        h_flex()
                            .id(*id)
                            .gap_2()
                            .p_4()
                            .when(
                                matches!(self.selected_index, Some(selected_index)  if selected_index == index),
                                |this| this.bg(cx.theme().accent),
                            )
                            .child(Label::new(icon))
                            .child(
                                Label::new(name)
                                    .when_some(description.as_ref(), |this, description| {
                                        this.secondary(description)
                                    }),
                            )
                            .child(
                                match mode {
                                    crate::database::Mode::Contextual => Tag::primary(),
                                    crate::database::Mode::Single => Tag::info(),
                                    crate::database::Mode::AssistantOnly => Tag::success(),
                                }
                                .child(mode.to_string())
                                .outline(),
                            )
                    },
                )),
                Err(err) => {
                    this.child(Alert::error("temporary-alert", err.to_string()).title("Error"))
                }
            })
    }
}
