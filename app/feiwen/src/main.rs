use errors::FeiwenResult;
use gpui::*;
use views::WorkspaceView;

mod errors;
mod views;

fn main() -> FeiwenResult<()> {
    App::new().run(|cx: &mut AppContext| {
        let theme = theme::get_theme();
        cx.set_global(theme);
        components::bind_input_keys(cx);

        if let Err(_err) = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("HTTP Client".into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |cx| cx.new_view(WorkspaceView::new),
        ) {
            // todo log
        };
    });
    Ok(())
}
