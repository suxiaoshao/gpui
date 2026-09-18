//! The window is a view of application-owned, non-persistent conversations.
use crate::{
    features::temporary::startup::TemporaryStartup, state::conversation::ConversationState,
};
use gpui_kit::{component::Root, *};
use std::{path::PathBuf, time::Duration};
use window_ext::{WindowExt as _, WindowLevel};

const WINDOW_SIZE: Size<Pixels> = size(px(960.), px(620.));
const RECYCLE_DELAY: Duration = Duration::from_secs(600);

pub(crate) struct Temporary {
    pub command: Option<PathBuf>,
    pub state: Option<Entity<ConversationState>>,
    pub environment: Option<(
        Entity<crate::state::config::ConfigController>,
        Entity<crate::pi::PiProbeController>,
    )>,
    window: Option<WindowHandle<Root>>,
    pub draining: bool,
    pub cleanup: Option<Task<()>>,
    pub cleanup_result: Option<Result<usize, String>>,
    front: Option<platform_ext::app::FrontmostApp>,
    recycle: Option<Task<()>>,
    paste: Option<Task<()>>,
    visibility_epoch: u64,
    visible: bool,
}
impl Global for Temporary {}

pub fn init(cx: &mut App) {
    crate::features::temporary::init(cx);
    cx.set_global(Temporary {
        command: None,
        state: None,
        environment: None,
        window: None,
        draining: false,
        cleanup: None,
        cleanup_result: None,
        front: None,
        recycle: None,
        paste: None,
        visibility_epoch: 0,
        visible: false,
    });
}
pub fn set_command(command: PathBuf, cx: &mut App) {
    let state = cx.global::<Temporary>().state.clone();
    if let Some(state) = state {
        state.update(cx, |s, _| s.set_command(command.clone()));
    }
    cx.global_mut::<Temporary>().command = Some(command);
}
pub fn state(cx: &mut App) -> Option<Entity<ConversationState>> {
    if cx.global::<Temporary>().draining {
        return None;
    }
    if let Some(state) = &cx.global::<Temporary>().state {
        return Some(state.clone());
    }
    let command = cx.global::<Temporary>().command.clone()?;
    let state = cx.new(|cx| ConversationState::temporary(command, cx));
    cx.global_mut::<Temporary>().state = Some(state.clone());
    Some(state)
}
pub fn toggle(cx: &mut App) {
    if cx.global::<Temporary>().visible
        && let Some(handle) = cx.global::<Temporary>().window
        && handle.update(cx, |_, window, cx| hide(window, cx)).is_ok()
    {
        return;
    }
    remember_frontmost(cx);
    show(cx);
}
pub fn remember_frontmost(cx: &mut App) {
    if cx.global::<Temporary>().front.is_none() {
        cx.global_mut::<Temporary>().front = platform_ext::app::capture_frontmost();
    }
}
fn paste_failure(window: &mut Window, cx: &mut App) {
    use gpui_kit::component::{
        WindowExt as _,
        notification::{Notification, NotificationType},
    };
    window.push_notification(
        Notification::new()
            .message(crate::foundation::i18n::t(cx, "temporary-paste-failed"))
            .with_type(NotificationType::Warning),
        cx,
    );
}
pub(crate) fn paste_answer(text: String, window: &mut Window, cx: &mut App) {
    let owner = cx.global::<Temporary>();
    if owner.paste.is_some() || !owner.visible {
        return;
    }
    let available = owner.front.is_some() && platform_ext::app::check_paste_access().is_ok();
    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
    if !available {
        paste_failure(window, cx);
        return;
    }
    hide_impl(window, true, Some(text), cx);
}
pub fn hide(window: &mut Window, cx: &mut App) {
    hide_impl(window, true, None, cx);
}
pub(crate) fn on_deactivate(window: &mut Window, cx: &mut App) {
    // The user has already chosen another app. Do not steal focus back from it.
    if !window.is_window_active() {
        hide_impl(window, false, None, cx);
    }
}
fn hide_impl(window: &mut Window, restore_front: bool, paste: Option<String>, cx: &mut App) {
    let owner = cx.global_mut::<Temporary>();
    if !owner.visible {
        return;
    }
    owner.visible = false;
    owner.visibility_epoch += 1;
    let epoch = owner.visibility_epoch;
    let front = owner.front.take().filter(|_| restore_front);
    let handle = owner.window;
    let native = window.native_window_handle().ok();
    // Native show/hide may synchronously re-enter AppKit/GPUI. Never hold an App
    // borrow across them, and discard operations superseded by a later summon.
    let pasting = paste.is_some();
    let task = cx.spawn(async move |cx| {
        let result = async {
            if !cx.update(|cx| cx.global::<Temporary>().visibility_epoch == epoch) {
                return Ok(());
            }
            let native = native.ok_or("Window handle unavailable")?;
            native.hide().map_err(|_| "Cannot hide temporary window")?;
            if let Some(front) = front {
                front.restore();
                if let Some(text) = paste {
                    // App activation is asynchronous. Recheck both ownership and
                    // clipboard contents before injecting a single paste.
                    for _ in 0..25 {
                        cx.background_executor()
                            .timer(Duration::from_millis(20))
                            .await;
                        if !cx.update(|cx| cx.global::<Temporary>().visibility_epoch == epoch) {
                            return Ok(());
                        }
                        if front.is_frontmost() {
                            let unchanged = cx.update(|cx| {
                                cx.read_from_clipboard()
                                    .and_then(|item| item.text())
                                    .as_deref()
                                    == Some(text.as_str())
                            });
                            if !unchanged {
                                return Err("Clipboard changed before paste");
                            }
                            return front.paste();
                        }
                    }
                    return Err("The original application could not be focused");
                }
            }
            Ok(())
        }
        .await;
        cx.update(|cx| {
            if pasting {
                cx.global_mut::<Temporary>().paste = None;
            }
            if let Err(error) = result {
                tracing::warn!(error, "temporary return failed");
                if pasting && let Some(handle) = handle {
                    let _ = handle.update(cx, |_, window, cx| paste_failure(window, cx));
                }
                if pasting && cx.global::<Temporary>().visibility_epoch == epoch {
                    show(cx);
                }
            }
        });
    });
    if pasting {
        cx.global_mut::<Temporary>().paste = Some(task);
    } else {
        task.detach();
    }
    let timer = cx.background_executor().timer(RECYCLE_DELAY);
    let recycle = cx.spawn(async move |cx| {
        timer.await;
        cx.update(|cx| {
            let owner = cx.global_mut::<Temporary>();
            if owner.visible || owner.visibility_epoch != epoch || owner.window != handle {
                return;
            }
            owner.window = None;
            // Only release the view. Global session entities keep consuming Pi events.
            if let Some(handle) = handle {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        });
    });
    cx.global_mut::<Temporary>().recycle = Some(recycle);
}
pub fn show(cx: &mut App) {
    if cx.global::<Temporary>().draining {
        cx.global_mut::<Temporary>().front = None;
        return;
    }
    let display = target_display_id(cx);
    cx.global_mut::<Temporary>().recycle = None;
    let existing = cx
        .global::<Temporary>()
        .window
        .filter(|handle| handle.read(cx).is_ok());
    let handle = match existing {
        Some(handle) => handle,
        None => {
            let result = cx.open_window(window_options(display, cx), |window, cx| {
                if let Err(error) = window.set_window_level(WindowLevel::ModalPanel) {
                    tracing::warn!(%error, "set temporary window level failed");
                }
                window.on_window_should_close(cx, |window, cx| {
                    hide(window, cx);
                    false
                });
                let view = cx.new(|cx| TemporaryStartup::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            });
            match result {
                Ok(handle) => handle,
                Err(error) => {
                    tracing::error!(%error, "open temporary window failed");
                    cx.global_mut::<Temporary>().front = None;
                    return;
                }
            }
        }
    };
    cx.global_mut::<Temporary>().window = Some(handle);
    let reveal = handle.update(cx, |root, window, cx| {
        if let Ok(view) = root.view().clone().downcast::<TemporaryStartup>() {
            view.update(cx, |view, cx| view.focus_search(window, cx));
        }
        let size = display
            .and_then(|id| cx.find_display(id))
            .map(|d| fitting_size(window.bounds().size, d.bounds().size))
            .unwrap_or(WINDOW_SIZE);
        (
            window.native_window_handle(),
            Bounds::centered(display, size, cx),
        )
    });
    let Ok((Ok(native), bounds)) = reveal else {
        cx.global_mut::<Temporary>().front = None;
        return;
    };
    let owner = cx.global_mut::<Temporary>();
    owner.visibility_epoch += 1;
    owner.visible = true;
    let epoch = owner.visibility_epoch;
    cx.spawn(async move |cx| {
        if !cx.update(|cx| cx.global::<Temporary>().visibility_epoch == epoch) {
            return;
        }
        if let Err(error) = native.set_window_level(WindowLevel::ModalPanel) {
            tracing::warn!(%error, "set temporary window level failed");
        }
        if let Err(error) = native.move_and_resize(bounds, display) {
            tracing::warn!(%error, "position temporary window failed");
        }
        if let Err(error) = native.show() {
            tracing::warn!(%error, "show temporary window failed");
            let front = cx.update(|cx| {
                let owner = cx.global_mut::<Temporary>();
                owner.visible = false;
                owner.front.take()
            });
            if let Some(front) = front {
                front.restore();
            }
        }
    })
    .detach();
}
fn window_options(display: Option<DisplayId>, cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            display,
            WINDOW_SIZE,
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(point(px(-100.), px(-100.))),
        }),
        window_background: WindowBackgroundAppearance::Opaque,
        is_resizable: false,
        kind: WindowKind::PopUp,
        show: false,
        focus: false,
        display_id: display,
        app_id: Some("Gupi".into()),
        ..Default::default()
    }
}
fn target_display_id(cx: &App) -> Option<DisplayId> {
    let displays = cx.displays();
    if let Some(id) = platform_ext::app::current_mouse_display_id()
        && let Some(display) = displays.iter().find(|d| u64::from(d.id()) == id)
    {
        return Some(display.id());
    }
    // Windows cursor coordinates are physical; GPUI display bounds are logical.
    if !cfg!(target_os = "windows")
        && let Some((x, y)) = platform_ext::app::current_mouse_location()
        && let Some(display) = displays
            .iter()
            .find(|d| d.bounds().contains(&point(px(x), px(y))))
    {
        return Some(display.id());
    }
    cx.primary_display()
        .or_else(|| displays.into_iter().next())
        .map(|d| d.id())
}
fn fitting_size(current: Size<Pixels>, display: Size<Pixels>) -> Size<Pixels> {
    if current.width <= px(0.)
        || current.height <= px(0.)
        || current.width > display.width
        || current.height > display.height
    {
        WINDOW_SIZE
    } else {
        current
    }
}
pub fn drain(cx: &mut App) -> Option<Task<()>> {
    let owner = cx.global_mut::<Temporary>();
    owner.draining = true;
    owner.recycle = None;
    owner.visibility_epoch += 1;
    let cleanup = owner.cleanup.take();
    let state = owner.state.clone();
    let flush = state.map(|state| state.update(cx, |s, cx| s.flush(cx)));
    if cleanup.is_none() && flush.is_none() {
        return None;
    }
    Some(cx.spawn(async move |_| {
        if let Some(cleanup) = cleanup {
            cleanup.await;
        }
        if let Some(flush) = flush {
            flush.await;
        }
    }))
}

