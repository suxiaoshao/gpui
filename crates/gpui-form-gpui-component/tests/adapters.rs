use gpui::{
    AppContext as _, Context, Entity, EventEmitter, IntoElement, Render, Subscription,
    TestAppContext, View, VisualTestContext, Window, WindowHandle, div,
};
use gpui_component::{
    combobox::{ComboboxEvent, ComboboxState},
    input::{EditorState, InputEvent, InputState, TextareaState},
    select::{SelectEvent, SelectState},
};
use gpui_form::{ControlBinding, ControlProjection, DynamicPath, Form, FormSchema, ResolveError};
use gpui_form_gpui_component::{
    FormCombobox, FormEditor, FormInput, FormIntegerInput, FormSelect, FormTextarea, IntegerInput,
    IntegerInputError, IntegerInputEvent, IntegerInputPolicyError, IntegerInputState,
};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Details {
    note: String,
    budget: u32,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Row {
    value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct AdapterInput {
    #[form(required, validate(on_blur, on_submit))]
    name: String,
    budget: u32,
    model: Option<String>,
    tools: Vec<String>,
    #[form(items)]
    rows: Vec<Row>,
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

#[derive(Clone)]
enum ProbeEvent {
    Set(String),
}

struct ProjectionProbe {
    values: Vec<String>,
    retired: usize,
    subscriptions: Vec<Subscription>,
    _binding: Option<ControlBinding>,
}

impl EventEmitter<ProbeEvent> for ProjectionProbe {}

fn bind_total_probe(
    form: &Entity<Form<AdapterInput>>,
    window: &mut Window,
    cx: &mut Context<AdapterHarness>,
) -> Entity<ProjectionProbe> {
    let initial = AdapterInput::NAME.get(form, cx);
    let probe = cx.new(|_| ProjectionProbe {
        values: vec![initial],
        retired: 0,
        subscriptions: Vec::new(),
        _binding: None,
    });
    let (binding, writer) = AdapterInput::NAME.bind_control_in(
        form,
        &probe,
        |probe, projection, _window, _cx| match projection {
            ControlProjection::Value(value) => probe.values.push(value),
            ControlProjection::Retired => probe.retired += 1,
        },
        window,
        cx,
    );
    let subscription = cx.subscribe_in(
        &probe,
        window,
        move |_, _, event: &ProbeEvent, window, cx| match event {
            ProbeEvent::Set(value) => writer.defer_set(value.clone(), window, cx),
        },
    );
    probe.update(cx, |probe, _| {
        probe.subscriptions.push(subscription);
        probe._binding = Some(binding);
    });
    probe
}

fn bind_dynamic_probe(
    form: &Entity<Form<AdapterInput>>,
    path: DynamicPath<AdapterInput, String>,
    window: &mut Window,
    cx: &mut Context<AdapterHarness>,
) -> Result<Entity<ProjectionProbe>, ResolveError> {
    let initial = path.try_get(form, cx)?;
    let probe = cx.new(|_| ProjectionProbe {
        values: vec![initial],
        retired: 0,
        subscriptions: Vec::new(),
        _binding: None,
    });
    let (binding, writer) = path.try_bind_control_in(
        form,
        &probe,
        |probe, projection, _window, _cx| match projection {
            ControlProjection::Value(value) => probe.values.push(value),
            ControlProjection::Retired => probe.retired += 1,
        },
        window,
        cx,
    )?;
    let subscription = cx.subscribe_in(
        &probe,
        window,
        move |_, _, event: &ProbeEvent, window, cx| match event {
            ProbeEvent::Set(value) => writer.defer_set(value.clone(), window, cx),
        },
    );
    probe.update(cx, |probe, _| {
        probe.subscriptions.push(subscription);
        probe._binding = Some(binding);
    });
    Ok(probe)
}

impl AdapterHarness {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| {
            Form::new(AdapterInput {
                name: "initial".to_string(),
                budget: 1024,
                model: Some("beta".to_string()),
                tools: vec!["beta".to_string()],
                rows: vec![
                    Row {
                        value: "first row".to_string(),
                    },
                    Row {
                        value: "second row".to_string(),
                    },
                ],
                details: Some(Details {
                    note: "note".to_string(),
                    budget: 7,
                }),
            })
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
    cx.update(|_, cx| assert_eq!(AdapterInput::NAME.get(&form, cx), "user"));

    cx.update(|_, cx| AdapterInput::NAME.set(&form, "external".to_string(), cx));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "external"));
}

#[gpui::test]
fn textarea_preserves_newlines_and_blur_validation(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, control) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            (
                root.form.clone(),
                FormTextarea::new(
                    &root.form,
                    AdapterInput::NAME,
                    TextareaState::new,
                    window,
                    cx,
                ),
            )
        })
    });
    let textarea = (*control).clone();

    cx.update(|window, cx| {
        textarea.update(cx, |textarea, cx| {
            textarea.set_value("first line\nsecond line", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(AdapterInput::NAME.get(&form, cx), "first line\nsecond line");
        AdapterInput::NAME.set(&form, String::new(), cx);
    });
    cx.run_until_parked();
    cx.update(|_, cx| textarea.update(cx, |_, cx| cx.emit(InputEvent::Blur)));
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!AdapterInput::NAME.errors(&form, cx).is_empty()));
}

