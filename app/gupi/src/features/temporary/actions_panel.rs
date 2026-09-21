use super::*;
use gpui_kit::base::actions::{Cancel, Confirm, SelectDown, SelectUp};
use gpui_kit::component::{
    command::{Command, CommandGroup, CommandItem, CommandState},
    input::Escape,
};

pub(super) struct ActionsPanel {
    owner: WeakEntity<TemporaryView>,
    target: Option<String>,
    input: Entity<InputState>,
    list: Entity<CommandState>,
    pub(super) original_focus: Option<FocusHandle>,
    _subscription: Subscription,
}
impl ActionsPanel {
    pub(super) fn new(
        owner: WeakEntity<TemporaryView>,
        target: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let original_focus = window.focused(cx);
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t(cx, "temporary-search-actions")));
        let subscription = cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        Self {
            owner,
            target,
            input,
            list: cx.new(|cx| CommandState::new(window, cx)),
            original_focus,
            _subscription: subscription,
        }
    }
}
impl Focusable for ActionsPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
impl Render for ActionsPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(owner) = self.owner.upgrade() else {
            return div().into_any_element();
        };
        let view = owner.read(cx);
        let state = view.state.read(cx);
        let valid = state.selected == self.target;
        let title = state
            .current()
            .map(|s| navigation::display_title(&s.info, cx))
            .unwrap_or_else(|| t(cx, "temporary-title"));
        let home = view.home.read(cx);
        let query = self.input.read(cx).value().trim().to_lowercase();
        let mut groups = Vec::new();
        let mut choices = Vec::new();
        for kinds in [
            &[Kind::PasteAnswer, Kind::CopyTemporaryAnswer][..],
            &[
                Kind::FocusInput,
                Kind::Model,
                Kind::RevealWorkspace,
                Kind::TrashTemporary,
            ][..],
            &[Kind::New, Kind::Stop][..],
        ] {
            let mut group = CommandGroup::new();
            let mut rows = Vec::new();
            for &kind in kinds {
                let label = home.command_label(kind, cx);
                if !query.split_whitespace().all(|word| {
                    format!("{label} {}", kind.search_terms())
                        .to_lowercase()
                        .contains(word)
                }) {
                    continue;
                }
                let enabled = valid
                    && match kind {
                        Kind::PasteAnswer | Kind::CopyTemporaryAnswer => view.has_answer(),
                        Kind::RevealWorkspace => state.current().is_some(),
                        Kind::TrashTemporary => home.action_enabled(Kind::Delete, cx),
                        _ => home.action_enabled(kind, cx),
                    };
                let icon = match kind {
                    Kind::PasteAnswer => IconName::ArrowRight,
                    Kind::CopyTemporaryAnswer => IconName::Copy,
                    Kind::FocusInput => IconName::SquarePen,
                    Kind::Model => IconName::Brain,
                    Kind::RevealWorkspace => IconName::FolderOpen,
                    Kind::TrashTemporary => IconName::Trash,
                    Kind::New => IconName::Plus,
                    Kind::Stop if state.current().is_some_and(|s| s.busy()) => IconName::Square,
                    _ => IconName::X,
                };
                let stopping = kind == Kind::Stop && state.current().is_some_and(|s| s.busy());
                let item = CommandItem::new()
                    .label(label.clone())
                    .disabled(!enabled)
                    .child(move |window, cx| {
                        h_flex()
                            .id(format!("temporary-action-{kind:?}"))
                            .when(kind == Kind::Stop, |row| {
                                row.debug_selector(move || {
                                    if stopping {
                                        "temporary-action-stop"
                                    } else {
                                        "temporary-action-hide"
                                    }
                                    .into()
                                })
                            })
                            .w_full()
                            .min_w_0()
                            .gap_2()
                            .child(Icon::new(icon).size_4())
                            .child(
                                div()
                                    .flex_1()
                                    .truncate()
                                    .when(kind == Kind::TrashTemporary, |row| {
                                        row.text_color(cx.theme().danger)
                                    })
                                    .child(label.clone()),
                            )
                            .children(crate::features::command_palette::binding(kind, window))
                    });
                group = group.item(item);
                rows.push(kind);
            }
            if !rows.is_empty() {
                groups.push(group);
                choices.push(rows);
            }
        }
        let owner = self.owner.clone();
        let cancel_owner = owner.clone();
        let input = self.input.clone();
        let target = self.target.clone();
        let command = groups.into_iter().enumerate().fold(
            Command::new(&self.list)
                .searchable(false)
                .bordered(false)
                .max_h(px(350.)),
            |command, (i, group)| {
                command
                    .when(i > 0, |command| command.separator())
                    .group(group)
            },
        );
        v_flex()
            .key_context("GupiTemporaryActions")
            .w(px(360.))
            .p_1()
            .rounded(cx.theme().radius_lg)
            .overflow_hidden()
            .on_action(cx.listener(|_, _: &input::MoveUp, window, cx| {
                window.dispatch_action(Box::new(SelectUp), cx)
            }))
            .on_action(cx.listener(|_, _: &input::MoveDown, window, cx| {
                window.dispatch_action(Box::new(SelectDown), cx)
            }))
            .on_action(cx.listener(|this, _: &input::Enter, window, cx| {
                if !this.input.update(cx, |input, cx| {
                    input.marked_text_range(window, cx).is_some()
                }) {
                    window.dispatch_action(Box::new(Confirm { secondary: false }), cx);
                }
            }))
            .on_action(
                cx.listener(|_, _: &Escape, window, cx| {
                    window.dispatch_action(Box::new(Cancel), cx)
                }),
            )
            .child(
                command
                    .header(move |_, _, cx| {
                        div()
                            .px_3()
                            .py_2()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(title.clone())
                    })
                    .footer(move |_, _, cx| {
                        div()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .child(Input::new(&input).appearance(false).p_0())
                    })
                    .on_confirm(move |ix, window, cx| {
                        let Some(&kind) =
                            choices.get(ix.section).and_then(|group| group.get(ix.row))
                        else {
                            return;
                        };
                        let _ = owner.update(cx, |view, cx| {
                            if view.state.read(cx).selected != target {
                                return;
                            }
                            view.close_actions(window, cx);
                            view.run(&Run(kind), window, cx);
                        });
                    })
                    .on_cancel(move |window, cx| {
                        let _ = cancel_owner.update(cx, |view, cx| view.close_actions(window, cx));
                    }),
            )
            .into_any_element()
    }
}
