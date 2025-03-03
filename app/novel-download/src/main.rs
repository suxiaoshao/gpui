use gpui::*;
use gpui_component::{StyledExt, button::Button, input::TextInput};

mod crawler;
mod errors;

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
}

impl Render for Example {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().key_context("NovelDownload").p_4().size_full().child(
            div().h_flex().gap_1().child(self.input.clone()).child(
                Button::new("send")
                    .on_click(cx.listener(|this, _, _, cx| {
                        println!("{}", this.input.read(cx).text());
                    }))
                    .child("send"),
            ),
        )
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
