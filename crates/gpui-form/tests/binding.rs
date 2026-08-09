use std::cell::RefCell;

use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_form::{ControlBinding, ControlProjection, ControlWriter, Form, FormEvent, FormSchema};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Details {
    note: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Model {
    title: String,
    #[form(child)]
    details: Option<Details>,
}

fn model(title: &str) -> Model {
    Model {
        title: title.into(),
        details: Some(Details {
            note: "note".into(),
        }),
    }
}

#[derive(Default)]
struct ProjectionLog {
    values: Vec<ControlProjection<String>>,
}

struct BindingHarness {
    form: Entity<Form<Model>>,
    first: Entity<ProjectionLog>,
    second: Entity<ProjectionLog>,
    dynamic: Entity<ProjectionLog>,
    _first_binding: ControlBinding,
    _second_binding: ControlBinding,
    _dynamic_binding: ControlBinding,
    first_writer: ControlWriter<Model, String>,
    second_writer: ControlWriter<Model, String>,
}

impl BindingHarness {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| Form::new(model("initial")));
        let first = cx.new(|_| ProjectionLog::default());
        let second = cx.new(|_| ProjectionLog::default());
        let dynamic = cx.new(|_| ProjectionLog::default());

        let (first_binding, first_writer) = Model::TITLE.bind_control_in(
            &form,
            &first,
            |log, projection, _, _| log.values.push(projection),
            window,
            cx,
        );
        let (second_binding, second_writer) = Model::TITLE.bind_control_in(
            &form,
            &second,
            |log, projection, _, _| log.values.push(projection),
            window,
            cx,
        );
        let details = Model::ROOT
            .then(Model::DETAILS)
            .some()
            .resolve(&form, cx)
            .expect("initial optional path resolves")
            .expect("initial details are present");
        let (dynamic_binding, _) = details
            .then(Details::NOTE)
            .try_bind_control_in(
                &form,
                &dynamic,
                |log, projection, _, _| log.values.push(projection),
                window,
                cx,
            )
            .expect("initial dynamic binding resolves");

        Self {
            form,
            first,
            second,
            dynamic,
            _first_binding: first_binding,
            _second_binding: second_binding,
            _dynamic_binding: dynamic_binding,
            first_writer,
            second_writer,
        }
    }
}

impl Render for BindingHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn open_harness(cx: &mut TestAppContext) -> WindowHandle<BindingHarness> {
    cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| BindingHarness::new(window, cx))
        })
        .expect("open binding test window")
    })
}

fn clear(log: &Entity<ProjectionLog>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| log.update(cx, |log, _| log.values.clear()));
}

fn values(
    log: &Entity<ProjectionLog>,
    cx: &mut VisualTestContext,
) -> Vec<ControlProjection<String>> {
    cx.update(|_, cx| log.read(cx).values.clone())
}

#[gpui::test]
fn source_write_is_suppressed_while_another_binding_receives_the_value(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let (form, first, second, writer) = cx.update(|_window, cx| {
        root.read_with(cx, |root, _| {
            (
                root.form.clone(),
                root.first.clone(),
                root.second.clone(),
                root.first_writer.clone(),
            )
        })
    });
    clear(&first, &mut cx);
    clear(&second, &mut cx);

    cx.update(|window, cx| {
        root.update(cx, |_, cx| writer.defer_set("written".into(), window, cx));
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(Model::TITLE.get(&form, cx), "written"));
    assert!(values(&first, &mut cx).is_empty());
    assert_eq!(
        values(&second, &mut cx),
        [ControlProjection::Value("written".into())]
    );
}

#[gpui::test]
fn equal_writer_set_clears_its_issue_without_a_model_event(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let (form, writer) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| (root.form.clone(), root.first_writer.clone()))
    });
    let events = std::rc::Rc::new(RefCell::new(0));
    let observed = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&form, move |_, event: &FormEvent<Model>, _| {
            if matches!(event, FormEvent::ModelChanged(_)) {
                *observed.borrow_mut() += 1;
            }
        })
        .detach();
    });

    cx.update(|window, cx| {
        root.update(cx, |_, cx| {
            writer.defer_set_issue(
                "editor",
                gpui_form::ValidationMessage::literal("incomplete"),
                window,
                cx,
            );
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!form.read(cx).is_valid()));

    cx.update(|window, cx| {
        root.update(cx, |_, cx| writer.defer_set("initial".into(), window, cx));
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(form.read(cx).is_valid()));
    assert_eq!(*events.borrow(), 0);
}

#[gpui::test]
fn mailbox_keeps_the_latest_external_value_and_never_overwrites_a_new_editor_write(
    cx: &mut TestAppContext,
) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let (form, first, writer) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| {
            (
                root.form.clone(),
                root.first.clone(),
                root.first_writer.clone(),
            )
        })
    });
    clear(&first, &mut cx);

    cx.update(|_, cx| {
        assert!(Model::TITLE.set(&form, "one".into(), cx));
        assert!(Model::TITLE.set(&form, "two".into(), cx));
    });
    cx.run_until_parked();
    assert_eq!(
        values(&first, &mut cx),
        [ControlProjection::Value("two".into())]
    );
    clear(&first, &mut cx);

    cx.update(|window, cx| {
        root.update(cx, |_, cx| writer.defer_set("editor".into(), window, cx));
        assert!(Model::TITLE.set(&form, "newer-external".into(), cx));
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(Model::TITLE.get(&form, cx), "editor"));
    assert!(values(&first, &mut cx).is_empty());
}

