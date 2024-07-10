use errors::FeiwenResult;
use gpui::*;

mod errors;

struct TestView;

impl Render for TestView {
    fn render(&mut self, _cx: &mut ViewContext<Self>) -> impl IntoElement {
        div()
    }
}

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
            |cx| cx.new_view(|_cx| TestView),
        ) {
            // todo log
        };
    });
    Ok(())
}
