use gpui::{
    AppContext, Context, Entity, EntityInputHandler, IntoElement, ParentElement, Render,
    Subscription, TestAppContext, Window,
};
use gpui_component::{
    Root,
    input::{Input, InputEditsEvent, InputEvent, InputState, NumberInputEvent, StepAction},
    v_flex,
};
use gpui_form::{ControlMapping, EditResult, Form, FormSchema};
use gpui_form_gpui_component::{
    FormIntegerInput, FormMappedInput, IntegerInput, IntegerInputEvent, IntegerInputState,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, PartialEq, FormSchema)]
struct Model {
    #[form(items)]
    rows: Vec<Values>,
    accepted: Vec<i32>,
    count: u32,
}
#[derive(Clone, PartialEq, FormSchema)]
struct Values {
    accepted: Vec<i32>,
    count: u32,
}
struct Mapping(Rc<RefCell<Vec<String>>>);
impl ControlMapping<Vec<i32>, String> for Mapping {
    type Context = ();
    fn format(&self, storage: &Vec<i32>, _: &()) -> String {
        storage.last().unwrap().to_string()
    }
    fn parse(
        &self,
        text: String,
        _: &Vec<i32>,
        _: Option<&Vec<i32>>,
        _: &(),
    ) -> EditResult<Vec<i32>, String> {
        self.0.borrow_mut().push(text.clone());
        match text.parse::<i32>() {
            Ok(value) => EditResult::update(move |latest: &Vec<i32>| {
                let mut next = latest.clone();
                next.push(value);
                Ok(next)
            }),
            Err(_) => EditResult::draft(text, "number", "invalid integer"),
        }
    }
}
struct Harness {
    form: Entity<Form<Model>>,
    mapped: FormMappedInput<()>,
    integer: Option<FormIntegerInput<u32>>,
    integer_editor: Entity<InputState>,
    snapshots: Rc<RefCell<Vec<String>>>,
    changes: Rc<RefCell<Vec<String>>>,
    parses: Rc<RefCell<Vec<String>>>,
    integers: Rc<RefCell<Vec<String>>>,
    _subscriptions: Vec<Subscription>,
}
impl Harness {
    fn new(dynamic: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| {
            Form::new(Model {
                rows: vec![Values {
                    accepted: vec![0],
                    count: 0,
                }],
                accepted: vec![0],
                count: 0,
            })
        });
        let parses = Rc::new(RefCell::new(Vec::new()));
        let row = Model::ROOT.then(Model::ROWS).items(&form, cx).remove(0);
        let mapped = if dynamic {
            FormMappedInput::try_new(
                &form,
                row.clone().then(Values::ACCEPTED),
                "mapped",
                Mapping(parses.clone()),
                (),
                InputState::new,
                window,
                cx,
            )
        } else {
            FormMappedInput::new(
                &form,
                Model::ACCEPTED,
                "mapped",
                Mapping(parses.clone()),
                (),
                InputState::new,
                window,
                cx,
            )
        }
        .unwrap();
        let integer = if dynamic {
            FormIntegerInput::try_new(
                &form,
                row.then(Values::COUNT),
                "integer",
                IntegerInputState::new,
                window,
                cx,
            )
        } else {
            FormIntegerInput::new(
                &form,
                Model::COUNT,
                "integer",
                IntegerInputState::new,
                window,
                cx,
            )
        }
        .unwrap();
        let integer_editor = integer.read(cx).editor().clone();
        let snapshots = Rc::new(RefCell::new(Vec::new()));
        let changes = Rc::new(RefCell::new(Vec::new()));
        let integers = Rc::new(RefCell::new(Vec::new()));
        let out = snapshots.clone();
        let edits = cx.subscribe_in(
            &mapped,
            window,
            move |_, _, event: &InputEditsEvent, _, _| {
                out.borrow_mut()
                    .extend(event.edits().iter().map(|(_, text)| text.to_string()));
            },
        );
        let out = changes.clone();
        let change = cx.subscribe_in(
            &mapped,
            window,
            move |_, state, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    out.borrow_mut().push(state.read(cx).value().to_string());
                }
            },
        );
        let out = integers.clone();
        let numbers = cx.subscribe_in(
            &integer,
            window,
            move |_, _, event: &IntegerInputEvent<u32>, _, _| {
                if let IntegerInputEvent::Change(value) = event {
                    out.borrow_mut().push(format!("{value:?}"));
                }
            },
        );
        Self {
            form,
            mapped,
            integer: Some(integer),
            integer_editor,
            snapshots,
            changes,
            parses,
            integers,
            _subscriptions: vec![edits, change, numbers],
        }
    }
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .child(Input::new(&self.mapped))
            .child(IntegerInput::new(self.integer.as_ref().unwrap()))
    }
}
#[gpui::test]
fn same_frame_total_native_edits(cx: &mut TestAppContext) {
    same_frame_native_edits(cx, false);
}
#[gpui::test]
fn same_frame_dynamic_native_edits(cx: &mut TestAppContext) {
    same_frame_native_edits(cx, true);
}
fn same_frame_native_edits(cx: &mut TestAppContext, dynamic: bool) {
    cx.update(gpui_component::init);
    let mut owner = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Harness::new(dynamic, window, cx));
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    let owner = owner.unwrap();
    visual.run_until_parked();
    visual.update(|window, cx| {
        owner.update(cx, |this, cx| {
            for text in ["1", "-", "2"] {
                this.mapped.update(cx, |state, cx| {
                    state.replace_text_in_range(Some(0..state.value().len()), text, window, cx)
                });
                this.integer_editor.update(cx, |state, cx| {
                    state.replace_text_in_range(Some(0..state.value().len()), text, window, cx)
                });
            }
        })
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        let this = owner.read(cx);
        assert_eq!(*this.snapshots.borrow(), vec!["1", "-", "2"]);
        assert_eq!(*this.changes.borrow(), vec!["2", "2", "2"]);
        assert_eq!(*this.parses.borrow(), vec!["1", "-", "2"]);
        let model = Model::ROOT.get(&this.form, cx);
        let (accepted, count) = if dynamic {
            (&model.rows[0].accepted, model.rows[0].count)
        } else {
            (&model.accepted, model.count)
        };
        assert_eq!(*accepted, vec![0, 1, 2]);
        assert_eq!(count, 2);
        assert_eq!(
            this.form.read(cx).revision().get(),
            6,
            "both intermediate drafts must enter Form, not only notify observers"
        );
        assert_eq!(
            *this.integers.borrow(),
            vec!["Ok(1)", "Err(Incomplete)", "Ok(2)"]
        );

        assert!(
            !this
                .integer
                .as_ref()
                .unwrap()
                .editor_status(cx)
                .unwrap()
                .has_draft()
        );
    });
    visual.update(|window, cx| {
        owner.update(cx, |this, cx| {
            for text in ["3", "-"] {
                this.integer_editor.update(cx, |state, cx| {
                    state.replace_text_in_range(Some(0..state.value().len()), text, window, cx)
                });
            }
        });
    });
    visual.run_until_parked();
    visual.update(|window, cx| {
        owner.update(cx, |this, cx| {
            assert_eq!(
                *this.integers.borrow(),
                vec![
                    "Ok(1)",
                    "Err(Incomplete)",
                    "Ok(2)",
                    "Ok(3)",
                    "Err(Incomplete)"
                ]
            );
            assert!(
                this.integer
                    .as_ref()
                    .unwrap()
                    .editor_status(cx)
                    .unwrap()
                    .has_draft()
            );
            assert!(this.form.update(cx, |form, cx| form.prepare(cx)).is_err());
            // Remount through the public constructor: the retained draft is "-", not "3".
            drop(this.integer.take());
            let row = Model::ROOT
                .then(Model::ROWS)
                .items(&this.form, cx)
                .remove(0);
            let restored = if dynamic {
                FormIntegerInput::try_new(
                    &this.form,
                    row.then(Values::COUNT),
                    "integer",
                    IntegerInputState::new,
                    window,
                    cx,
                )
            } else {
                FormIntegerInput::new(
                    &this.form,
                    Model::COUNT,
                    "integer",
                    IntegerInputState::new,
                    window,
                    cx,
                )
            }
            .unwrap();
            assert_eq!(restored.read(cx).editor().read(cx).value().as_ref(), "-");
            this.integer = Some(restored);
        });
    });
    visual.update(|_, cx| {
        owner
            .read(cx)
            .integer
            .as_ref()
            .unwrap()
            .read(cx)
            .editor()
            .clone()
            .update(cx, |_, cx| {
                cx.emit(NumberInputEvent::Step(StepAction::Increment));
            });
    });
    visual.run_until_parked();
    visual.update(|_, cx| {
        let this = owner.read(cx);
        let model = Model::ROOT.get(&this.form, cx);
        assert_eq!(
            if dynamic {
                model.rows[0].count
            } else {
                model.count
            },
            4
        );
        assert!(
            !this
                .integer
                .as_ref()
                .unwrap()
                .editor_status(cx)
                .unwrap()
                .has_draft()
        );
    });
}
