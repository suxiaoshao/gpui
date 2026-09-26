use super::actions::{Kind, Run};
use super::*;
use crate::{
    app::menus,
    foundation::{
        i18n::t_with_args,
        session_catalog::{ScanProgress, SessionInfo},
    },
    state::conversation::Activity,
};
use fluent_bundle::FluentArgs;
use gpui_kit::component::{
    Collapsible as CollapsibleTrait, Icon, StyledExt,
    input::{Input, InputState},
    label::Label,
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
    progress::Progress,
    sidebar::{Sidebar, SidebarCollapsible, SidebarItem},
    spinner::Spinner,
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder as _;

type SessionRow = (String, SessionInfo, Activity, bool);

#[derive(Clone)]
enum NavigationItem {
    Project(Entity<ProjectView>),
    Loading { progress: Option<ScanProgress> },
    Failed,
    Empty,
}
impl CollapsibleTrait for NavigationItem {
    fn is_collapsed(&self) -> bool {
        false
    }
    fn collapsed(self, _: bool) -> Self {
        self
    }
}
impl SidebarItem for NavigationItem {
    fn render(
        self,
        id: impl Into<ElementId>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Project(project) => project.into_any_element(),
            Self::Loading { progress } => catalog_loading(progress, false, cx),
            Self::Failed => div()
                .px_2()
                .py_4()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(t(cx, "conversation-scan-failed"))
                .into_any_element(),
            Self::Empty => div()
                .id(id)
                .px_2()
                .py_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t(cx, "conversation-catalog-empty"))
                .into_any_element(),
        }
    }
}

fn catalog_loading(progress: Option<ScanProgress>, refresh: bool, cx: &App) -> AnyElement {
    let mut args = FluentArgs::new();
    let (label, percent) = match progress {
        None => (t(cx, "conversation-restoring"), None),
        Some(ScanProgress::Discovering { files }) => {
            args.set("count", files as i64);
            (t_with_args(cx, "conversation-discovering", &args), None)
        }
        Some(ScanProgress::Reading { completed, total }) => {
            args.set("completed", completed as i64);
            args.set("total", total as i64);
            (
                t_with_args(
                    cx,
                    if refresh {
                        "conversation-refresh-progress"
                    } else {
                        "conversation-read-progress"
                    },
                    &args,
                ),
                Some(if total == 0 {
                    100.
                } else {
                    completed as f32 / total as f32 * 100.
                }),
            )
        }
    };
    v_flex()
        .id(if refresh {
            "catalog-refresh"
        } else {
            "catalog-loading"
        })
        .w_full()
        .gap_2()
        .px_2()
        .when(!refresh, |row| row.h_24().justify_center())
        .when(refresh, |row| row.py_2())
        .role(Role::Status)
        .aria_label(label.clone())
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(
            h_flex()
                .gap_2()
                .min_w_0()
                .when(percent.is_none(), |row| row.child(Spinner::new().small()))
                .child(Label::new(label).truncate().min_w_0()),
        )
        .when_some(percent, |column, percent| {
            column.child(Progress::new("catalog-progress").value(percent).xsmall())
        })
        .into_any_element()
}