#[gpui::test]
fn editor_highlighter_changes_keep_form_binding_active(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, control) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            (
                root.form.clone(),
                FormEditor::new(
                    &root.form,
                    AdapterInput::NAME,
                    |window, cx| EditorState::new(window, cx).language("rust"),
                    window,
                    cx,
                ),
            )
        })
    });
    let editor = (*control).clone();

    cx.update(|window, cx| {
        editor.update(cx, |editor, cx| {
            editor.set_highlighter("json", cx);
            editor.set_value("{\n  \"value\": true\n}", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(AdapterInput::NAME.get(&form, cx), "{\n  \"value\": true\n}");
        AdapterInput::NAME.set(&form, "external editor value".to_string(), cx);
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(editor.read(cx).value().as_ref(), "external editor value"));
}

#[gpui::test]
fn total_input_projects_replace_reset_and_rebase(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);

    cx.update(|_, cx| {
        let mut replacement = AdapterInput::ROOT.get(&form, cx);
        replacement.name = "replaced".to_string();
        form.update(cx, |form, cx| form.replace(replacement, cx));
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "replaced"));

    cx.update(|_, cx| form.update(cx, |form, cx| form.reset(cx)));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "initial"));

    cx.update(|_, cx| {
        let mut canonical = AdapterInput::ROOT.get(&form, cx);
        canonical.name = "rebased".to_string();
        form.update(cx, |form, cx| form.rebase(canonical, cx));
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "rebased"));
}

#[gpui::test]
fn total_input_discards_stale_projection_after_a_new_native_edit(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);

    cx.update(|window, cx| {
        AdapterInput::NAME.set(&form, "one".to_string(), cx);
        AdapterInput::NAME.set(&form, "two".to_string(), cx);
        input.update(cx, |input, cx| {
            input.set_value("editor".to_string(), window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(AdapterInput::NAME.get(&form, cx), "editor");
        assert_eq!(input.read(cx).value().as_ref(), "editor");
    });
}

#[gpui::test]
fn item_adapter_ignores_append_and_reorder_of_its_collection(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, dynamic_input) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let rows = AdapterInput::ROOT.then(AdapterInput::ROWS);
            let first = rows.items(&root.form, cx).remove(0);
            let control = FormInput::try_new(
                &root.form,
                first.then(Row::VALUE),
                InputState::new,
                window,
                cx,
            )
            .expect("bind item input");
            (root.form.clone(), control)
        })
    });
    let state = (*dynamic_input).clone();

    cx.update(|window, cx| {
        state.update(cx, |input, cx| {
            input.set_value("local editor".to_string(), window, cx)
        });
        let rows = AdapterInput::ROOT.then(AdapterInput::ROWS);
        rows.append(
            &form,
            Row {
                value: "appended".to_string(),
            },
            cx,
        )
        .expect("append row");
        let items = rows.items(&form, cx);
        rows.move_before(&form, &items[0], &items[1], cx)
            .expect("reorder rows");
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(state.read(cx).value().as_ref(), "local editor");
        assert_eq!(
            AdapterInput::ROOT
                .then(AdapterInput::ROWS)
                .items(&form, cx)
                .len(),
            3
        );
    });
}