#[gpui::test]
fn dynamic_retirement_projects_once_and_total_binding_survives_whole_model_lifecycle(
    cx: &mut TestAppContext,
) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let (form, first, dynamic) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| {
            (root.form.clone(), root.first.clone(), root.dynamic.clone())
        })
    });
    clear(&first, &mut cx);
    clear(&dynamic, &mut cx);

    cx.update(|_, cx| {
        assert!(Model::DETAILS.set(&form, None, cx));
    });
    cx.run_until_parked();
    assert_eq!(values(&dynamic, &mut cx), [ControlProjection::Retired]);

    cx.update(|_, cx| {
        form.update(cx, |form, cx| form.replace(model("replaced"), cx));
        form.update(cx, |form, cx| form.reset(cx));
        form.update(cx, |form, cx| form.rebase(model("rebased"), cx));
    });
    cx.run_until_parked();
    assert_eq!(values(&dynamic, &mut cx), [ControlProjection::Retired]);
    assert_eq!(
        values(&first, &mut cx),
        [ControlProjection::Value("rebased".into())]
    );
}

#[gpui::test]
fn invalid_editor_sequence_suppresses_an_older_queued_authoritative_value(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let (form, first, writer) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| {
            (
                root.form.clone(),
                root.first.clone(),
                root.first_writer.clone(),
            )
        })
    });
    clear(&first, &mut cx);

    cx.update(|window, cx| {
        assert!(Model::TITLE.set(&form, "authoritative".into(), cx));
        root.update(cx, |_, cx| {
            writer.defer_set_issue(
                "invalid-editor-text",
                gpui_form::ValidationMessage::literal("incomplete"),
                window,
                cx,
            );
        });
    });
    cx.run_until_parked();

    assert!(values(&first, &mut cx).is_empty());
    cx.update(|_, cx| {
        assert_eq!(Model::TITLE.get(&form, cx), "authoritative");
        let report = form.read(cx).validation_report();
        assert_eq!(report.issues().len(), 1);
        assert_eq!(report.issues()[0].code(), "invalid-editor-text");
    });
}

