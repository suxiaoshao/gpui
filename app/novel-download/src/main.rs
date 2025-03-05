use gpui::*;
use views::WorkspaceView;

mod crawler;
mod errors;
mod views;

actions!(novel_download, [Quit]);
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
            |window, cx| cx.new(|cx| WorkspaceView::new(window, cx)),
        ) {
            // todo log
        };
    });
}

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}
