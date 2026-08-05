use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, View, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_component::{
    combobox::{ComboboxEvent, ComboboxState},
    input::{InputEvent, InputState},
    select::{SelectEvent, SelectState},
};
use gpui_form::{Form, FormSchema};
use gpui_form_gpui_component::{
    FormCombobox, FormInput, FormIntegerInput, FormSelect, IntegerInput, IntegerInputError,
    IntegerInputEvent, IntegerInputPolicyError, IntegerInputState,
};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Details {
    note: String,
    budget: u32,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct AdapterInput {
    #[form(required, validate(on_blur, on_submit))]
    name: String,
    budget: u32,
    model: Option<String>,
    tools: Vec<String>,
    #[form(child)]
    details: Option<Details>,
}

struct AdapterHarness {
    form: Entity<Form<AdapterInput>>,
    input: Entity<InputState>,
    control: Option<FormInput>,
    integer_input: Entity<InputState>,
    integer_control: Option<FormIntegerInput<u32>>,
    select_control: FormSelect<Vec<String>>,
    combobox_control: FormCombobox<Vec<String>>,
}

impl AdapterHarness {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| {
            Form::try_new(AdapterInput {
                name: "initial".to_string(),
                budget: 1024,
                model: Some("beta".to_string()),
                tools: vec!["beta".to_string()],
                details: Some(Details {
                    note: "note".to_string(),
                    budget: 7,
                }),
            })
            .expect("build form")
        });
        let control = FormInput::new(&form, AdapterInput::NAME, InputState::new, window, cx);
        let input = (*control).clone();
        let integer_control = FormIntegerInput::new(
            &form,
            AdapterInput::BUDGET,
            |window, cx| IntegerInputState::new(window, cx).min(1).max(4096).step(1),
            window,
            cx,
        )
        .expect("bind integer input");
        let integer_input = integer_control.read(cx).editor().clone();
        let options = vec!["alpha".to_string(), "beta".to_string()];
        let select_control = FormSelect::new(
            &form,
            AdapterInput::MODEL,
            {
                let options = options.clone();
                move |window, cx| SelectState::new(options, None, window, cx)
            },
            window,
            cx,
        );
        let combobox_control = FormCombobox::new(
            &form,
            AdapterInput::TOOLS,
            move |window, cx| ComboboxState::new(options, Vec::new(), window, cx).multiple(true),
            window,
            cx,
        );

        Self {
            form,
            input,
            control: Some(control),
            integer_input,
            integer_control: Some(integer_control),
            select_control,
            combobox_control,
        }
    }
}

impl Render for AdapterHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn open_harness(cx: &mut TestAppContext) -> WindowHandle<AdapterHarness> {
    cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| AdapterHarness::new(window, cx))
        })
        .expect("open adapter test window")
    })
}

fn entities(
    root: &Entity<AdapterHarness>,
    cx: &mut VisualTestContext,
) -> (Entity<Form<AdapterInput>>, Entity<InputState>) {
    cx.update(|_, cx| root.read_with(cx, |root, _| (root.form.clone(), root.input.clone())))
}

#[gpui::test]
fn total_input_mirrors_form_and_component_without_echo(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);

    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("user", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(form.read(cx).value().name, "user"));

    cx.update(|_, cx| AdapterInput::NAME.set(&form, "external".to_string(), cx));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "external"));
}

#[gpui::test]
fn dynamic_input_stops_writing_after_optional_payload_is_replaced(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");

    let (form, dynamic_control) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let details = AdapterInput::ROOT
                .then(AdapterInput::DETAILS)
                .try_some(root.form.read(cx))
                .expect("details are present");
            let path = details.then(Details::NOTE);
            let control = FormInput::try_new(&root.form, path, InputState::new, window, cx)
                .expect("bind dynamic input");
            (root.form.clone(), control)
        })
    });
    let dynamic_input = (*dynamic_control).clone();

    cx.update(|_, cx| {
        let details = AdapterInput::ROOT.then(AdapterInput::DETAILS);
        details.set(&form, None, cx);
        details.set(
            &form,
            Some(Details {
                note: "replacement".to_string(),
                budget: 9,
            }),
            cx,
        );
    });
    cx.update(|window, cx| {
        dynamic_input.update(cx, |input, cx| {
            input.set_value("stale", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            dynamic_input.read(cx).value().as_ref(),
            "stale",
            "the retired native control keeps its own editor text"
        );
        assert_eq!(dynamic_input.entity_id(), (*dynamic_control).entity_id());
        assert_eq!(
            form.read(cx)
                .value()
                .details
                .as_ref()
                .expect("replacement details")
                .note,
            "replacement"
        );
    });
    drop(dynamic_input);
}

#[gpui::test]
fn dropping_bound_control_stops_component_to_form_sync(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);
    cx.update(|_, cx| root.update(cx, |root, _| root.control = None));
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("detached", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(form.read(cx).value().name, "initial"));
}