// Match Jaco's navigation rows: one full-width hit target, 28px height,
// sidebar theme tokens, and a shrinking label rather than a nested text button.
fn navigation_row(
    id: impl Into<ElementId>,
    label: String,
    icon: Option<IconName>,
    active: bool,
    cx: &App,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let action = Rc::new(action);
    let click = action.clone();
    let key = action.clone();
    h_flex()
        .id(id)
        .relative()
        .w_full()
        .min_w_0()
        .h_7()
        .p_2()
        .items_center()
        .gap_x_2()
        .overflow_hidden()
        .flex_shrink_0()
        .rounded(cx.theme().radius)
        .text_sm()
        .text_color(cx.theme().sidebar_foreground.opacity(0.7))
        .role(Role::Button)
        .aria_label(label.clone())
        .focusable()
        .tab_stop(true)
        .cursor_pointer()
        .when(active, |row| {
            row.font_medium()
                .bg(cx.theme().tokens.sidebar_accent.background)
                .text_color(cx.theme().sidebar_accent_foreground)
        })
        .hover(|row| {
            row.bg(cx.theme().tokens.sidebar_accent.background.opacity(0.8))
                .text_color(cx.theme().sidebar_accent_foreground)
        })
        .focus_visible(|row| {
            row.bg(cx.theme().tokens.sidebar_accent.background)
                .text_color(cx.theme().sidebar_accent_foreground)
        })
        .on_click(move |_, window, cx| click(window, cx))
        .on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                key(window, cx);
            }
        })
        .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
            action(window, cx)
        })
        .when_some(icon, |row, icon| {
            row.child(Icon::new(icon).size_4().flex_none())
        })
        .child(Label::new(label).text_sm().truncate().flex_1().min_w_0())
}
fn action_slot() -> Div {
    h_flex()
        .absolute()
        .top_0()
        .right_2()
        .bottom_0()
        .w_6()
        .items_center()
        .justify_end()
}

