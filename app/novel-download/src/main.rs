use gpui::*;
use gpui_component::input::TextInput;

actions!(novel_download, [Quit]);

pub struct Example {
    input: Entity<TextInput>,
}

impl Example {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            input: cx.new(|cx| TextInput::new(window, cx)),
        }
    }

    fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
}

impl Render for Example {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("NovelDownload")
            .p_4()
            .size_full()
            .child("helllo")
            .child(self.input.clone())
    }
}

fn main() {
    let app = Application::new();

    app.run(move |cx| {
        gpui_component::init(cx);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.activate(true);
        cx.on_action(quit);

        if let Err(_err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Novel Download".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Example::new(window, cx)),
        ) {
            // todo log
        };
    });
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}
