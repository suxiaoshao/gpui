use crate::{
    hotkey::GlobalHotkeyState,
    i18n::I18n,
    views::temporary::detail::{TemplateDetailView, TemporaryDetailState},
};
use gpui::*;
use gpui_component::{Root, TitleBar};
use tracing::{Level, event};

pub(crate) mod detail;

pub fn init(cx: &mut App) {
    event!(Level::INFO, "Initializing temporary view");
    detail::init(cx);
}

pub(crate) struct TemporaryView {
    _subscription: Vec<Subscription>,
    pub(crate) detail: Entity<TemplateDetailView>,
}

impl TemporaryView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscription = vec![cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                GlobalHotkeyState::request_hide_with_delay(window, cx);
                return;
            }
            this.focus_chat_form(window, cx);
        })];
        let detail = cx.new(|cx| TemplateDetailView::new(window, cx));
        Self {
            _subscription,
            detail,
        }
    }

    pub(crate) fn focus_chat_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.detail
            .update(cx, |detail, cx| detail.focus_chat_form(window, cx));
    }
}

impl Render for TemporaryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.detail.clone())
    }
}

pub(crate) struct DetachedTemporaryView {
    pub(crate) detail: Entity<TemplateDetailView>,
}

impl DetachedTemporaryView {
    fn new_with_state(
        state: TemporaryDetailState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let detail = cx.new(|cx| TemplateDetailView::new_with_state(state, window, cx));
        Self { detail }
    }

    pub(crate) fn focus_chat_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.detail
            .update(cx, |detail, cx| detail.focus_chat_form(window, cx));
    }
}

impl Render for DetachedTemporaryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.detail.clone())
    }
}

pub(crate) fn open_detached_temporary_window(
    state: TemporaryDetailState,
    cx: &mut App,
) -> anyhow::Result<WindowHandle<Root>> {
    let title = cx.global::<I18n>().t("temporary-chat-title");
    let handle = cx.open_window(
        WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..TitleBar::title_bar_options()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(840.), px(680.)),
                cx,
            ))),
            ..Default::default()
        },
        move |window, cx| {
            let view = cx.new(|cx| DetachedTemporaryView::new_with_state(state, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    )?;

    if let Err(err) = handle.update(cx, |root, window, cx| {
        if let Ok(view) = root.view().clone().downcast::<DetachedTemporaryView>() {
            view.update(cx, |view, cx| view.focus_chat_form(window, cx));
        }
    }) {
        event!(Level::ERROR, error = ?err, "Failed to focus detached temporary window");
    }

    Ok(handle)
}
