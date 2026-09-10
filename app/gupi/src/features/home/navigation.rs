use super::*;
use crate::{app::menus, foundation::session_catalog::SessionInfo, state::conversation::Activity};
use gpui_kit::component::{
    Collapsible as CollapsibleTrait, Icon, StyledExt,
    command::{Command, CommandItem, CommandState},
    input::{Input, InputState},
    label::Label,
    menu::{ContextMenuExt, PopupMenu, PopupMenuItem},
    sidebar::{Sidebar, SidebarItem},
    spinner::Spinner,
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder as _;

type SessionRow = (String, SessionInfo, Activity);

#[derive(Clone)]
enum NavigationItem {
    Project(ProjectItem),
    Loading { refresh: bool },
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
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        match self {
            Self::Project(project) => project.render(id, window, cx).into_any_element(),
            Self::Loading { refresh } => catalog_loading(refresh, cx),
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

fn catalog_loading(refresh: bool, cx: &App) -> AnyElement {
    let label = t(
        cx,
        if refresh {
            "conversation-refreshing"
        } else {
            "conversation-scanning"
        },
    );
    h_flex()
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
        .child(Spinner::new().small())
        .child(Label::new(label).truncate().min_w_0())
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
                .map(|(_, _, a)| *a)
                .max()
                .unwrap_or(Activity::Idle)
        } else {
            Activity::Idle
        };
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
                    cx.notify();
                });
            },
        )
        .group(group.clone())
        .pr_8()
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .when(activity != Activity::Idle, |row| {
            row.child(
                action_slot()
                    .group_hover(group.clone(), |style| style.opacity(0.))
                    .child(activity_mark(activity, cx)),
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
                        new_state.update(cx, |s, cx| s.new_draft(Some(new_cwd.clone()), cx));
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
                    state.update(cx, |s, cx| s.new_draft(Some(cwd.clone()), cx))
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
            for (index, (key, info, activity)) in self.rows.iter().enumerate() {
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
                .when(*activity != Activity::Idle, |row| {
                    row.pr_8()
                        .child(action_slot().child(activity_mark(*activity, cx)))
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
fn session_menu(
    menu: PopupMenu,
    key: String,
    state: Entity<ConversationState>,
    owner: WeakEntity<HomeView>,
    cx: &App,
) -> PopupMenu {
    let current = state.read(cx).sessions.get(&key);
    let busy = current.is_some_and(|s| s.busy());
    let connected = current.is_some_and(|s| s.instance.is_some());
    let path = current
        .map(|s| s.info.path.clone())
        .or_else(|| {
            state
                .read(cx)
                .catalog
                .iter()
                .flat_map(|catalog| &catalog.sessions)
                .find(|i| i.key() == key)
                .map(|i| i.path.clone())
        })
        .unwrap_or_default();
    let rename_key = key.clone();
    let reveal = path.clone();
    let copy = path.clone();
    let mut menu = menu
        .item(
            PopupMenuItem::new(t(cx, "conversation-rename"))
                .disabled(busy || path.as_os_str().is_empty())
                .on_click(move |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.rename_dialog(rename_key.clone(), window, cx)
                    });
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
    menu
}
pub(super) fn display_title(info: &SessionInfo, cx: &App) -> String {
    if info.title().is_empty() {
        t(cx, "conversation-untitled")
    } else {
        info.title().to_owned()
    }
}
fn activity_mark(activity: Activity, cx: &App) -> AnyElement {
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
    pub(super) fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let mut projects: Vec<(PathBuf, Vec<SessionRow>)> = vec![];
        for (key, info) in state.infos() {
            let activity = state
                .sessions
                .get(&key)
                .map(|s| s.activity())
                .unwrap_or(Activity::Idle);
            if let Some((_, rows)) = projects.iter_mut().find(|(cwd, _)| *cwd == info.cwd) {
                rows.push((key, info, activity));
            } else {
                projects.push((info.cwd.clone(), vec![(key, info, activity)]));
            }
        }
        let basename = |path: &PathBuf| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        };
        let names = projects
            .iter()
            .map(|(path, _)| basename(path))
            .collect::<Vec<_>>();
        let groups = projects
            .into_iter()
            .map(|(cwd, rows)| {
                let name = basename(&cwd);
                let label = if names.iter().filter(|n| **n == name).count() > 1 {
                    cwd.parent()
                        .and_then(|p| p.file_name())
                        .map(|p| format!("{}/{}", p.to_string_lossy(), name))
                        .unwrap_or(name)
                } else {
                    name
                };
                ProjectItem {
                    closed: !self.open_projects.contains(&cwd),
                    more: self.projects_with_more.contains(&cwd),
                    cwd,
                    label,
                    rows,
                    selected: state.selected.clone(),
                    state: self.state.clone(),
                    owner: cx.entity().downgrade(),
                }
            })
            .collect::<Vec<_>>();
        let mut items = Vec::new();
        if state.scanning() && state.catalog.is_none() {
            items.push(NavigationItem::Loading { refresh: false });
        }
        if state.catalog.is_some() {
            if groups.is_empty() && !state.scanning() {
                items.push(NavigationItem::Empty);
            }
            items.extend(groups.into_iter().map(NavigationItem::Project));
        }
        let owner = cx.entity().downgrade();
        let search_owner = owner.clone();
        let mut header = v_flex()
            .w_full()
            .gap_1()
            .child(navigation_row(
                "new-conversation",
                t(cx, "conversation-new"),
                Some(IconName::Plus),
                false,
                cx,
                move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.state.update(cx, |s, cx| s.new_draft(None, cx));
                        this.input.update(cx, |s, cx| s.focus(window, cx));
                    });
                },
            ))
            .child(navigation_row(
                "search-sessions",
                t(cx, "conversation-search"),
                Some(IconName::Search),
                false,
                cx,
                move |window, cx| {
                    let _ = search_owner.update(cx, |this, cx| this.search_dialog(window, cx));
                },
            ));
        if state.scanning() && state.catalog.is_some() {
            header = header.child(catalog_loading(true, cx));
        }
        let refresh = self.state.clone();
        let mut footer = v_flex()
            .w_full()
            .gap_1()
            .child(navigation_row(
                "sidebar-settings",
                t(cx, "menu-settings"),
                Some(IconName::Settings),
                false,
                cx,
                |window, cx| window.dispatch_action(Box::new(menus::ShowSettings), cx),
            ))
            .child(navigation_row(
                "refresh-sessions",
                t(cx, "conversation-refresh"),
                Some(IconName::RotateCw),
                false,
                cx,
                move |_, cx| refresh.update(cx, |s, cx| s.scan(cx)),
            ));
        if state
            .catalog
            .as_ref()
            .is_some_and(|catalog| !catalog.warnings.is_empty())
        {
            let owner = cx.entity().downgrade();
            footer = footer.child(navigation_row(
                "catalog-warnings",
                t(cx, "conversation-scan-warning"),
                Some(IconName::CircleAlert),
                false,
                cx,
                move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        let text = this
                            .state
                            .read(cx)
                            .catalog
                            .as_ref()
                            .map(|catalog| catalog.warnings.join("\n"))
                            .unwrap_or_default();
                        window.open_dialog(cx, move |dialog, _, cx| {
                            dialog
                                .title(t(cx, "conversation-scan-warning"))
                                .child(div().text_sm().child(text.clone()))
                        });
                    });
                },
            ));
        }
        Sidebar::new("sessions-sidebar")
            .collapsible(false)
            .collapsed(false)
            .border_r_0()
            .header(header)
            .children(items)
            .footer(footer)
            .w_full()
            .h_full()
            .into_any_element()
    }
    fn search_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let infos = self.state.read(cx).infos();
        let items = infos
            .iter()
            .map(|(_, i)| {
                CommandItem::new().label(display_title(i, cx)).keywords([
                    i.cwd.to_string_lossy().into_owned(),
                    i.first_message.clone(),
                ])
            })
            .collect::<Vec<_>>();
        let keys = infos.into_iter().map(|(k, _)| k).collect::<Vec<_>>();
        let state = self.state.clone();
        let search = cx.new(|cx| CommandState::new(window, cx));
        let search_focus = search.clone();
        window.open_dialog(cx, move |dialog, _, cx| {
            let keys = keys.clone();
            let state = state.clone();
            dialog
                .title(t(cx, "conversation-search"))
                .w(px(560.))
                .child(
                    Command::new(&search)
                        .items(items.clone())
                        .placeholder(t(cx, "conversation-search-placeholder"))
                        .empty(|_, _, cx| div().p_4().child(t(cx, "conversation-search-empty")))
                        .on_confirm(move |ix, window, cx| {
                            if let Some(key) = keys.get(ix.row) {
                                state.update(cx, |s, cx| s.open(key, cx));
                                window.close_dialog(cx);
                            }
                        }),
                )
        });
        search_focus.update(cx, |search, cx| search.focus(window, cx));
    }
    fn rename_dialog(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        // Opening the rename action may restore this source; merely opening its menu does not.
        if !self.state.read(cx).sessions.contains_key(&key) {
            let current = self.state.read(cx).selected.clone();
            self.state.update(cx, |s, cx| {
                s.open(&key, cx);
                s.selected = current;
            });
        }
        self.state.update(cx, |s, cx| s.connect(&key, cx));
        let title = self
            .state
            .read(cx)
            .sessions
            .get(&key)
            .map(|s| s.info.title().to_owned())
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
                    let ready = state
                        .read(cx)
                        .sessions
                        .get(&key)
                        .is_some_and(|s| s.state.is_some());
                    if !ready {
                        return false;
                    }
                    state.update(cx, |s, cx| s.rename(&key, name.trim().into(), cx));
                    true
                })
        });
    }
}
