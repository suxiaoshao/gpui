use errors::FeiwenResult;
use gpui::*;
use gpui_component::Root;
use views::WorkspaceView;

mod errors;
mod fetch;
mod store;
mod views;

actions!(feiwen, [Quit]);

fn quit(_: &Quit, cx: &mut App) {
    // event!(Level::INFO, "quit by action");
    cx.quit();
}

fn init(cx: &mut App) {
    gpui_component::init(cx);
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    cx.activate(true);
    cx.on_action(quit);

    store::init_store(cx);
}

fn main() -> FeiwenResult<()> {
    let app = Application::new().with_assets(gpui_component_assets::Assets);
    app.run(|cx: &mut App| {
        init(cx);
        if let Err(_err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("HTTP Client".into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| WorkspaceView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ) {
            // todo log
        };
    });
    Ok(())
}
