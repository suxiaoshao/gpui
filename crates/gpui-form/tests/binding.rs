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
            writer.defer_draft(
                "-".to_owned(),
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
        assert!(Model::TITLE.set(&form, "one".into(), cx).unwrap());
        assert!(Model::TITLE.set(&form, "two".into(), cx).unwrap());
    });
    cx.run_until_parked();
    assert_eq!(
        values(&first, &mut cx),
        [ControlProjection::Value("two".into())]
    );
    clear(&first, &mut cx);

    cx.update(|window, cx| {
        root.update(cx, |_, cx| writer.defer_set("editor".into(), window, cx));
        assert!(
            Model::TITLE
                .set(&form, "newer-external".into(), cx)
                .unwrap()
        );
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
        assert!(Model::DETAILS.set(&form, None, cx).unwrap());
    });
    cx.run_until_parked();
    assert_eq!(values(&dynamic, &mut cx), [ControlProjection::Retired]);

    cx.update(|_, cx| {
        form.update(cx, |form, cx| {
            form.replace(model("replaced"), gpui_form::DraftPolicy::Preserve, cx)
                .unwrap()
        });
        form.update(cx, |form, cx| {
            form.reset(gpui_form::DraftPolicy::Discard, cx).unwrap()
        });
        form.update(cx, |form, cx| {
            form.rebase(model("rebased"), gpui_form::DraftPolicy::Preserve, cx)
                .unwrap()
        });
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
        assert!(Model::TITLE.set(&form, "authoritative".into(), cx).unwrap());
        root.update(cx, |_, cx| {
            writer.defer_draft(
                "-".to_owned(),
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
fn control_write_preserves_peer_draft_in_its_model_transaction(cx: &mut TestAppContext) {
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
            second_writer.defer_draft(
                "-".to_owned(),
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
            FormEvent::DraftChanged(_) | FormEvent::SessionChanged(_) => {}
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
        assert!(!form.read(cx).is_valid());
        assert_eq!(form.read(cx).validation_report().issues().len(), 1);
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
            writer.defer_draft(
                "-".to_owned(),
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
        assert!(Model::TITLE.set(&form, "queued".into(), cx).unwrap());
        drop(binding);
        root.update(cx, |_, cx| writer.defer_set("ignored".into(), window, cx));
    });
    cx.run_until_parked();

    cx.update(|_, cx| {
        assert_eq!(Model::TITLE.get(&form, cx), "queued");
        assert!(!form.read(cx).is_valid());
        assert_eq!(form.read(cx).validation_report().issues().len(), 1);
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
            writer.defer_draft(
                "-".to_owned(),
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

#[gpui::test]
fn native_ticket_blocks_prepare_and_invalidates_saved_version_before_defer(
    cx: &mut TestAppContext,
) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    cx.update(|window, cx| {
        let (form, writer) =
            root.read_with(cx, |root, _| (root.form.clone(), root.first_writer.clone()));
        let version = form
            .update(cx, |form, cx| form.prepare(cx))
            .unwrap()
            .version();
        root.update(cx, |_, cx| writer.defer_set("initial".into(), window, cx));
        assert!(matches!(
            form.update(cx, |form, cx| form.prepare(cx)),
            Err(gpui_form::PrepareError::EditsPending)
        ));
        assert!(!form.update(cx, |form, cx| form.rebase_if_current(
            version,
            model("stale"),
            cx
        )));
        form.update(cx, |f, cx| f.discard_drafts(Model::TITLE, cx))
            .unwrap();
        root.update(cx, |_, cx| {
            writer.defer_set("discarded before admission".into(), window, cx)
        });
        let revision = form.read(cx).revision();
        form.update(cx, |f, cx| f.discard_drafts(Model::TITLE, cx))
            .unwrap();
        assert!(form.read(cx).revision() > revision);
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let form = root.read(cx).form.clone();
        assert_eq!(Model::TITLE.get(&form, cx), "initial");
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok());
    });
}

#[gpui::test]
fn drafts_survive_unmount_and_each_editor_owns_its_text(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let (form, binding, writer) = cx.update(|window, cx| {
        let form = root.read(cx).form.clone();
        let owner = root.read(cx).first.clone();
        let (binding, writer) =
            Model::TITLE.bind_control_in(&form, &owner, |_, _, _, _| {}, window, cx);
        let writer = writer.with_editor_key("offline", cx);
        root.update(cx, |_, cx| {
            writer.defer_draft("-".to_owned(), "syntax", "bad".into(), window, cx)
        });
        (form, binding, writer)
    });
    cx.run_until_parked();
    let revision = cx.update(|_, cx| form.read(cx).revision());
    cx.update(|window, cx| {
        root.update(cx, |_, cx| {
            writer.defer_draft("--".to_owned(), "syntax", "bad".into(), window, cx)
        })
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(form.read(cx).revision() > revision));
    drop(binding);
    cx.update(|_, cx| {
        assert!(form.read(cx).is_dirty());
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        Model::TITLE.set(&form, "programmatic".into(), cx).unwrap();
    });
    let (_binding, restored) = cx.update(|window, cx| {
        let owner = root.read(cx).first.clone();
        let (binding, writer) =
            Model::TITLE.bind_control_in(&form, &owner, |_, _, _, _| {}, window, cx);
        (binding, writer.with_editor_key("offline", cx))
    });
    cx.update(|window, cx| {
        assert_eq!(restored.draft::<String>(cx).unwrap(), Some("--".into()));
        let other = root.read(cx).second_writer.clone();
        root.update(cx, |_, cx| {
            other.defer_draft("other".to_owned(), "syntax", "bad".into(), window, cx)
        });
        root.update(cx, |_, cx| restored.defer_set("initial".into(), window, cx));
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(restored.draft::<String>(cx).unwrap().is_none());
        assert!(
            form.read(cx).is_dirty(),
            "other editor's draft still blocks submission"
        );
        form.update(cx, |form, cx| {
            form.reset(gpui_form::DraftPolicy::Discard, cx)
        })
        .unwrap();
        assert!(!form.read(cx).is_dirty());
    });
}

#[gpui::test]
fn preserving_lifecycle_refuses_to_retire_a_dynamic_draft_atomically(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let (form, note, _binding, writer) = cx.update(|window, cx| {
        let form = root.read(cx).form.clone();
        let note = Model::ROOT
            .then(Model::DETAILS)
            .some()
            .resolve(&form, cx)
            .unwrap()
            .unwrap()
            .then(Details::NOTE);
        let owner = root.read(cx).dynamic.clone();
        let (binding, writer) = note
            .try_bind_control_in(&form, &owner, |_, _, _, _| {}, window, cx)
            .unwrap();
        root.update(cx, |_, cx| {
            writer.defer_draft("draft".to_owned(), "syntax", "bad".into(), window, cx)
        });
        (form, note, binding, writer)
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let revision = form.read(cx).revision();
        assert!(matches!(
            form.update(cx, |form, cx| form.rebase(
                model("external"),
                gpui_form::DraftPolicy::Preserve,
                cx
            )),
            Err(gpui_form::LifecycleError::WouldRetireDrafts(_))
        ));
        assert_eq!(form.read(cx).revision(), revision);
        assert_eq!(Model::TITLE.baseline(&form, cx), "initial");
        assert!(Model::DETAILS.set(&form, None, cx).is_err());
        assert_eq!(note.try_get(&form, cx).unwrap(), "note");
        assert_eq!(writer.draft::<String>(cx).unwrap(), Some("draft".into()));
        form.update(cx, |form, cx| form.discard_drafts(note.clone(), cx))
            .unwrap();
        Model::DETAILS.set(&form, None, cx).unwrap();
        assert!(note.try_get(&form, cx).is_err());
    });
}

#[derive(Clone, Debug, PartialEq)]
enum Slot {
    Unset,
    Set(i32),
    Unreadable(String),
}
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct MappedModel {
    slot: Slot,
}
struct SlotMapping;
impl gpui_form::ControlMapping<Slot, String> for SlotMapping {
    type Context = i32;
    fn format(&self, stored: &Slot, inherited: &i32) -> String {
        match stored {
            Slot::Unset => inherited.to_string(),
            Slot::Set(n) => n.to_string(),
            Slot::Unreadable(s) => s.clone(),
        }
    }
    fn parse(
        &self,
        display: String,
        stored: &Slot,
        baseline: Option<&Slot>,
        inherited: &i32,
    ) -> gpui_form::EditResult<Slot, String> {
        if let Some(baseline) = baseline.filter(|b| self.format(b, inherited) == display) {
            return gpui_form::EditResult::Parsed(baseline.clone());
        }
        if self.format(stored, inherited) == display {
            return gpui_form::EditResult::Parsed(stored.clone());
        }
        match display.parse() {
            Ok(n) => gpui_form::EditResult::Parsed(Slot::Set(n)),
            Err(_) => gpui_form::EditResult::draft(display, "syntax", "number required"),
        }
    }
}

#[gpui::test]
fn mapping_keeps_domain_slots_drafts_and_captured_context_separate(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let (form, binding, writer, log) = cx.update(|window, cx| {
        let form = cx.new(|_| Form::new(MappedModel { slot: Slot::Unset }));
        let log = cx.new(|_| ProjectionLog::default());
        let (binding, writer) = gpui_form::bind_mapped_control_in(
            &form,
            MappedModel::SLOT,
            "slot",
            SlotMapping,
            10,
            &log,
            |log, value, _, _| log.values.push(value),
            window,
            cx,
        )
        .unwrap();
        assert_eq!(
            log.read(cx).values,
            vec![ControlProjection::Value("10".into())]
        );
        (form, binding, writer, log)
    });
    cx.update(|window, cx| root.update(cx, |_, cx| writer.defer_edit("22".into(), window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(MappedModel::SLOT.get(&form, cx), Slot::Set(22));
        root.update(cx, |_, cx| writer.defer_edit("10".into(), window, cx));
        binding.refresh(20, cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            MappedModel::SLOT.get(&form, cx),
            Slot::Unset,
            "queued input used the context at native event entry"
        );
        assert!(!form.read(cx).is_dirty());
        root.update(cx, |_, cx| writer.defer_edit("--".into(), window, cx));
    });
    cx.run_until_parked();
    let count = cx.update(|_, cx| log.read(cx).values.len());
    cx.update(|_, cx| binding.refresh(30, cx));
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            log.read(cx).values.len(),
            count,
            "refresh must not overwrite native draft text"
        );
        let report = form
            .update(cx, |f, cx| {
                f.rebase_preserving_edits(
                    MappedModel {
                        slot: Slot::Set(40),
                    },
                    cx,
                )
            })
            .unwrap();
        assert_eq!(
            report.conflicts().len(),
            1,
            "draft plus external change conflicts even though stored value equals old baseline"
        );
        assert_eq!(MappedModel::SLOT.get(&form, cx), Slot::Unset);
        assert!(form.update(cx, |f, cx| f.prepare(cx)).is_err());
    });
    cx.update(|_, cx| {
        form.update(cx, |f, cx| {
            f.rebase(
                MappedModel {
                    slot: Slot::Unreadable("unknown syntax".into()),
                },
                gpui_form::DraftPolicy::Discard,
                cx,
            )
        })
        .unwrap()
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            log.read(cx).values.last(),
            Some(&ControlProjection::Value("unknown syntax".into()))
        )
    });
}

#[gpui::test]
fn closed_window_settles_an_unadmitted_native_ticket(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut visual).unwrap();
    let form = visual.update(|window, cx| {
        let (form, writer) =
            root.read_with(cx, |root, _| (root.form.clone(), root.first_writer.clone()));
        root.update(cx, |_, cx| {
            writer.defer_set("never accepted".into(), window, cx)
        });
        assert!(form.update(cx, |f, cx| f.prepare(cx)).is_err());
        window.remove_window();
        form
    });
    visual.run_until_parked();
    drop(visual);
    cx.update(|cx| {
        assert_eq!(Model::TITLE.get(&form, cx), "initial");
        assert!(form.update(cx, |f, cx| f.prepare(cx)).is_ok());
    });
}

#[gpui::test]
fn retired_callback_cannot_invalidate_a_fresh_prepared_version_before_notification(
    cx: &mut TestAppContext,
) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    cx.update(|window, cx| {
        let form = root.read(cx).form.clone();
        let note = Model::ROOT
            .then(Model::DETAILS)
            .some()
            .resolve(&form, cx)
            .unwrap()
            .unwrap()
            .then(Details::NOTE);
        let owner = root.read(cx).dynamic.clone();
        let (_binding, writer) = note
            .try_bind_control_in(&form, &owner, |_, _, _, _| {}, window, cx)
            .unwrap();
        Model::DETAILS.set(&form, None, cx).unwrap();
        let prepared = form.update(cx, |f, cx| f.prepare(cx)).unwrap();
        root.update(cx, |_, cx| writer.defer_set("retired".into(), window, cx));
        assert!(form.update(cx, |f, cx| f.rebase_if_current(
            prepared.version(),
            prepared.value().clone(),
            cx
        )));
    });
}
