use crate::database::ConversationTemplate;
use gpui::*;
use std::rc::Rc;

actions!([Esc]);

const CONTEXT: &str = "template-detail";

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Esc, Some(CONTEXT))]);
}

type OnEsc = Rc<dyn Fn(&Esc, &mut Window, &mut App) + 'static>;

pub(crate) struct TemplateDetailView {
    focus_handle: FocusHandle,
    on_esc: OnEsc,
}

impl TemplateDetailView {
    pub fn new(
        template: &ConversationTemplate,
        on_esc: impl Fn(&Esc, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        Self {
            focus_handle,
            on_esc: Rc::new(on_esc),
        }
    }
}

impl Render for TemplateDetailView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let on_esc = self.on_esc.clone();
        div()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(move |action, window, cx| {
                (on_esc)(action, window, cx);
            })
            .size_full()
    }
}
