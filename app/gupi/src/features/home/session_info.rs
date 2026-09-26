//! Read-only information cached for the selected or context-menu conversation.
use super::*;
use crate::foundation::{i18n::t_with_args, session_catalog::SessionInfo};
use fluent_bundle::FluentArgs;
use gpui_kit::component::{
    StyledExt,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};
use std::path::{Path, PathBuf};

struct SessionInfoView {
    state: Entity<ConversationState>,
    key: String,
    scroll: ScrollHandle,
    _subscription: Subscription,
}

type Property = (
    &'static str,
    String,
    &'static str,
    Option<String>,
    Option<PathBuf>,
);

struct CachedSession {
    temporary: bool,
    loaded: bool,
    info: Option<SessionInfo>,
    state: Option<pi_rpc::protocol::SessionState>,
    stats: Option<pi_rpc::protocol::SessionStats>,
    stats_loading: bool,
}

impl CachedSession {
    fn read(state: &ConversationState, key: &str) -> Self {
        let session = state.sessions.get(key);
        let info = session.map(|session| session.info.clone()).or_else(|| {
            state
                .catalog
                .data()
                .and_then(|catalog| catalog.sessions.iter().find(|info| info.key() == key))
                .cloned()
        });
        Self {
            temporary: state.temporary,
            loaded: session.is_some(),
            info,
            state: session.and_then(|session| session.state.clone()),
            stats: session.and_then(|session| session.stats.data().cloned()),
            stats_loading: session.is_some_and(|session| session.stats.running()),
        }
    }
}

impl HomeView {
    pub(super) fn open_session_info(
        &mut self,
        key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) || self.has_image_preview(cx) {
            return;
        }
        let state = self.state.clone();
        let view = cx.new(|cx| SessionInfoView::new(state, key, window, cx));
        window.open_dialog(cx, move |dialog, window, cx| {
            dialog
                .title(t(cx, "conversation-session-info-title"))
                .width(
                    (window.rem_size() * 48.)
                        .min(window.viewport_size().width - window.rem_size() * 2.),
                )
                .on_ok(|_, _, _| false)
                .content({
                    let view = view.clone();
                    move |content, _, _| content.min_h_0().child(view.clone())
                })
        });
    }
}

impl SessionInfoView {
    fn new(
        state: Entity<ConversationState>,
        key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |this, _, event: &ConversationEvent, _, cx| {
                if let ConversationEvent::Changed(changes) = event
                    && (changes.catalog || changes.sessions.contains_key(&this.key))
                {
                    cx.notify();
                }
            },
        );
        Self {
            state,
            key,
            scroll: ScrollHandle::new(),
            _subscription: subscription,
        }
    }
}

fn path_text(path: &Path) -> Option<String> {
    (!path.as_os_str().is_empty()).then(|| path.to_string_lossy().into_owned())
}

fn session_file(
    stats: Option<&pi_rpc::protocol::SessionStats>,
    state: Option<&pi_rpc::protocol::SessionState>,
    info: Option<&SessionInfo>,
    temporary: bool,
) -> Option<PathBuf> {
    if temporary {
        return None;
    }
    if let Some(path) = info
        .map(|info| &info.path)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Some(path.clone());
    }
    stats
        .and_then(|stats| stats.session_file.as_deref())
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            state
                .and_then(|state| state.session_file.as_deref())
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from)
        })
        .filter(|path| path.is_file())
}

fn context_value(
    usage: Option<&pi_rpc::protocol::ContextUsage>,
    unknown: &str,
    cx: &App,
) -> String {
    let mut args = FluentArgs::new();
    args.set(
        "tokens",
        usage
            .and_then(|usage| usage.tokens)
            .map(|value| value.to_string())
            .unwrap_or_else(|| unknown.to_owned()),
    );
    args.set(
        "window",
        usage
            .filter(|usage| usage.context_window > 0)
            .map(|usage| usage.context_window.to_string())
            .unwrap_or_else(|| unknown.to_owned()),
    );
    args.set(
        "percent",
        usage
            .and_then(|usage| usage.percent)
            .filter(|percent| percent.is_finite())
            .map(|percent| format!("{percent:.1}%"))
            .unwrap_or_else(|| unknown.to_owned()),
    );
    t_with_args(cx, "conversation-session-info-context-value", &args)
}