#[gpui::test]
fn control_write_clears_peer_issue_in_its_model_transaction(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let (form, first_writer, second_writer) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| {
            (
                root.form.clone(),
                root.first_writer.clone(),
                root.second_writer.clone(),
            )
        })
    });

    cx.update(|window, cx| {
        root.update(cx, |_, cx| {
            second_writer.defer_set_issue(
                "peer-invalid",
                gpui_form::ValidationMessage::literal("peer invalid"),
                window,
                cx,
            );
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!form.read(cx).is_valid()));

    let model_events = std::rc::Rc::new(std::cell::Cell::new(0));
    let validation_events = std::rc::Rc::new(std::cell::Cell::new(0));
    cx.update(|_, cx| {
        let model_events = model_events.clone();
        let validation_events = validation_events.clone();
        cx.subscribe(&form, move |_, event: &FormEvent<Model>, _| match event {
            FormEvent::ModelChanged(_) => model_events.set(model_events.get() + 1),
            FormEvent::ValidationChanged { .. } => {
                validation_events.set(validation_events.get() + 1)
            }
        })
        .detach();
    });

    cx.update(|window, cx| {
        root.update(cx, |_, cx| {
            first_writer.defer_set("accepted".into(), window, cx);
        });
    });
    cx.run_until_parked();

    cx.update(|_, cx| {
        assert_eq!(Model::TITLE.get(&form, cx), "accepted");
        assert!(form.read(cx).is_valid());
        assert!(form.read(cx).validation_report().issues().is_empty());
    });
    assert_eq!(model_events.get(), 1);
    assert_eq!(validation_events.get(), 0);
}

#[gpui::test]
fn dropped_binding_makes_queued_and_future_writer_work_no_op(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let form = cx.update(|_, cx| root.read_with(cx, |root, _| root.form.clone()));
    let owner = cx.update(|_, cx| cx.new(|_| ProjectionLog::default()));
    let (binding, writer) = cx.update(|window, cx| {
        Model::TITLE.bind_control_in(
            &form,
            &owner,
            |log, projection, _, _| log.values.push(projection),
            window,
            cx,
        )
    });
    clear(&owner, &mut cx);

    cx.update(|window, cx| {
        root.update(cx, |_, cx| {
            writer.defer_set_issue(
                "temporary-invalid",
                gpui_form::ValidationMessage::literal("temporary invalid"),
                window,
                cx,
            );
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(!form.read(cx).is_valid()));

    cx.update(|window, cx| {
        assert!(Model::TITLE.set(&form, "queued".into(), cx));
        drop(binding);
        root.update(cx, |_, cx| writer.defer_set("ignored".into(), window, cx));
    });
    cx.run_until_parked();

    cx.update(|_, cx| {
        assert_eq!(Model::TITLE.get(&form, cx), "queued");
        assert!(form.read(cx).is_valid());
        assert!(form.read(cx).validation_report().issues().is_empty());
    });
    assert!(values(&owner, &mut cx).is_empty());
}

struct FormDropHarness {
    log: Entity<ProjectionLog>,
    _binding: ControlBinding,
    writer: ControlWriter<Model, String>,
}

impl FormDropHarness {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|_| Form::new(model("temporary")));
        let log = cx.new(|_| ProjectionLog::default());
        let (binding, writer) = Model::TITLE.bind_control_in(
            &form,
            &log,
            |log, projection, _, _| log.values.push(projection),
            window,
            cx,
        );
        Self {
            log,
            _binding: binding,
            writer,
        }
    }
}

impl Render for FormDropHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
fn writer_is_no_op_after_form_drop(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| FormDropHarness::new(window, cx))
        })
        .expect("open form-drop test window")
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("form-drop harness root");
    let (log, writer) =
        cx.update(|_, cx| root.read_with(cx, |root, _| (root.log.clone(), root.writer.clone())));

    cx.update(|window, cx| {
        root.update(cx, |_, cx| writer.defer_set("ignored".into(), window, cx));
    });
    cx.run_until_parked();

    assert!(values(&log, &mut cx).is_empty());
}

#[gpui::test]
fn public_debug_output_omits_schema_addresses_and_private_control_identity(
    cx: &mut TestAppContext,
) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("binding harness root");
    let (form, writer) = cx.update(|_, cx| {
        root.read_with(cx, |root, _| (root.form.clone(), root.first_writer.clone()))
    });
    let model_debug = std::rc::Rc::new(RefCell::new(None));
    cx.update(|_, cx| {
        let model_debug = model_debug.clone();
        cx.subscribe(&form, move |_, event: &FormEvent<Model>, _| {
            if let FormEvent::ModelChanged(change) = event {
                *model_debug.borrow_mut() = Some(format!("{change:?}"));
            }
        })
        .detach();
    });

    cx.update(|window, cx| {
        root.update(cx, |_, cx| writer.defer_set("changed".into(), window, cx));
    });
    cx.run_until_parked();
    let model_debug = model_debug
        .borrow()
        .clone()
        .expect("writer change was published");

    cx.update(|window, cx| {
        root.update(cx, |_, cx| {
            writer.defer_set_issue(
                "private-control-issue",
                gpui_form::ValidationMessage::literal("invalid"),
                window,
                cx,
            );
        });
    });
    cx.run_until_parked();
    let issue_debug =
        cx.update(|_, cx| format!("{:?}", form.read(cx).validation_report().issues()[0]));

    for debug in [&model_debug, &issue_debug] {
        assert!(!debug.contains("title"));
        assert!(!debug.contains("details"));
        assert!(!debug.contains("Case"));
        assert!(!debug.contains("CanonicalAddress"));
        assert!(!debug.contains("ControlOrigin"));
        assert!(!debug.contains("control_id"));
        assert!(!debug.contains("generation"));
        assert!(!debug.contains("editor_sequence"));
    }
}
