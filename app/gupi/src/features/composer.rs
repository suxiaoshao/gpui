//! Shared input surface. Callers own draft state and business actions.
use gpui_kit::{
    component::{
        ActiveTheme, h_flex,
        input::{InputGroup, InputGroupAddon, InputGroupAddonAlignment, Textarea},
        v_flex,
    },
    *,
};

pub(crate) fn surface(cx: &App) -> Div {
    v_flex()
        .w_full()
        .min_w_0()
        .pb_2()
        .rounded_2xl()
        .shadow_sm()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

pub(crate) struct Composer {
    id: ElementId,
    input: Textarea,
    picker: AnyElement,
    leading: Vec<AnyElement>,
    actions: Option<AnyElement>,
}

impl Composer {
    pub fn new(id: impl Into<ElementId>, input: Textarea, picker: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            input,
            picker: picker.into_any_element(),
            leading: vec![],
            actions: None,
        }
    }

    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading.push(element.into_any_element());
        self
    }

    pub fn actions(mut self, element: impl IntoElement) -> Self {
        self.actions = Some(element.into_any_element());
        self
    }

    pub fn build(self) -> InputGroup {
        InputGroup::new(self.id).input(self.input).addon(
            InputGroupAddon::new("footer")
                .align(InputGroupAddonAlignment::BlockEnd)
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .items_center()
                        .flex_wrap()
                        .gap_2()
                        .children(self.leading)
                        .child(
                            h_flex()
                                .flex_1()
                                .min_w(px(300.))
                                .justify_end()
                                .items_center()
                                .gap_2()
                                .child(div().min_w_0().max_w(px(340.)).child(self.picker))
                                .children(self.actions),
                        ),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::Composer;
    use gpui_kit::component::{
        Root,
        button::Button,
        input::{Textarea, TextareaState},
    };
    use gpui_kit::{
        AppContext, ClipboardItem, Context, Entity, Focusable, InteractiveElement, IntoElement,
        Modifiers, Render, TestAppContext, Window,
    };
    use std::{cell::Cell, rc::Rc};

    struct Fixture {
        input: Entity<TextareaState>,
        clicks: Rc<Cell<usize>>,
        file_pastes: Rc<Cell<usize>>,
        readonly: bool,
        disabled: bool,
    }
    impl Render for Fixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            let file_pastes = self.file_pastes.clone();
            Composer::new(
                "fixture",
                Textarea::new(&self.input).on_paste(move |item, _, _| {
                    if item
                        .entries()
                        .iter()
                        .any(|entry| matches!(entry, gpui_kit::ClipboardEntry::ExternalPaths(_)))
                    {
                        file_pastes.set(file_pastes.get() + 1);
                        true
                    } else {
                        false
                    }
                }),
                Button::new("model")
                    .label("Model")
                    .debug_selector(|| "model".into())
                    .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
            )
            .build()
            .readonly(self.readonly)
            .disabled(self.disabled)
        }
    }

    #[gpui_kit::test]
    fn readonly_composer_preserves_actions_and_editable_paste_runs_once(cx: &mut TestAppContext) {
        exercise_composer(cx);
    }

    fn exercise_composer(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut fixture = None;
        let (_, visual) = cx.add_window_view(|window, cx| {
            let input = cx.new(|cx| TextareaState::new(window, cx).default_value("draft"));
            let view = cx.new(|_| Fixture {
                input,
                clicks: Rc::new(Cell::new(0)),
                file_pastes: Rc::new(Cell::new(0)),
                readonly: true,
                disabled: false,
            });
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();
        visual.run_until_parked();
        let bounds = visual.debug_bounds("model").unwrap();
        visual.simulate_click(bounds.center(), Modifiers::default());
        visual.update(|window, cx| {
            assert_eq!(fixture.read(cx).clicks.get(), 1);
            fixture.read(cx).input.focus_handle(cx).focus(window, cx);
            cx.write_to_clipboard(ClipboardItem::new_string(" appended".into()));
        });
        visual.dispatch_action(gpui_kit::component::input::Paste);
        visual.simulate_input("blocked");
        visual.update(|_, cx| {
            assert_eq!(fixture.read(cx).input.read(cx).value(), "draft");
            fixture.update(cx, |view, cx| {
                view.readonly = false;
                cx.notify();
            });
        });
        visual.run_until_parked();
        visual.update(|_, cx| {
            cx.write_to_clipboard(
                gpui_kit::ClipboardEntry::ExternalPaths(gpui_kit::ExternalPaths(
                    [std::path::PathBuf::from("/tmp/fixture.txt")]
                        .into_iter()
                        .collect(),
                ))
                .into(),
            )
        });
        visual.dispatch_action(gpui_kit::component::input::Paste);
        visual.update(|_, cx| {
            assert_eq!(fixture.read(cx).file_pastes.get(), 1);
            assert_eq!(
                fixture.read(cx).input.read(cx).value(),
                "draft",
                "handled attachment must not also insert a path"
            );
            cx.write_to_clipboard(ClipboardItem::new_string(" appended".into()));
        });
        visual.dispatch_action(gpui_kit::component::input::MoveToEnd);
        visual.dispatch_action(gpui_kit::component::input::Paste);
        visual.run_until_parked();
        visual.update(|_, cx| {
            assert_eq!(fixture.read(cx).input.read(cx).value(), "draft appended");
            fixture.update(cx, |view, cx| {
                view.disabled = true;
                cx.notify();
            });
        });
        visual.run_until_parked();
        let bounds = visual.debug_bounds("model").unwrap();
        visual.simulate_click(bounds.center(), Modifiers::default());
        visual.update(|_, cx| assert_eq!(fixture.read(cx).clicks.get(), 1));
    }
}