#[derive(Clone)]
struct ProjectItem {
    cwd: PathBuf,
    label: String,
    rows: Vec<SessionRow>,
    selected: Option<String>,
    closed: bool,
    more: bool,
    state: Entity<ConversationState>,
    owner: WeakEntity<HomeView>,
}
// The shell is always expanded; project expansion is independent of Sidebar's
// optional icon-only mode, which this app disables.
impl CollapsibleTrait for ProjectItem {
    fn is_collapsed(&self) -> bool {
        false
    }
    fn collapsed(self, _: bool) -> Self {
        self
    }
}
impl SidebarItem for ProjectItem {
    fn render(self, _: impl Into<ElementId>, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let project = self.cwd.clone();
        let owner = self.owner.clone();
        let group: SharedString = format!("project-row-{}", self.cwd.display()).into();
        let activity = if self.closed {
            self.rows
                .iter()
                .map(|(_, _, a, _)| *a)
                .max()
                .unwrap_or(Activity::Idle)
        } else {
            Activity::Idle
        };
        let unread = self.rows.iter().filter(|r| r.3).count();
        let context_path = project.clone();
        let menu_state = self.state.clone();
        let new_state = self.state.clone();
        let new_cwd = project.clone();
        let tooltip = project.to_string_lossy().into_owned();
        let header = navigation_row(
            format!("project-{}", project.display()),
            self.label,
            Some(if self.closed {
                IconName::Folder
            } else {
                IconName::FolderOpen
            }),
            false,
            cx,
            move |_, cx| {
                let _ = owner.update(cx, |this, cx| {
                    if !this.open_projects.remove(&project) {
                        this.open_projects.insert(project.clone());
                    }
                    this.sync_navigation_selection(cx);
                    cx.notify();
                });
            },
        )
        .group(group.clone())
        .pr_12()
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .when(activity != Activity::Idle || unread > 0, |row| {
            row.child(
                action_slot()
                    .w_10()
                    .group_hover(group.clone(), |style| style.opacity(0.))
                    .child(
                        h_flex()
                            .gap_1()
                            .children((unread > 0).then(|| {
                                div()
                                    .id("project-unread")
                                    .text_xs()
                                    .text_color(cx.theme().primary)
                                    .child(unread.to_string())
                                    .role(Role::Status)
                                    .aria_label(format!(
                                        "{}: {unread}",
                                        t(cx, "notification-unread")
                                    ))
                            }))
                            .child(activity_mark(activity, cx)),
                    ),
            )
        })
        .child(
            action_slot().child(
                Button::new(format!("project-new-{}", self.cwd.display()))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .opacity(0.)
                    .group_hover(group, |style| style.opacity(1.))
                    .focus_visible(|style| style.opacity(1.))
                    .tooltip(t(cx, "conversation-new"))
                    .accessibility_label(t(cx, "conversation-new"))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        new_state.update(cx, |s, cx| s.new_or_reuse(Some(new_cwd.clone()), cx));
                    }),
            ),
        )
        .context_menu(move |menu, _, cx| {
            let state = menu_state.clone();
            let cwd = context_path.clone();
            let reveal = cwd.clone();
            let copy = cwd.clone();
            menu.item(
                PopupMenuItem::new(t(cx, "conversation-new")).on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| s.new_or_reuse(Some(cwd.clone()), cx))
                }),
            )
            .separator()
            .item(
                PopupMenuItem::new(t(cx, "action-locate"))
                    .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
            )
            .item(
                PopupMenuItem::new(t(cx, "conversation-copy-path")).on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        copy.to_string_lossy().into_owned(),
                    ))
                }),
            )
        });
        let mut content = v_flex()
            .id(format!("project-group-{}", self.cwd.display()))
            .w_full()
            .min_w_0()
            .pb_1()
            .child(header);
        if !self.closed {
            let mut children = v_flex()
                .min_w_0()
                .border_l_1()
                .border_color(cx.theme().sidebar_border)
                .gap_1()
                .ml_3p5()
                .pl_2p5()
                .py_0p5();
            for (index, (key, info, activity, unread)) in self.rows.iter().enumerate() {
                if index >= 5 && !self.more && self.selected.as_ref() != Some(key) {
                    continue;
                }
                let title = display_title(info, cx);
                let tooltip = title.clone();
                let open_key = key.clone();
                let state = self.state.clone();
                let context_key = key.clone();
                let context_state = self.state.clone();
                let owner = self.owner.clone();
                let row = navigation_row(
                    format!("session-{key}"),
                    title,
                    None,
                    self.selected.as_ref() == Some(key),
                    cx,
                    move |_, cx| state.update(cx, |s, cx| s.open(&open_key, cx)),
                )
                .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                .when(*activity != Activity::Idle || *unread, |row| {
                    row.pr_10().child(
                        action_slot().w_8().child(
                            h_flex()
                                .gap_1()
                                .children(unread.then(|| unread_mark(cx)))
                                .child(activity_mark(*activity, cx)),
                        ),
                    )
                })
                .context_menu(move |menu, _, cx| {
                    session_menu(
                        menu,
                        context_key.clone(),
                        context_state.clone(),
                        owner.clone(),
                        cx,
                    )
                });
                children = children.child(row);
            }
            content = content.child(children);
            if self.rows.len() > 5 {
                let owner = self.owner.clone();
                let path = self.cwd.clone();
                content = content.child(
                    h_flex().ml_8().mt_1().h_7().child(
                        Button::new(format!("show-more-{}", path.display()))
                            .text()
                            .small()
                            .text_color(cx.theme().sidebar_foreground.opacity(0.5))
                            .label(t(
                                cx,
                                if self.more {
                                    "conversation-show-less"
                                } else {
                                    "conversation-show-more"
                                },
                            ))
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    if !this.projects_with_more.remove(&path) {
                                        this.projects_with_more.insert(path.clone());
                                    }
                                    this.sync_navigation_selection(cx);
                                    cx.notify();
                                });
                            }),
                    ),
                );
            }
        }
        content
    }
}
pub(crate) fn session_menu(
    menu: PopupMenu,
    key: String,
    state: Entity<ConversationState>,
    owner: WeakEntity<HomeView>,
    cx: &App,
) -> PopupMenu {
    if state.read(cx).temporary {
        let can_delete = state.read(cx).can_delete(&key);
        let reveal_state = state.clone();
        let reveal_key = key.clone();
        let info_owner = owner.clone();
        let info_key = key.clone();
        return menu
            .item(
                PopupMenuItem::new(t(cx, "conversation-session-info-command")).on_click(
                    move |_, window, cx| {
                        let _ = info_owner.update(cx, |this, cx| {
                            this.open_session_info(info_key.clone(), window, cx)
                        });
                    },
                ),
            )
            .separator()
            .item(
                PopupMenuItem::new(t(cx, "temporary-reveal-workspace")).on_click(
                    move |_, _, cx| {
                        if let Some(path) = reveal_state.read(cx).temporary_workspace(&reveal_key) {
                            cx.reveal_path(&path);
                        }
                    },
                ),
            )
            .item(
                PopupMenuItem::new(t(cx, "conversation-delete"))
                    .disabled(!can_delete)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| s.delete(&key, cx));
                    }),
            );
    }

    let current = state.read(cx).sessions.get(&key);
    let busy = current.is_some_and(|s| s.busy());
    let connected = current.is_some_and(|s| s.instance.is_some());
    let path = current
        .map(|s| s.info.path.clone())
        .or_else(|| {
            state
                .read(cx)
                .catalog
                .data()
                .into_iter()
                .flat_map(|catalog| &catalog.sessions)
                .find(|i| i.key() == key)
                .map(|i| i.path.clone())
        })
        .unwrap_or_default();
    let rename_key = key.clone();
    let reveal = path.clone();
    let copy = path.clone();
    let delete_key = key.clone();
    let delete_state = state.clone();
    let can_delete = state.read(cx).can_delete(&key);
    let can_rename = state.read(cx).can_rename(&key, cx);
    let can_clone = state.read(cx).can_clone(&key, cx);
    let clone_key = key.clone();
    let clone_state = state.clone();
    let info_owner = owner.clone();
    let info_key = key.clone();
    let mut menu = menu
        .item(
            PopupMenuItem::new(t(cx, "conversation-session-info-command")).on_click(
                move |_, window, cx| {
                    let _ = info_owner.update(cx, |this, cx| {
                        this.open_session_info(info_key.clone(), window, cx)
                    });
                },
            ),
        )
        .separator()
        .item(
            PopupMenuItem::new(t(cx, "conversation-rename"))
                .disabled(!can_rename || path.as_os_str().is_empty())
                .on_click(move |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.rename_dialog(rename_key.clone(), window, cx)
                    });
                }),
        )
        .item(
            PopupMenuItem::new(t(cx, "conversation-clone"))
                .disabled(!can_clone)
                .on_click(move |_, _, cx| {
                    clone_state.update(cx, |s, cx| s.clone_session(&clone_key, cx));
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new(t(cx, "action-locate"))
                .disabled(path.as_os_str().is_empty())
                .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
        )
        .item(
            PopupMenuItem::new(t(cx, "conversation-copy-path"))
                .disabled(path.as_os_str().is_empty())
                .on_click(move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        copy.to_string_lossy().into_owned(),
                    ))
                }),
        );
    if connected {
        menu = menu.separator().item(
            PopupMenuItem::new(t(
                cx,
                if busy {
                    "conversation-stop"
                } else {
                    "conversation-close-run"
                },
            ))
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if busy {
                        s.abort(&key, cx);
                    } else {
                        s.close(&key, cx);
                    }
                })
            }),
        );
    }
    menu.separator().item(
        PopupMenuItem::new(t(cx, "conversation-delete"))
            .disabled(!can_delete)
            .on_click(move |_, _, cx| {
                delete_state.update(cx, |s, cx| s.delete(&delete_key, cx));
            }),
    )
}
pub(crate) fn display_title(info: &SessionInfo, cx: &App) -> String {
    if info.title().is_empty() {
        t(cx, "conversation-untitled")
    } else {
        info.title().to_owned()
    }
}
pub(crate) fn activity_mark(activity: Activity, cx: &App) -> AnyElement {
    let (key, content) = match activity {
        Activity::Idle => return div().into_any_element(),
        Activity::Loading => (
            "conversation-loading",
            Spinner::new().small().into_any_element(),
        ),
        Activity::Running => (
            "conversation-running",
            Spinner::new().small().into_any_element(),
        ),
        Activity::Waiting => (
            "conversation-waiting",
            Icon::new(IconName::MessageCircle)
                .size_4()
                .text_color(cx.theme().warning)
                .into_any_element(),
        ),
        Activity::Failed => (
            "conversation-failed",
            Icon::new(IconName::CircleAlert)
                .size_4()
                .text_color(cx.theme().danger)
                .into_any_element(),
        ),
    };
    let label = t(cx, key);
    div()
        .id("activity")
        .role(Role::Status)
        .aria_label(label.clone())
        .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
        .child(content)
        .into_any_element()
}

