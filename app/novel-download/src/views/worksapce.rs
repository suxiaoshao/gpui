use gpui::*;
use gpui_component::{StyledExt, button::Button, input::TextInput};

pub(crate) enum WorkspaceEvent {
    Send(String),
}

#[derive(Default)]
struct Workspace {}

impl EventEmitter<WorkspaceEvent> for Workspace {}

pub struct WorkspaceView {
    input: Entity<TextInput>,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
}

impl WorkspaceView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspce = cx.new(|_| Workspace::default());
        cx.subscribe(&workspce, Self::subscribe).detach();
        Self {
            input: cx.new(|cx| TextInput::new(window, cx)),
            workspace: workspce,
            focus_handle: cx.focus_handle(),
        }
    }
    fn subscribe(
        &mut self,
        subscriber: Entity<Workspace>,
        emitter: &WorkspaceEvent,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            WorkspaceEvent::Send(text) => {
                println!("{}", text);
            }
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .key_context("NovelDownload")
            .p_4()
            .size_full()
            .child(
                div().h_flex().gap_1().child(self.input.clone()).child(
                    Button::new("send")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.workspace.update(cx, |_data, cx| {
                                let text = this.input.read(cx).text();
                                cx.emit(WorkspaceEvent::Send(text.to_string()));
                                this.input.update(cx, |this, cx| {
                                    this.set_text("", window, cx);
                                });
                                this.focus_handle.focus(window);
                            });
                        }))
                        .child("send")
                        .track_focus(&self.focus_handle),
                ),
            )
    }
}
