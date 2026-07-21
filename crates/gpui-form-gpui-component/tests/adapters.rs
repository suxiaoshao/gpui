use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_component::{
    combobox::{ComboboxEvent, ComboboxState},
    input::{InputEvent, InputState},
    select::{SelectEvent, SelectState},
};
use gpui_form::FormStore as _;
use gpui_form_gpui_component::{
    FormCombobox, FormInput, FormIntegerInput, FormSelect, IntegerInputState,
};

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = AdapterFormStore)]
struct AdapterInput {
    #[form(required, validate(on_blur, on_submit))]
    name: String,
    budget: u32,
    model: Option<String>,
    tools: Vec<String>,
}

struct AdapterHarness {
    form: Entity<AdapterFormStore>,
    input: Entity<InputState>,
    control: Option<FormInput>,
    integer_input: Entity<InputState>,
    integer_control: Option<FormIntegerInput<u32>>,
    select_control: FormSelect<Vec<String>>,
    combobox_control: FormCombobox<Vec<String>>,
}

impl AdapterHarness {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|cx| {
            AdapterFormStore::from_value(
                AdapterInput {
                    name: "initial".to_string(),
                    budget: 1024,
                    model: Some("beta".to_string()),
                    tools: vec!["beta".to_string()],
                },
                cx,
            )
        });
        let control = FormInput::new(
            AdapterFormStore::name_field(&form),
            InputState::new,
            window,
            cx,
        )
        .expect("bind input");
        let input = (*control).clone();
        let integer_control = FormIntegerInput::new(
            AdapterFormStore::budget_field(&form),
            |window, cx| IntegerInputState::new(window, cx).min(1).max(4096).step(1),
            window,
            cx,
        )
        .expect("bind integer input");
        let integer_input = integer_control.read(cx).editor().clone();
        let options = vec!["alpha".to_string(), "beta".to_string()];
        let select_control = FormSelect::new(
            AdapterFormStore::model_field(&form),
            {
                let options = options.clone();
                move |window, cx| SelectState::new(options, None, window, cx)
            },
            window,
            cx,
        )
        .expect("bind select");
        let combobox_control = FormCombobox::new(
            AdapterFormStore::tools_field(&form),
            move |window, cx| ComboboxState::new(options, Vec::new(), window, cx).multiple(true),
            window,
            cx,
        )
        .expect("bind combobox");

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
) -> (Entity<AdapterFormStore>, Entity<InputState>) {
    cx.update(|_, cx| root.read_with(cx, |root, _| (root.form.clone(), root.input.clone())))
}

#[gpui::test]
fn input_adapter_mirrors_form_and_component_without_reentrant_update(cx: &mut TestAppContext) {
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

    cx.update(|_, cx| {
        AdapterFormStore::name_field(&form)
            .set("external".to_string(), cx)
            .expect("form is alive");
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "external"));
}

#[gpui::test]
fn two_inputs_share_one_typed_field_without_echo(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, first_input) = entities(&root, &mut cx);
    let (second_control, second_input) = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let control = FormInput::new(
                AdapterFormStore::name_field(&root.form),
                InputState::new,
                window,
                cx,
            )
            .expect("bind second input");
            let input = (*control).clone();
            (control, input)
        })
    });

    cx.update(|window, cx| {
        first_input.update(cx, |input, cx| {
            input.set_value("shared", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(form.read(cx).value().name, "shared");
        assert_eq!(second_input.read(cx).value().as_ref(), "shared");
    });
    drop(second_control);
}

#[gpui::test]
fn input_blur_runs_field_validation(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);

    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            cx.emit(InputEvent::Change);
            cx.emit(InputEvent::Blur);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(form.read(cx).value().name, "");
        assert!(
            !AdapterFormStore::name_field(&form)
                .errors(cx)
                .unwrap()
                .is_empty()
        );
    });
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
fn incomplete_integer_text_preserves_typed_value_and_issue_dies_with_control(
    cx: &mut TestAppContext,
) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| {
            (root.form.clone(), root.integer_input.clone())
        })
    });
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("-", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(form.read(cx).value().budget, 1024);
        assert!(!form.read(cx).is_valid());
        root.update(cx, |root, _| root.integer_control = None);
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(form.read(cx).is_valid()));
}

#[gpui::test]
fn select_and_combobox_bind_values_and_use_current_items(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.select_control.update(cx, |select, cx| {
                select.set_items(vec!["beta".into(), "alpha".into()], window, cx);
                select.set_selected_value(&"beta".to_string(), window, cx);
                cx.emit(SelectEvent::Confirm(Some("alpha".to_string())));
            });
            root.combobox_control.update(cx, |combobox, cx| {
                combobox.set_items(vec!["beta".into(), "alpha".into()], window, cx);
                combobox.set_selected_values(&["beta".to_string()], window, cx);
                cx.emit(ComboboxEvent::Change(vec!["alpha".to_string()]));
            });
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let root = root.read(cx);
        assert_eq!(root.form.read(cx).value().model.as_deref(), Some("alpha"));
        assert_eq!(root.form.read(cx).value().tools, ["alpha"]);
    });
}