impl HomeView {
    pub(super) fn new_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.new_or_reuse(None, cx);
        });
        self.input.update(cx, |input, cx| input.focus(window, cx));
    }

    pub(super) fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let groups = self
            .navigation
            .order
            .iter()
            .filter_map(|cwd| self.navigation.groups.get(cwd))
            .cloned()
            .collect::<Vec<_>>();
        let mut items = Vec::new();
        if state.scanning() && state.catalog.data().is_none() {
            items.push(NavigationItem::Loading {
                progress: state.catalog.progress(),
            });
        }
        if state.catalog.error().is_some() && state.catalog.data().is_none() {
            items.push(NavigationItem::Failed);
        }
        if state.catalog.data().is_some() || !groups.is_empty() {
            if groups.is_empty() && !state.scanning() {
                items.push(NavigationItem::Empty);
            }
            items.extend(groups.into_iter().map(NavigationItem::Project));
        }
        let owner = cx.entity().downgrade();
        let search_owner = owner.clone();
        let mut header = v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .child(div().flex_1().text_lg().child(t(cx, "app-title")))
                    .child(
                        Button::new("search-sessions")
                            .ghost()
                            .small()
                            .icon(IconName::Search)
                            .tooltip_with_action(
                                t(cx, "conversation-search"),
                                &Run(Kind::QuickOpen),
                                Some("Gupi"),
                            )
                            .accessibility_label(t(cx, "conversation-search"))
                            .on_click(move |_, window, cx| {
                                let _ = search_owner
                                    .update(cx, |this, cx| this.search_dialog(window, cx));
                            }),
                    ),
            )
            .child(
                navigation_row(
                    "new-conversation",
                    t(cx, "conversation-new"),
                    Some(IconName::Plus),
                    false,
                    cx,
                    move |window, cx| {
                        let _ = owner
                            .update(cx, |this, cx| this.run_action(&Run(Kind::New), window, cx));
                    },
                )
                .children(crate::features::command_palette::binding(Kind::New, window)),
            );
        if state.catalog.running() && state.catalog.data().is_some() {
            header = header.child(catalog_loading(state.catalog.progress(), true, cx));
        }
        let refresh = self.state.clone();
        let mut footer = v_flex()
            .w_full()
            .gap_1()
            .child(
                navigation_row(
                    "sidebar-settings",
                    t(cx, "menu-settings"),
                    Some(IconName::Settings),
                    false,
                    cx,
                    |window, cx| window.dispatch_action(Box::new(menus::ShowSettings), cx),
                )
                .children(crate::features::command_palette::binding(
                    Kind::Settings,
                    window,
                )),
            )
            .child(
                navigation_row(
                    "refresh-sessions",
                    t(cx, "conversation-refresh"),
                    Some(IconName::RotateCw),
                    false,
                    cx,
                    move |_, cx| refresh.update(cx, |s, cx| s.scan(cx)),
                )
                .children(crate::features::command_palette::binding(
                    Kind::Scan,
                    window,
                ))
                .when(state.scanning(), |row| {
                    row.opacity(0.5).cursor_default().tab_stop(false)
                }),
            );
        if state.catalog.error().is_some() {
            let owner = cx.entity().downgrade();
            footer = footer.child(navigation_row(
                "catalog-error",
                t(cx, "conversation-scan-failed"),
                Some(IconName::CircleAlert),
                false,
                cx,
                move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        let text = this
                            .state
                            .read(cx)
                            .catalog
                            .error()
                            .unwrap_or_default()
                            .to_owned();
                        window.open_dialog(cx, move |dialog, _, cx| {
                            dialog
                                .title(t(cx, "conversation-scan-failed"))
                                .child(div().text_sm().child(text.clone()))
                        });
                    });
                },
            ));
        }
        Sidebar::new("sessions-sidebar")
            // The collapse transition animates clip-width, which must not lag
            // behind the content width during a pointer-driven resize.
            .collapsible(if self.resizing_sidebar() {
                SidebarCollapsible::None
            } else {
                SidebarCollapsible::Offcanvas
            })
            .collapsed(!self.show_sidebar)
            .border_r_0()
            .header(header)
            .children(items)
            .footer(footer)
            .w(px(self.pane_layout.left))
            .h_full()
            .into_any_element()
    }
    fn search_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(true, window, cx);
    }
    pub(super) fn rename_dialog(
        &mut self,
        key: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = self
            .state
            .read(cx)
            .infos()
            .into_iter()
            .find(|(target, _)| target == &key)
            .map(|(_, info)| info.title().to_owned())
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(title, window, cx);
            s
        });
        let state = self.state.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let input2 = input.clone();
            let state = state.clone();
            let key = key.clone();
            dialog
                .title(t(cx, "conversation-rename"))
                .child(Input::new(&input))
                .on_ok(move |_, _, cx| {
                    let name = input2.read(cx).value().to_string();
                    if name.trim().is_empty() {
                        return false;
                    }
                    state.update(cx, |s, cx| s.rename(&key, name.trim().into(), cx))
                })
        });
    }
}