fn property(
    label: String,
    value: String,
    id: &str,
    copy: Option<String>,
    reveal: Option<PathBuf>,
    window: &mut Window,
    cx: &mut Context<SessionInfoView>,
) -> AnyElement {
    let mut actions = h_flex().flex_none().gap_1();
    if let Some(value) = copy {
        actions = actions.child(super::messages::copy_button(
            format!("session-info-copy-{id}"),
            value,
            t(cx, "conversation-copy"),
            window,
            cx,
        ));
    }
    if let Some(path) = reveal {
        let label = t(cx, "action-locate");
        actions = actions.child(
            Button::new(format!("session-info-reveal-{id}"))
                .ghost()
                .xsmall()
                .icon(IconName::FolderOpen)
                .tooltip(label.clone())
                .accessibility_label(label)
                .on_click(move |_, _, cx| cx.reveal_path(&path)),
        );
    }
    h_flex()
        .w_full()
        .min_w_0()
        .items_start()
        .gap_3()
        .py_1()
        .child(
            div()
                .w_32()
                .flex_none()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(
            div()
                .id(format!("session-info-value-{id}"))
                .min_w_0()
                .flex_1()
                .text_sm()
                .whitespace_normal()
                .aria_label(value.clone())
                .child(value),
        )
        .child(actions)
        .into_any_element()
}

fn section(
    label: String,
    properties: Vec<Property>,
    window: &mut Window,
    cx: &mut Context<SessionInfoView>,
) -> AnyElement {
    let mut rows = Vec::with_capacity(properties.len());
    for (label_key, value, id, copy, reveal) in properties {
        rows.push(property(
            t(cx, label_key),
            value,
            id,
            copy,
            reveal,
            window,
            cx,
        ));
    }
    v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .child(div().font_semibold().child(label))
        .children(rows)
        .into_any_element()
}

fn stat_value(value: Option<u64>, unknown: &str, loading: bool, cx: &App) -> String {
    value.map(|value| value.to_string()).unwrap_or_else(|| {
        if loading {
            t(cx, "composer-stats-loading")
        } else {
            unknown.to_owned()
        }
    })
}

impl Render for SessionInfoView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cached = {
            let state = self.state.read(cx);
            CachedSession::read(state, &self.key)
        };
        let unknown = t(cx, "conversation-unknown");
        let not_saved = t(cx, "conversation-session-info-not-saved");
        let info = cached.info.as_ref();
        let pi_id = cached
            .state
            .as_ref()
            .map(|state| state.session_id.clone())
            .or_else(|| {
                cached
                    .stats
                    .as_ref()
                    .and_then(|stats| stats.session_id.clone())
            })
            .or_else(|| {
                info.filter(|info| !info.path.as_os_str().is_empty())
                    .map(|info| info.id.clone())
            });
        let id_value = if cached.temporary && pi_id.is_none() {
            not_saved.clone()
        } else {
            pi_id.clone().unwrap_or_else(|| unknown.clone())
        };
        let title = info
            .map(|info| super::navigation::display_title(info, cx))
            .unwrap_or_else(|| unknown.clone());
        let cwd = info.and_then(|info| path_text(&info.cwd));
        let cwd_reveal = info
            .map(|info| info.cwd.clone())
            .filter(|path| path.is_dir());
        let file = session_file(
            cached.stats.as_ref(),
            cached.state.as_ref(),
            info,
            cached.temporary,
        );
        let file_not_saved = cached.temporary
            || file.is_none()
                && cached.loaded
                && info.is_some_and(|info| info.path.as_os_str().is_empty());
        let file_value = if file_not_saved {
            not_saved.clone()
        } else {
            file.as_deref()
                .and_then(path_text)
                .unwrap_or_else(|| unknown.clone())
        };
        let file_copy = (!file_not_saved)
            .then(|| file.as_deref().and_then(path_text))
            .flatten();
        let file_reveal = file.filter(|path| path.is_file());
        let stats = cached.stats.as_ref();
        let loading = cached.stats_loading && stats.is_none();
        let usage = stats.map(|stats| &stats.tokens);
        let context = stats.and_then(|stats| stats.context_usage.as_ref());
        let context_value = context_value(context, &unknown, cx);
        let model = cached
            .state
            .as_ref()
            .and_then(|state| state.model.as_ref())
            .map(|model| format!("{} · {}", model.name, model.provider))
            .unwrap_or_else(|| unknown.clone());
        let thinking = cached
            .state
            .as_ref()
            .map(|state| state.thinking_level.trim())
            .filter(|level| !level.is_empty())
            .map(|level| super::pickers::thinking_label(level, cx))
            .unwrap_or_else(|| unknown.clone());

        let mut content = v_flex().min_w_0().gap_4().p_4();
        content = content.child(section(
            t(cx, "conversation-session-info-identity"),
            vec![
                ("conversation-session-info-name", title, "name", None, None),
                ("conversation-session-info-id", id_value, "id", pi_id, None),
                (
                    "conversation-session-info-directory",
                    cwd.clone().unwrap_or_else(|| unknown.clone()),
                    "directory",
                    cwd,
                    cwd_reveal,
                ),
                (
                    "conversation-session-info-file",
                    file_value,
                    "file",
                    file_copy,
                    file_reveal,
                ),
            ],
            window,
            cx,
        ));
        content = content.child(section(
            t(cx, "conversation-session-info-runtime"),
            vec![
                (
                    "conversation-session-info-model",
                    model,
                    "model",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-thinking",
                    thinking,
                    "thinking",
                    None,
                    None,
                ),
            ],
            window,
            cx,
        ));
        content = content.child(section(
            t(cx, "conversation-session-info-usage"),
            vec![
                (
                    "conversation-session-info-input",
                    usage
                        .map(|usage| usage.input.to_string())
                        .unwrap_or_else(|| stat_value(None, &unknown, loading, cx)),
                    "input",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-output",
                    usage
                        .map(|usage| usage.output.to_string())
                        .unwrap_or_else(|| stat_value(None, &unknown, loading, cx)),
                    "output",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-cache-read",
                    usage
                        .map(|usage| usage.cache_read.to_string())
                        .unwrap_or_else(|| stat_value(None, &unknown, loading, cx)),
                    "cache-read",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-cache-write",
                    usage
                        .map(|usage| usage.cache_write.to_string())
                        .unwrap_or_else(|| stat_value(None, &unknown, loading, cx)),
                    "cache-write",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-cost",
                    stats
                        .map(|stats| format!("${:.4}", stats.cost))
                        .unwrap_or_else(|| stat_value(None, &unknown, loading, cx)),
                    "cost",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-context",
                    context.map(|_| context_value).unwrap_or_else(|| {
                        if loading {
                            t(cx, "composer-stats-loading")
                        } else {
                            unknown.clone()
                        }
                    }),
                    "context",
                    None,
                    None,
                ),
            ],
            window,
            cx,
        ));
        content = content.child(section(
            t(cx, "conversation-session-info-counts"),
            vec![
                (
                    "conversation-session-info-user-messages",
                    stat_value(
                        stats.and_then(|stats| stats.user_messages),
                        &unknown,
                        loading,
                        cx,
                    ),
                    "user-messages",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-assistant-messages",
                    stat_value(
                        stats.and_then(|stats| stats.assistant_messages),
                        &unknown,
                        loading,
                        cx,
                    ),
                    "assistant-messages",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-tool-calls",
                    stat_value(
                        stats.and_then(|stats| stats.tool_calls),
                        &unknown,
                        loading,
                        cx,
                    ),
                    "tool-calls",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-tool-results",
                    stat_value(
                        stats.and_then(|stats| stats.tool_results),
                        &unknown,
                        loading,
                        cx,
                    ),
                    "tool-results",
                    None,
                    None,
                ),
                (
                    "conversation-session-info-total-messages",
                    stat_value(
                        stats.and_then(|stats| stats.total_messages),
                        &unknown,
                        loading,
                        cx,
                    ),
                    "total-messages",
                    None,
                    None,
                ),
            ],
            window,
            cx,
        ));
        content
            .id("conversation-session-info")
            .max_h((window.rem_size() * 32.).min(window.viewport_size().height * 0.65))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .vertical_scrollbar(&self.scroll)
    }
}
