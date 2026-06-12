use crate::{app::APP_NAME, features::temporary::TemporaryWindow, foundation::I18n};
use gpui::*;
use gpui_component::{Root, TitleBar};
use tracing::{Level, event};
use window_ext::WindowExt as _;

const TEMPORARY_WINDOW_SIZE: Size<Pixels> = size(px(960.), px(620.));

pub(crate) fn open_temporary_window(cx: &mut App) {
    if let Some(window) = find_temporary_window(cx) {
        if let Err(err) = window.update(cx, |root, window, cx| {
            reveal_temporary_window(root, window, cx);
        }) {
            event!(Level::ERROR, error = ?err, "activate ai-chat2 temporary window failed");
        }
        return;
    }

    let title = cx.global::<I18n>().t("temporary-window-title");
    let result = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(TEMPORARY_WINDOW_SIZE, cx)),
            titlebar: Some(temporary_titlebar_options(title)),
            window_background: WindowBackgroundAppearance::Opaque,
            is_resizable: true,
            kind: WindowKind::Normal,
            app_id: Some(APP_NAME.to_owned()),
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| TemporaryWindow::new(window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    );

    match result {
        Ok(window) => {
            let _ = window.update(cx, |root, window, cx| {
                reveal_temporary_window(root, window, cx);
            });
        }
        Err(err) => {
            event!(Level::ERROR, error = ?err, "open ai-chat2 temporary window failed");
        }
    }
}

fn find_temporary_window(cx: &App) -> Option<WindowHandle<Root>> {
    cx.windows().iter().find_map(|window| {
        let root = window.downcast::<Root>()?;
        let root_view = root.read(cx).ok()?.view().clone();
        root_view.downcast::<TemporaryWindow>().ok().map(|_| root)
    })
}

fn reveal_temporary_window(root: &mut Root, window: &mut Window, cx: &mut Context<Root>) {
    if let Err(err) = window.show() {
        event!(Level::ERROR, error = ?err, "show ai-chat2 temporary window failed");
    }
    window.activate_window();

    let _ = root
        .view()
        .clone()
        .downcast::<TemporaryWindow>()
        .map(|view| {
            view.update(cx, |view, cx| view.focus_search_input(window, cx));
        });
}

fn temporary_titlebar_options(title: impl Into<SharedString>) -> TitlebarOptions {
    TitlebarOptions {
        title: Some(title.into()),
        ..TitleBar::title_bar_options()
    }
}
