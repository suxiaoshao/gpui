use crate::{
    features::home::actions::{Kind, Run},
    foundation::{i18n::t, paths},
};
use gpui_kit::component::{GlobalState, WindowExt, button::Button, input, v_flex};
use gpui_kit::*;

actions!(
    gupi,
    [
        Quit,
        ShowSettings,
        ShowMainWindow,
        ShowCommandPalette,
        ShowTemporaryWindow,
        About,
        Minimize,
        Zoom,
        Fullscreen,
        Hide,
        HideOthers,
        ShowAll,
        UserGuide,
        PiDocs,
        ReportIssue,
        ShowLogs,
        CopyDiagnostics
    ]
);
struct AppliedLocale(&'static str);
impl Global for AppliedLocale {}
/// Signals only menu definition changes, not unrelated component global state.
pub(crate) struct MenusChanged;
impl Global for MenusChanged {}

pub(crate) const CONVERSATION_COMMANDS: [Kind; 6] = [
    Kind::New,
    Kind::QuickOpen,
    Kind::Rename,
    Kind::Export,
    Kind::Sidebar,
    Kind::History,
];
#[derive(Default, PartialEq)]
struct ConversationCommands([bool; 6]);
impl Global for ConversationCommands {}
/// Native menus capture enabled state. Only rebuild when the active window's
/// capabilities change, not for each streamed message or render.
pub(crate) fn conversation_commands(enabled: [bool; 6], window: &Window, cx: &mut App) {
    if !window.is_window_active() {
        return;
    }
    let value = ConversationCommands(enabled);
    if cx.try_global::<ConversationCommands>() == Some(&value) {
        return;
    }
    cx.set_global(value);
    refresh_native(cx);
}

pub(crate) fn init(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        cx.bind_keys([
            KeyBinding::new("cmd-h", Hide, None),
            KeyBinding::new("alt-cmd-h", HideOthers, None),
            KeyBinding::new("cmd-m", Minimize, None),
            KeyBinding::new("ctrl-cmd-f", Fullscreen, None),
        ]);
        cx.on_action(|_: &Hide, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    }
    cx.on_action(|_: &Minimize, cx| with_window(cx, |window, _| window.minimize_window()));
    cx.on_action(|_: &Zoom, cx| with_window(cx, |window, _| window.zoom_window()));
    cx.on_action(|_: &Fullscreen, cx| with_window(cx, |window, _| window.toggle_fullscreen()));
    cx.on_action(|_: &UserGuide, cx| {
        cx.open_url("https://github.com/suxiaoshao/gpui/tree/main/app/gupi#readme")
    });
    cx.on_action(|_: &PiDocs, cx| {
        cx.open_url("https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent#readme")
    });
    cx.on_action(|_: &ReportIssue, cx| {
        cx.open_url("https://github.com/suxiaoshao/gpui/issues/new/choose")
    });
    cx.on_action(|_: &ShowLogs, cx| show_logs(cx));
    cx.on_action(|_: &CopyDiagnostics, cx| copy_diagnostics(cx));
    cx.on_action(|_: &About, cx| {
        with_window(cx, |window, cx| {
            if window.has_active_dialog(cx) {
                return;
            }
            window.open_dialog(cx, |dialog, _, cx| {
                dialog.title(t(cx, "menu-about")).child(
                    v_flex()
                        .gap_3()
                        .child(format!("Gupi {}", env!("CARGO_PKG_VERSION")))
                        .child(
                            Button::new("copy-diagnostics")
                                .label(t(cx, "menu-copy-diagnostics"))
                                .on_click(|_, _, cx| copy_diagnostics(cx)),
                        )
                        .child(t(cx, "settings-diagnostics-help")),
                )
            });
        })
    });
}
fn with_window(cx: &mut App, action: impl FnOnce(&mut Window, &mut App) + 'static) {
    if let Some(window) = cx.active_window() {
        // Menu actions can arrive while that window is already leased for
        // dispatch. Keep the target but update it after dispatch has completed.
        cx.defer(move |cx| {
            if let Err(error) = window.update(cx, |_, window, cx| action(window, cx)) {
                tracing::warn!(%error, "apply native window action failed");
            }
        });
    }
}
pub(crate) fn show_logs(cx: &App) {
    match paths::log_dir() {
        Ok(path) => cx.reveal_path(&path),
        Err(error) => tracing::warn!(%error, "resolve log directory failed"),
    }
}
pub(crate) fn copy_diagnostics(cx: &mut App) {
    let command = cx
        .try_global::<super::MainWindow>()
        .map(|main| {
            main.view
                .read(cx)
                .config
                .read(cx)
                .preferences(cx)
                .pi_executable()
        })
        .unwrap_or_else(|| "pi".into());
    let directory = |result: std::io::Result<std::path::PathBuf>| {
        result
            .map(|path| path.display().to_string())
            .unwrap_or_else(|error| error.to_string())
    };
    // Deliberately diagnostic field names; no environment dump or conversation content.
    let text = format!(
        "Gupi: {}\nPlatform: {} / {}\nPi command: {}\nConfig: {}\nLogs: {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        command.display(),
        directory(paths::config_dir()),
        directory(paths::log_dir()),
    );
    cx.write_to_clipboard(ClipboardItem::new_string(text));
}
pub(crate) fn refresh(cx: &mut App) {
    let locale = crate::foundation::i18n::locale(cx);
    if cx
        .try_global::<AppliedLocale>()
        .is_some_and(|old| old.0 == locale)
    {
        return;
    }
    cx.set_global(AppliedLocale(locale));
    super::notifications::refresh_labels(cx);
    refresh_native(cx);
}

fn app_menus(cx: &App) -> Vec<Menu> {
    let command = |label, kind| {
        let enabled = CONVERSATION_COMMANDS
            .iter()
            .position(|candidate| *candidate == kind)
            .and_then(|index| {
                cx.try_global::<ConversationCommands>()
                    .map(|state| state.0[index])
            })
            .unwrap_or(false);
        MenuItem::action(t(cx, label), Run(kind)).disabled(!enabled)
    };
    let mut app_items = vec![
        MenuItem::action(t(cx, "menu-about"), About),
        MenuItem::separator(),
        MenuItem::action(t(cx, "menu-settings"), ShowSettings),
    ];
    #[cfg(target_os = "macos")]
    app_items.extend([
        MenuItem::separator(),
        MenuItem::os_submenu(t(cx, "menu-services"), SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action(t(cx, "menu-hide"), Hide),
        MenuItem::action(t(cx, "menu-hide-others"), HideOthers),
        MenuItem::action(t(cx, "menu-show-all"), ShowAll),
    ]);
    app_items.extend([
        MenuItem::separator(),
        MenuItem::action(t(cx, "menu-quit"), Quit),
    ]);
    vec![
        Menu::new("Gupi").items(app_items),
        Menu::new(t(cx, "menu-conversation")).items([
            command("conversation-new", Kind::New),
            command("conversation-search", Kind::QuickOpen),
            MenuItem::separator(),
            command("conversation-rename", Kind::Rename),
            command("conversation-export", Kind::Export),
        ]),
        Menu::new(t(cx, "menu-edit")).items([
            MenuItem::os_action(t(cx, "menu-undo"), input::Undo, OsAction::Undo),
            MenuItem::os_action(t(cx, "menu-redo"), input::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(t(cx, "menu-cut"), input::Cut, OsAction::Cut),
            MenuItem::os_action(t(cx, "menu-copy"), input::Copy, OsAction::Copy),
            MenuItem::os_action(t(cx, "menu-paste"), input::Paste, OsAction::Paste),
            MenuItem::os_action(
                t(cx, "menu-select-all"),
                input::SelectAll,
                OsAction::SelectAll,
            ),
        ]),
        Menu::new(t(cx, "menu-view")).items([
            MenuItem::action(t(cx, "command-palette"), ShowCommandPalette),
            command("conversation-sidebar", Kind::Sidebar),
            command("conversation-history", Kind::History),
        ]),
        Menu::new(t(cx, "menu-window")).items([
            MenuItem::action(t(cx, "menu-show-main"), ShowMainWindow),
            MenuItem::action(t(cx, "temporary-title"), ShowTemporaryWindow),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu-minimize"), Minimize),
            MenuItem::action(t(cx, "menu-zoom"), Zoom),
            MenuItem::action(t(cx, "menu-fullscreen"), Fullscreen),
        ]),
        Menu::new(t(cx, "menu-help")).items([
            MenuItem::action(t(cx, "menu-docs"), UserGuide),
            MenuItem::action(t(cx, "menu-pi-docs"), PiDocs),
            MenuItem::action(t(cx, "menu-report-issue"), ReportIssue),
            MenuItem::separator(),
            MenuItem::action(t(cx, "menu-logs"), ShowLogs),
            MenuItem::action(t(cx, "menu-copy-diagnostics"), CopyDiagnostics),
        ]),
    ]
}

/// Native menus capture the keymap. Rebuild on binding or language changes.
pub(crate) fn refresh_native(cx: &mut App) {
    cx.set_menus(app_menus(cx));
    let menus = app_menus(cx).into_iter().map(Menu::owned).collect();
    GlobalState::global_mut(cx).set_app_menus(menus);
    super::tray::refresh(cx);
    cx.set_global(MenusChanged);
    #[cfg(target_os = "macos")]
    if let Err(error) = platform_ext::app::set_windows_menu_from_main_menu_index(4) {
        tracing::warn!(%error, "register Gupi window menu failed");
    }
}

#[cfg(test)]
mod tests {
    use super::with_window;
    use gpui_kit::{Context, IntoElement, Render, TestAppContext, Window, div};
    use std::{cell::Cell, rc::Rc};

    struct Page;
    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui_kit::test]
    fn native_window_action_runs_after_the_dispatch_window_is_released(cx: &mut TestAppContext) {
        let (_, visual) = cx.add_window_view(|window, _| {
            window.activate_window();
            Page
        });
        visual.run_until_parked();
        let called = Rc::new(Cell::new(false));
        let result = called.clone();
        // Global menu callbacks run with the dispatching window already leased.
        visual.update(|_, cx| with_window(cx, move |_, _| result.set(true)));
        visual.run_until_parked();
        assert!(called.get());
    }
}