#[gpui::test]
fn custom_adapter_suppresses_its_echo_and_projects_to_peer(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, first, second) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let first = bind_total_probe(&root.form, window, cx);
            let second = bind_total_probe(&root.form, window, cx);
            (root.form.clone(), first, second)
        })
    });

    cx.update(|_, cx| {
        first.update(cx, |_probe, cx| {
            cx.emit(ProbeEvent::Set("first".to_string()))
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(AdapterInput::NAME.get(&form, cx), "first");
        assert_eq!(first.read(cx).values, ["initial"]);
        assert_eq!(second.read(cx).values, ["initial", "first"]);
    });
}

#[gpui::test]
fn custom_dynamic_adapter_receives_retired_once_and_writer_stops(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, probe) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let details = AdapterInput::ROOT
                .then(AdapterInput::DETAILS)
                .some()
                .resolve(&root.form, cx)
                .expect("resolve details")
                .expect("details are present");
            let probe = bind_dynamic_probe(&root.form, details.then(Details::NOTE), window, cx)
                .expect("bind dynamic probe");
            (root.form.clone(), probe)
        })
    });

    cx.update(|_, cx| AdapterInput::DETAILS.set(&form, None, cx));
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(probe.read(cx).retired, 1);
        probe.update(cx, |_probe, cx| {
            cx.emit(ProbeEvent::Set("stale".to_string()))
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(probe.read(cx).retired, 1);
        assert!(AdapterInput::DETAILS.get(&form, cx).is_none());
    });
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
                .some()
                .resolve(&root.form, cx)
                .expect("resolve details")
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
            AdapterInput::DETAILS
                .get(&form, cx)
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
    cx.update(|_, cx| assert_eq!(AdapterInput::NAME.get(&form, cx), "initial"));
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
    cx.update(|_, cx| assert_eq!(AdapterInput::NAME.get(&form, cx), "initial"));
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
        let mut replacement = AdapterInput::ROOT.get(&form, cx);
        form.update(cx, |form, cx| {
            replacement.name = "replacement".into();
            form.replace(replacement, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(AdapterInput::NAME.get(&form, cx), "replacement"));
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
        integer_control.update(cx, |_, cx| {
            cx.emit(IntegerInputEvent::Change(Ok(1024)));
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(AdapterInput::BUDGET.get(&form, cx), 1024);
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok());
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
                .some()
                .resolve(&root.form, cx)
                .unwrap()
                .expect("details are present");
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
    cx.update(|_, cx| assert!(AdapterInput::DETAILS.get(&form, cx).is_none()));
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
        assert_eq!(AdapterInput::MODEL.get(&form, cx).as_deref(), Some("alpha"));
        assert_eq!(AdapterInput::TOOLS.get(&form, cx), ["alpha".to_string()]);
    });
}

#[gpui::test]
fn option_refresh_reprojects_current_delegate_without_mutating_form(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, select, combobox) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| {
            (
                root.form.clone(),
                std::ops::Deref::deref(&root.select_control).clone(),
                std::ops::Deref::deref(&root.combobox_control).clone(),
            )
        })
    });

    cx.update(|window, cx| {
        let selected_model = AdapterInput::MODEL.get(&form, cx);
        select.update(cx, |state, cx| {
            state.set_items(vec!["alpha".to_string()], window, cx);
            match selected_model.as_ref() {
                Some(value) => state.set_selected_value(value, window, cx),
                None => state.set_selected_index(None, window, cx),
            }
        });

        let selected_tools = AdapterInput::TOOLS.get(&form, cx);
        combobox.update(cx, |state, cx| {
            state.set_items(vec!["alpha".to_string()], window, cx);
            state.set_selected_values(&selected_tools, window, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(AdapterInput::MODEL.get(&form, cx).as_deref(), Some("beta"));
        assert_eq!(AdapterInput::TOOLS.get(&form, cx), ["beta".to_string()]);
        assert_eq!(select.read(cx).selected_value(), None);
        assert!(combobox.read(cx).selected_values().is_empty());
    });
}

#[test]
fn integer_policy_errors_are_stable() {
    assert_eq!(
        IntegerInputPolicyError::NonPositiveStep.to_string(),
        "integer input step must be positive"
    );
}