#[gpui::test]
fn dropping_bound_control_cancels_already_queued_callback(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("queued", window, cx);
            cx.emit(InputEvent::Change);
        });
        root.update(cx, |root, _| root.control = None);
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(form.read(cx).value().name, "initial"));
}

#[gpui::test]
fn whole_model_replacement_cancels_queued_total_callback(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("queued", window, cx);
            cx.emit(InputEvent::Change);
        });
        form.update(cx, |form, cx| {
            let mut replacement = form.value().clone();
            replacement.name = "replacement".into();
            form.replace(replacement, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(form.read(cx).value().name, "replacement"));
}

#[gpui::test]
fn dropping_integer_control_releases_its_blocking_issue(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, integer_control) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| {
            (
                root.form.clone(),
                std::ops::Deref::deref(root.integer_control.as_ref().unwrap()).clone(),
            )
        })
    });
    cx.update(|_, cx| {
        integer_control.update(cx, |_, cx| {
            cx.emit(IntegerInputEvent::Change(Err(
                IntegerInputError::InvalidSyntax,
            )));
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        root.update(cx, |root, _| root.integer_control = None);
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok());
    });
}

#[gpui::test]
fn retiring_dynamic_subtree_revokes_control_issue_and_callbacks(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, control) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let details = AdapterInput::ROOT
                .then(AdapterInput::DETAILS)
                .try_some(root.form.read(cx))
                .unwrap();
            let control = FormIntegerInput::try_new(
                &root.form,
                details.then(Details::BUDGET),
                IntegerInputState::new,
                window,
                cx,
            )
            .unwrap();
            (root.form.clone(), control)
        })
    });
    let state = std::ops::Deref::deref(&control).clone();
    cx.update(|_, cx| {
        state.update(cx, |_, cx| {
            cx.emit(IntegerInputEvent::Change(Err(
                IntegerInputError::InvalidSyntax,
            )));
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        AdapterInput::ROOT
            .then(AdapterInput::DETAILS)
            .set(&form, None, cx);
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok());
    });
    cx.update(|_, cx| {
        state.update(cx, |_, cx| cx.emit(IntegerInputEvent::Change(Ok(99))));
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(form.read(cx).value().details.is_none()));
}

#[gpui::test]
fn integer_select_and_combobox_use_typed_total_paths(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");

    let form = cx.update(|_window, cx| {
        let (form, integer_control, integer_input, select, combobox) =
            root.read_with(cx, |root, _| {
                (
                    root.form.clone(),
                    std::ops::Deref::deref(root.integer_control.as_ref().expect("integer control"))
                        .clone(),
                    root.integer_input.clone(),
                    std::ops::Deref::deref(&root.select_control).clone(),
                    std::ops::Deref::deref(&root.combobox_control).clone(),
                )
            });
        assert_eq!(
            IntegerInput::new(&integer_control).entity_id(),
            Some(integer_control.entity_id())
        );
        assert_eq!(
            integer_control.read(cx).editor().entity_id(),
            integer_input.entity_id()
        );
        select.update(cx, |_, cx| {
            cx.emit(SelectEvent::Confirm(Some("alpha".to_string())))
        });
        combobox.update(cx, |_, cx| {
            cx.emit(ComboboxEvent::Change(vec!["alpha".to_string()]))
        });
        form
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(form.read(cx).value().model.as_deref(), Some("alpha"));
        assert_eq!(form.read(cx).value().tools, ["alpha".to_string()]);
    });
}

#[test]
fn integer_policy_errors_are_stable() {
    assert_eq!(
        IntegerInputPolicyError::NonPositiveStep.to_string(),
        "integer input step must be positive"
    );
}