/// Retained presentation cache: only changed projects invalidate their view.
#[derive(Default)]
pub(super) struct Navigation {
    rows: BTreeMap<String, SessionRow>,
    groups: BTreeMap<PathBuf, Entity<ProjectView>>,
    order: Vec<PathBuf>,
}
struct ProjectView(ProjectItem);
impl Render for ProjectView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.0.clone().render("project", window, cx)
    }
}
impl HomeView {
    pub(super) fn sync_navigation(&mut self, source: Option<&str>, cx: &mut Context<Self>) {
        if self.state.read(cx).temporary {
            return;
        }
        let state = self.state.read(cx);
        let mut affected = HashSet::new();
        match source {
            Some(key) => {
                if let Some(session) = state.sessions.get(key) {
                    let row = (
                        key.to_owned(),
                        session.info.clone(),
                        session.activity(),
                        session.unread,
                    );
                    if self.navigation.rows.get(key) != Some(&row) {
                        if let Some(old) = self.navigation.rows.insert(key.to_owned(), row) {
                            affected.insert(old.1.cwd);
                        }
                        affected.insert(session.info.cwd.clone());
                    }
                    // The catalog alias disappears when a local draft gains a file.
                    let alias = session.info.key();
                    if alias != key
                        && !alias.is_empty()
                        && let Some(old) = self.navigation.rows.remove(&alias)
                    {
                        affected.insert(old.1.cwd);
                    }
                } else if let Some(old) = self.navigation.rows.remove(key) {
                    affected.insert(old.1.cwd);
                }
            }
            None => {
                let rows: BTreeMap<_, _> = state
                    .infos()
                    .into_iter()
                    .map(|(key, info)| {
                        let activity = state
                            .sessions
                            .get(&key)
                            .map(|s| s.activity())
                            .unwrap_or(Activity::Idle);
                        {
                            let unread = state.sessions.get(&key).is_some_and(|s| s.unread);
                            (key.clone(), (key, info, activity, unread))
                        }
                    })
                    .collect();
                for (key, row) in self.navigation.rows.iter().chain(rows.iter()) {
                    if self.navigation.rows.get(key) != rows.get(key) {
                        affected.insert(row.1.cwd.clone());
                    }
                }
                self.navigation.rows = rows;
            }
        }
        if source.is_some() && affected.is_empty() {
            return;
        }
        self.refresh_navigation(affected, cx);
    }
    pub(super) fn sync_navigation_selection(&mut self, cx: &mut Context<Self>) {
        self.refresh_navigation(HashSet::new(), cx);
    }
    fn refresh_navigation(&mut self, affected: HashSet<PathBuf>, cx: &mut Context<Self>) {
        let selected_key = self.state.read(cx).selected.clone();
        let mut changed = false;
        for cwd in affected {
            let mut rows = self
                .navigation
                .rows
                .values()
                .filter(|(_, info, _, _)| info.cwd == cwd)
                .cloned()
                .collect::<Vec<_>>();
            rows.sort_by(|(ak, a, _, _), (bk, b, _, _)| {
                b.activity.cmp(&a.activity).then(ak.cmp(bk))
            });
            if rows.is_empty() {
                changed |= self.navigation.groups.remove(&cwd).is_some();
                continue;
            }
            let selected = selected_key
                .as_ref()
                .filter(|key| rows.iter().any(|(k, _, _, _)| k == *key))
                .cloned();
            let item = ProjectItem {
                closed: !self.open_projects.contains(&cwd),
                more: self.projects_with_more.contains(&cwd),
                label: String::new(),
                cwd: cwd.clone(),
                rows,
                selected,
                state: self.state.clone(),
                owner: cx.weak_entity(),
            };
            if let Some(group) = self.navigation.groups.get(&cwd) {
                group.update(cx, |group, cx| {
                    let label = group.0.label.clone();
                    group.0 = item;
                    group.0.label = label;
                    cx.notify();
                });
            } else {
                self.navigation
                    .groups
                    .insert(cwd, cx.new(|_| ProjectView(item)));
            }
            changed = true;
        }
        let selected = self.state.read(cx).selected.clone();
        let basename = |path: &PathBuf| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        };
        let names = self
            .navigation
            .groups
            .keys()
            .map(basename)
            .collect::<Vec<_>>();
        for (cwd, group) in &self.navigation.groups {
            let name = basename(cwd);
            let label = if names.iter().filter(|other| **other == name).count() > 1 {
                cwd.parent()
                    .and_then(|p| p.file_name())
                    .map(|p| format!("{}/{}", p.to_string_lossy(), name))
                    .unwrap_or(name)
            } else {
                name
            };
            let current = group.read(cx);
            let selected = selected
                .as_ref()
                .filter(|key| current.0.rows.iter().any(|(k, _, _, _)| k == *key))
                .cloned();
            let closed = !self.open_projects.contains(cwd);
            let more = self.projects_with_more.contains(cwd);
            if current.0.selected != selected
                || current.0.closed != closed
                || current.0.more != more
                || current.0.label != label
            {
                group.update(cx, |group, cx| {
                    group.0.selected = selected;
                    group.0.closed = closed;
                    group.0.more = more;
                    group.0.label = label;
                    cx.notify();
                });
                changed = true;
            }
        }
        if changed {
            let mut order = self.navigation.groups.keys().cloned().collect::<Vec<_>>();
            order.sort_by(|a, b| {
                let a_row = &self.navigation.groups[a].read(cx).0.rows[0];
                let b_row = &self.navigation.groups[b].read(cx).0.rows[0];
                b_row
                    .1
                    .activity
                    .cmp(&a_row.1.activity)
                    .then(a_row.0.cmp(&b_row.0))
            });
            self.navigation.order = order;
            cx.notify();
        }
    }
}

