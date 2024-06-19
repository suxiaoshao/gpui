use gpui::*;
use http_form::HttpFormView;

mod http_form;
mod http_method;
mod http_tab;

fn main() {
    App::new().run(|cx: &mut AppContext| {
        let theme = theme::get_theme();
        cx.set_global(theme);

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("HTTP Client".into()),
                    ..Default::default()
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                ..Default::default()
            },
            |cx| cx.new_view(HttpFormView::new),
        );
    });
}