pub fn clean_released(cx: &mut App) {
    if cx.global::<Temporary>().cleanup.is_some() || cx.global::<Temporary>().draining {
        return;
    }
    let root = match crate::foundation::paths::temporary_dir() {
        Ok(root) => root,
        Err(e) => {
            cx.global_mut::<Temporary>().cleanup_result = Some(Err(e.to_string()));
            return;
        }
    };
    let task = cx.spawn(async move |cx| {
        let candidates = smol::unblock(move || -> Result<Vec<PathBuf>, String> {
            if !root.exists() {
                return Ok(vec![]);
            }
            if std::fs::symlink_metadata(&root)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                return Err("Temporary root must not be a symlink".into());
            }
            std::fs::read_dir(root)
                .map_err(|e| e.to_string())?
                .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
                .collect()
        })
        .await;
        let result = match candidates {
            Err(e) => Err(e),
            Ok(mut candidates) => {
                // Capture protection after enumeration: newly created conversations
                // cannot appear in the candidate list without being protected here.
                let protected = cx.update(|cx| {
                    cx.global::<Temporary>()
                        .state
                        .as_ref()
                        .map(|state| {
                            state
                                .read(cx)
                                .sessions
                                .values()
                                .map(|s| s.info.cwd.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                });
                candidates.retain(|p| {
                    !protected.contains(p)
                        && p.file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with("draft-"))
                });
                smol::unblock(move || {
                    let mut count = 0;
                    for path in candidates {
                        crate::state::conversation::temporary::trash_workspace(&path)?;
                        count += 1;
                    }
                    Ok(count)
                })
                .await
            }
        };
        cx.update(|cx| {
            let owner = cx.global_mut::<Temporary>();
            owner.cleanup_result = Some(result);
            owner.cleanup = None;
        });
    });
    cx.global_mut::<Temporary>().cleanup = Some(task);
}

#[cfg(test)]
pub(crate) fn make_visible_for_test(cx: &mut App) {
    cx.global_mut::<Temporary>().visible = true;
}

#[cfg(test)]
mod tests {
    use super::{RECYCLE_DELAY, Temporary, WINDOW_SIZE, fitting_size, hide, init};
    use crate::state::conversation::ConversationState;
    use gpui_kit::component::Root;
    use gpui_kit::{
        AppContext, Context, Entity, IntoElement, Render, TestAppContext, Window, WindowHandle,
        WindowOptions, div, px, size,
    };
    use std::time::Duration;
    struct EmptyView;
    impl Render for EmptyView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    fn setup(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<ConversationState>) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::state::pi::init(cx);
            init(cx);
            let state = cx.new(|cx| ConversationState::temporary("unused-pi".into(), cx));
            state.update(cx, |s, _| s.selected = Some("retained-selection".into()));
            let window = cx
                .open_window(WindowOptions::default(), |window, cx| {
                    let view = cx.new(|_| EmptyView);
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .unwrap();
            let owner = cx.global_mut::<Temporary>();
            owner.state = Some(state.clone());
            owner.window = Some(window);
            owner.visible = true;
            (window, state)
        })
    }
    #[gpui_kit::test]
    fn popup_opens_before_pi_is_ready(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            app_theme::init(cx);
            crate::state::theme::init(cx);
            crate::foundation::i18n::apply(Default::default(), cx);
            crate::state::pi::init(cx);
            init(cx);
            super::show(cx);
            let owner = cx.global::<Temporary>();
            assert!(owner.command.is_none());
            assert!(owner.state.is_none());
            let window = owner
                .window
                .expect("Pi readiness must not prevent opening the popup");
            assert!(window.read(cx).is_ok());
        });
    }
    #[gpui_kit::test]
    fn hidden_window_expires_without_releasing_conversation_state(cx: &mut TestAppContext) {
        let (window, state) = setup(cx);
        cx.update(|cx| window.update(cx, |_, window, cx| hide(window, cx)).unwrap());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(599));
        cx.run_until_parked();
        cx.update(|cx| assert!(window.read(cx).is_ok()));
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(window.read(cx).is_err());
            assert!(cx.global::<Temporary>().window.is_none());
            assert_eq!(cx.global::<Temporary>().state.as_ref(), Some(&state));
            assert_eq!(
                state.read(cx).selected.as_deref(),
                Some("retained-selection")
            );
            assert!(!state.read(cx).draining());
        });
    }
    #[gpui_kit::test]
    fn showing_again_cancels_hidden_window_recycling(cx: &mut TestAppContext) {
        let (window, _) = setup(cx);
        cx.update(|cx| window.update(cx, |_, window, cx| hide(window, cx)).unwrap());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(599));
        cx.run_until_parked();
        cx.update(super::show);
        cx.executor().advance_clock(RECYCLE_DELAY);
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(window.read(cx).is_ok());
            assert!(cx.global::<Temporary>().recycle.is_none());
        });
    }
    #[gpui_kit::test]
    fn replacement_window_is_not_closed_by_an_old_timer(cx: &mut TestAppContext) {
        let (old, _) = setup(cx);
        cx.update(|cx| old.update(cx, |_, window, cx| hide(window, cx)).unwrap());
        cx.run_until_parked();
        let newer = cx.update(|cx| {
            let newer = cx
                .open_window(WindowOptions::default(), |window, cx| {
                    let view = cx.new(|_| EmptyView);
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .unwrap();
            cx.global_mut::<Temporary>().window = Some(newer);
            newer
        });
        cx.executor().advance_clock(RECYCLE_DELAY);
        cx.run_until_parked();
        cx.update(|cx| assert!(newer.read(cx).is_ok()));
    }
    #[test]
    fn recenter_preserves_only_sizes_that_fit_the_target_screen() {
        assert_eq!(
            fitting_size(WINDOW_SIZE, size(px(1920.), px(1080.))),
            WINDOW_SIZE
        );
        assert_eq!(
            fitting_size(size(px(2000.), px(1200.)), size(px(1920.), px(1080.))),
            WINDOW_SIZE
        );
        assert_eq!(
            fitting_size(size(px(0.), px(0.)), size(px(1920.), px(1080.))),
            WINDOW_SIZE
        );
    }
}