pub(crate) fn unread_mark(cx: &App) -> impl IntoElement + use<> {
    div()
        .id("session-unread")
        .size_1p5()
        .rounded_full()
        .bg(cx.theme().primary)
        .role(Role::Status)
        .aria_label(t(cx, "notification-unread"))
}

#[cfg(test)]
mod sync_tests {
    use super::HomeView;
    use crate::state::{
        config::AppLanguage,
        conversation::{ConversationState, Session, notify, notify_session},
        pi,
    };
    use gpui_kit::component::Root;
    use gpui_kit::{AppContext, TestAppContext, px, size};
    use std::{path::PathBuf, rc::Rc};

    fn session(name: &str, cwd: &str) -> Session {
        let mut session = Session::from_rpc_messages(&[]);
        session.info.cwd = cwd.into();
        session.info.name = Some(name.into());
        session.state = Some(
            serde_json::from_value(serde_json::json!({
                "sessionId": name, "isStreaming": false, "isCompacting": false
            }))
            .unwrap(),
        );
        session
    }

    #[gpui_kit::test]
    fn inserting_a_session_preserves_other_project_views_and_background_updates_preserve_body(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            app_theme::init(cx);
            crate::state::theme::init(cx);
            crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
            pi::init(cx);
            cx.set_global(crate::state::layout::LayoutState::default());
        });
        let state = cx.new(|cx| ConversationState::new("unused".into(), cx));
        state.update(cx, |s, _| {
            s.sessions
                .insert("a".into(), session("Alpha", "/tmp/project-a"));
            s.sessions
                .insert("b".into(), session("Beta", "/tmp/project-b"));
            s.selected = Some("a".into());
        });
        let mut home = None;
        let (_, visual) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| HomeView::with_state(state.clone(), window, cx));
            home = Some(view.clone());
            Root::new(view, window, cx)
        });
        let home = home.unwrap();
        visual.simulate_resize(size(px(1200.), px(800.)));
        visual.run_until_parked();
        let (a, b, body, history) = visual.update(|_, cx| {
            let home = home.read(cx);
            (
                home.navigation.groups[&PathBuf::from("/tmp/project-a")].clone(),
                home.navigation.groups[&PathBuf::from("/tmp/project-b")].clone(),
                home.views["a"].rows.clone(),
                home.history_list.read(cx).delegate().rows.clone(),
            )
        });
        let notifications = Rc::new(std::cell::Cell::new(0));
        let captured = notifications.clone();
        let _subscription =
            visual.update(|_, cx| cx.observe(&b, move |_, _| captured.set(captured.get() + 1)));
        visual.update(|_, cx| {
            state.update(cx, |s, cx| {
                s.sessions
                    .insert("new".into(), session("New", "/tmp/project-a"));
                notify(cx);
            })
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            let home = home.read(cx);
            assert_eq!(home.navigation.groups[&PathBuf::from("/tmp/project-a")], a);
            assert_eq!(home.navigation.groups[&PathBuf::from("/tmp/project-b")], b);
            assert_eq!(a.read(cx).0.rows.len(), 2);
            assert!(Rc::ptr_eq(&home.views["a"].rows, &body));
            assert!(Rc::ptr_eq(
                &home.history_list.read(cx).delegate().rows,
                &history
            ));
        });
        assert_eq!(
            notifications.get(),
            0,
            "adding in A must not invalidate project B"
        );
        visual.update(|_, cx| {
            state.update(cx, |s, cx| {
                s.sessions.get_mut("b").unwrap().content_revision += 1;
                notify_session("b", cx);
            })
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            let home = home.read(cx);
            assert!(Rc::ptr_eq(&home.views["a"].rows, &body));
            assert!(Rc::ptr_eq(
                &home.history_list.read(cx).delegate().rows,
                &history
            ));
        });
        assert_eq!(
            notifications.get(),
            0,
            "body changes must not invalidate unchanged navigation metadata"
        );
        // Disclosure state remains on the owner while the retained project updates.
        visual.update(|_, cx| {
            home.update(cx, |home, cx| {
                home.open_projects.insert("/tmp/project-b".into());
                home.projects_with_more.insert("/tmp/project-b".into());
                home.sync_navigation(None, cx);
            })
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            assert!(!b.read(cx).0.closed);
            assert!(b.read(cx).0.more);
        });
    }
}
