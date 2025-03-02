use errors::FeiwenResult;
use gpui::*;
use views::WorkspaceView;

mod errors;
mod fetch;
mod store;
mod views;

fn main() -> FeiwenResult<()> {
    Application::new().run(|cx: &mut App| {
        let theme = theme::get_theme();
        cx.set_global(theme);
        components::bind_input_keys(cx);
        store::init_store(cx);

        if let Err(_err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("HTTP Client".into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |_, cx| cx.new(WorkspaceView::new),
        ) {
            // todo log
        };
    });
    Ok(())
}
