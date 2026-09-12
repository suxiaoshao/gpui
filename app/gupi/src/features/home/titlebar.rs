use super::*;
use crate::features::chrome;
use gpui_kit::base::animation::{EffectTransition, ease_in_out_cubic};
use gpui_kit::component::{Icon, menu::DropdownMenu, separator::Separator, tooltip::Tooltip};
use gpui_kit::prelude::FluentBuilder as _;
use std::time::Duration;

impl HomeView {
    pub(super) fn render_titlebar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let state = self.state.read(cx);
        let current = state.current();
        let title = current
            .map(|session| session.info.title())
            .filter(|title| !title.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| t(cx, "conversation-new"));
        window.set_window_title(&format!("{title} — Gupi"));
        let key = state.selected.clone();
        let has_actions = current.is_some_and(|session| {
            !session.info.path.as_os_str().is_empty() || session.instance.is_some()
        });
        let loading = current.is_some_and(|session| session.core_read.running());
        let can_refresh = current.is_some_and(|session| {
            !session.info.path.as_os_str().is_empty()
                && !loading
                && !session.command.running()
                && !session.model_change.running()
        });
        let leading = chrome::leading_space(window);
        let target_width = if self.show_sidebar {
            px(self.pane_layout.left)
        } else {
            leading + px(72.)
        };
        let widths = window.use_keyed_state("titlebar-sidebar-width", cx, |_, _| {
            (target_width, target_width)
        });
        if widths.read(cx).1 != target_width {
            widths.update(cx, |widths, _| *widths = (widths.1, target_width));
        }
        let (from_width, to_width) = *widths.read(cx);
        let sidebar_label = t(
            cx,
            if self.show_sidebar {
                "conversation-hide-sidebar"
            } else {
                "conversation-show-sidebar"
            },
        );
        let left = h_flex()
            .h_full()
            .flex_none()
            .pl(leading)
            .pr_3()
            .gap_1()
            .when(self.show_sidebar, |view| view.bg(cx.theme().sidebar))
            .when(!self.show_sidebar, |view| {
                view.pr_0().border_b_1().border_color(cx.theme().border)
            })
            .child(chrome::control(
                "sidebar-control",
                chrome::button("toggle-sidebar")
                    .icon(if self.show_sidebar {
                        gpui_kit::component::IconName::PanelLeftClose
                    } else {
                        gpui_kit::component::IconName::PanelLeftOpen
                    })
                    .tooltip(sidebar_label.clone())
                    .accessibility_label(sidebar_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_sidebar = !this.show_sidebar;
                        cx.notify();
                    })),
            ))
            .when(!self.show_sidebar, |view| {
                view.child(chrome::control(
                    "new-conversation-control",
                    chrome::button("titlebar-new-conversation")
                        .icon(IconName::SquarePen)
                        .tooltip(t(cx, "conversation-new"))
                        .accessibility_label(t(cx, "conversation-new"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.new_conversation(window, cx);
                        })),
                ))
                .child(Separator::vertical().relative().w(px(1.)).h_5().ml_1())
            });
        let left = EffectTransition::new(Duration::from_millis(200))
            .ease(ease_in_out_cubic)
            .width(from_width, to_width)
            .apply(
                left,
                ElementId::NamedInteger(
                    "titlebar-sidebar-width".into(),
                    (from_width.as_f32().to_bits() as u64) << 32
                        | to_width.as_f32().to_bits() as u64,
                ),
            );
        let mut main = h_flex()
            .h_full()
            .flex_1()
            .min_w_0()
            .px_3()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            // Match the resize divider below: both start at the content edge.
            .when(self.show_sidebar, |view| view.border_l_1())
            .child(Icon::new(IconName::Folder).small())
            .child(
                div()
                    .id("session-title")
                    .min_w_0()
                    .text_ellipsis()
                    .child(title.clone())
                    .tooltip(move |window, cx| Tooltip::new(title.clone()).build(window, cx)),
            );
        if let Some(key) = key.clone().filter(|_| has_actions) {
            let state = self.state.clone();
            let owner = cx.weak_entity();
            main = main.child(chrome::control(
                "session-menu-control",
                chrome::button("session-menu")
                    .icon(IconName::Ellipsis)
                    .tooltip(t(cx, "conversation-actions"))
                    .accessibility_label(t(cx, "conversation-actions"))
                    .dropdown_menu(move |menu, _, cx| {
                        navigation::session_menu(
                            menu,
                            key.clone(),
                            state.clone(),
                            owner.clone(),
                            cx,
                        )
                    }),
            ));
        }
        main = main
            .child(div().flex_1())
            .child(chrome::control(
                "refresh-control",
                chrome::button("refresh-session")
                    .icon(IconName::RefreshCw)
                    .loading(loading)
                    .disabled(!can_refresh)
                    .tooltip(t(cx, "conversation-refresh-current"))
                    .accessibility_label(t(cx, "conversation-refresh-current"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(key) = &key {
                            this.state.update(cx, |state, cx| {
                                state.connect(key, cx);
                                state.refresh(key, cx);
                            });
                        }
                    })),
            ))
            .child(chrome::control(
                "history-control",
                chrome::button("toggle-history")
                    .icon(IconName::PanelRight)
                    .tooltip(t(cx, "conversation-history"))
                    .accessibility_label(t(cx, "conversation-history"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_history = !this.show_history;
                        this.sync(true, window, cx);
                        if !this.show_history
                            && let Some(view) =
                                this.shown_key.as_ref().and_then(|key| this.views.get(key))
                        {
                            view.history_canvas
                                .update(cx, |canvas, cx| canvas.clear_pointer(cx));
                        }
                        if this.show_history && this.history_view != HistoryView::Tree {
                            this.history_list.update(cx, |list, cx| {
                                if let Some(last) = list.delegate().rows.len().checked_sub(1) {
                                    list.scroll_to_item(
                                        gpui_kit::component::IndexPath::new(last),
                                        ScrollStrategy::Bottom,
                                        window,
                                        cx,
                                    );
                                }
                            });
                        }
                    })),
            ));
        chrome::title_bar(cx).child(h_flex().size_full().min_w_0().child(left).child(main))
    }
}
