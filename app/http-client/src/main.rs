use errors::HttpClientResult;
use gpui::*;
use http_form::HttpFormView;

mod errors;
mod http_body;
mod http_form;
mod http_headers;
mod http_method;
mod http_params;
mod http_tab;
mod url_input;

fn main() -> HttpClientResult<()> {
    Application::new().run(|cx: &mut App| {
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
            |_, cx| cx.new(HttpFormView::new),
        ) {
            // todo log
        };
    });
    Ok(())
}
