use gpui::*;
use http_form::HttpFormView;

mod http_form;
mod http_method;

fn main() {
    App::new().run(|cx: &mut AppContext| {
        let theme = theme::get_theme();
        cx.set_global(theme);
        let bounds = Bounds::centered(None, size(px(600.0), px(600.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |cx| cx.new_view(HttpFormView::new),
        );
    });
}
