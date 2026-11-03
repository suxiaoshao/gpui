use gpui::{AppContext as _, TestAppContext};
use gpui_form::{Form, FormSchema, ValidationMessage, ValidationRule, ValidationTriggers};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Leaf {
    a: i32,
    b: i32,
}
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Model {
    #[form(child)]
    optional: Option<Leaf>,
    #[form(child)]
    child: Leaf,
    #[form(items)]
    rows: Vec<Leaf>,
}
fn model(a: i32, b: i32) -> Model {
    Model {
        optional: Some(Leaf { a: 1, b: 2 }),
        child: Leaf { a, b },
        rows: vec![Leaf { a: 1, b: 2 }],
    }
}

#[gpui::test]
fn three_way_rebase_merges_leaves_and_keeps_conflicting_local_values(cx: &mut TestAppContext) {
    cx.update(|cx| {
        for (base, local, remote, expected, conflict) in [
            (1, 1, 2, 2, false),
            (1, 2, 1, 2, false),
            (1, 2, 2, 2, false),
            (1, 2, 3, 2, true),
        ] {
            let form = cx.new(|_| Form::new(model(base, 1)));
            Model::ROOT
                .then(Model::CHILD)
                .then(Leaf::A)
                .set(&form, local, cx)
                .unwrap();
            let report = form
                .update(cx, |form, cx| {
                    form.rebase_preserving_edits(model(remote, 9), cx)
                })
                .unwrap();
            let a = Model::ROOT.then(Model::CHILD).then(Leaf::A);
            let b = Model::ROOT.then(Model::CHILD).then(Leaf::B);
            assert_eq!(a.get(&form, cx), expected);
            assert_eq!(a.baseline(&form, cx), remote);
            assert_eq!(a.is_dirty(&form, cx), expected != remote);
            assert_eq!(b.get(&form, cx), 9);
            assert!(!b.is_dirty(&form, cx));
            assert_eq!(!report.conflicts().is_empty(), conflict);
            assert!(form.read(cx).is_current(report.version()));
        }
    });
}

#[gpui::test]
fn collection_baseline_tracks_occurrences_and_parent_deletions(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::new(model(1, 2)));
        let rows = Model::ROOT.then(Model::ROWS);
        let original = rows.items(&form, cx)[0].clone();
        let added = rows.append(&form, Leaf { a: 3, b: 4 }, cx).unwrap();
        assert!(added.try_baseline(&form, cx).unwrap().is_none());
        rows.move_before(&form, &added, &original, cx).unwrap();
        assert_eq!(
            original.try_baseline(&form, cx).unwrap(),
            Some(Leaf { a: 1, b: 2 })
        );
        rows.remove(&form, original.clone(), cx).unwrap();
        assert!(rows.is_dirty(&form, cx));
        assert!(original.try_baseline(&form, cx).is_err());
        let mut remote = model(1, 2);
        remote.rows[0].a = 8;
        let report = form
            .update(cx, |form, cx| form.rebase_preserving_edits(remote, cx))
            .unwrap();
        assert_eq!(report.conflicts().len(), 1);
        assert_eq!(added.try_get(&form, cx).unwrap(), Leaf { a: 3, b: 4 });
        assert!(
            added.try_baseline(&form, cx).unwrap().is_none(),
            "unrelated external occurrences must not be matched by index"
        );
        let later = rows.append(&form, Leaf { a: 99, b: 100 }, cx).unwrap();
        assert!(
            later.try_baseline(&form, cx).unwrap().is_none(),
            "later local insertion must not acquire a remote-only baseline occurrence"
        );
    });
}

#[gpui::test]
fn rules_run_once_per_transaction_and_empty_results_clear_only_their_owner(
    cx: &mut TestAppContext,
) {
    use std::{cell::Cell, rc::Rc};
    cx.update(|cx| {
        let runs = Rc::new(Cell::new(0));
        let counter = runs.clone();
        let rule = ValidationRule::new(
            "range",
            move |r: gpui_form::ValidationRequest<'_, Leaf>, out| {
                counter.set(counter.get() + 1);
                if r.model().a > r.model().b {
                    out.at(Leaf::B)
                        .error("shared", ValidationMessage::literal("range"));
                }
            },
        )
        .dependency(Leaf::A)
        .dependency(Leaf::B)
        .triggers(ValidationTriggers::ALL);
        let second = ValidationRule::new("other", |_, out| {
            out.at(Leaf::B).error("shared", "other".into())
        })
        .dependency(Leaf::B)
        .triggers(ValidationTriggers::ALL);
        let form = cx.new(|_| {
            Form::new(Leaf { a: 9, b: 1 })
                .with_rules(vec![rule, second])
                .unwrap()
        });
        assert_eq!(runs.get(), 1);
        Leaf::ROOT.set(&form, Leaf { a: 8, b: 2 }, cx).unwrap();
        assert_eq!(runs.get(), 2);
        Leaf::A.set(&form, 1, cx).unwrap();
        assert_eq!(Leaf::B.errors(&form, cx).len(), 1);
        assert_eq!(
            Leaf::B.errors(&form, cx)[0].message(),
            &ValidationMessage::literal("other")
        );
        let duplicate = vec![
            ValidationRule::new("x", |_, _| {}).dependency(Leaf::A),
            ValidationRule::new("x", |_, _| {}).dependency(Leaf::B),
        ];
        assert!(
            form.update(cx, |f, cx| f.replace_rules(duplicate, cx))
                .is_err()
        );
        assert_eq!(Leaf::B.errors(&form, cx).len(), 1);
    });
}

#[gpui::test]
fn reinserted_optional_payload_has_no_previous_occurrence_baseline(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::new(model(1, 2)));
        let path = Model::ROOT.then(Model::OPTIONAL);
        let old = path
            .clone()
            .some()
            .resolve(&form, cx)
            .unwrap()
            .unwrap()
            .then(Leaf::A);
        Model::OPTIONAL.set(&form, None, cx).unwrap();
        Model::OPTIONAL
            .set(&form, Some(Leaf { a: 1, b: 2 }), cx)
            .unwrap();
        let new = path
            .some()
            .resolve(&form, cx)
            .unwrap()
            .unwrap()
            .then(Leaf::A);
        assert!(old.try_baseline(&form, cx).is_err());
        assert_eq!(new.try_baseline(&form, cx).unwrap(), None);
        form.update(cx, |f, cx| f.reset(gpui_form::DraftPolicy::Discard, cx))
            .unwrap();
        let reset = Model::ROOT
            .then(Model::OPTIONAL)
            .some()
            .resolve(&form, cx)
            .unwrap()
            .unwrap()
            .then(Leaf::A);
        assert_eq!(reset.try_baseline(&form, cx).unwrap(), Some(1));
    });
}

#[gpui::test]
fn equal_value_reorder_and_reconstruction_publish_only_structural_session_changes(
    cx: &mut TestAppContext,
) {
    use std::{cell::Cell, rc::Rc};
    let (_form, model_events, sessions) = cx.update(|cx| {
        let mut initial = model(1, 2);
        initial.rows.push(initial.rows[0].clone());
        let form = cx.new(|_| Form::new(initial));
        let model_events = Rc::new(Cell::new(0));
        let sessions = Rc::new(Cell::new(0));
        let m = model_events.clone();
        let s = sessions.clone();
        cx.subscribe(
            &form,
            move |_, event: &gpui_form::FormEvent<Model>, _| match event {
                gpui_form::FormEvent::ModelChanged(_) => m.set(m.get() + 1),
                gpui_form::FormEvent::SessionChanged(_) => s.set(s.get() + 1),
                _ => {}
            },
        )
        .detach();
        let rows = Model::ROOT.then(Model::ROWS);
        let old = rows.items(&form, cx);
        let revision = form.read(cx).revision();
        rows.move_before(&form, &old[1], &old[0], cx).unwrap();
        assert!(form.read(cx).revision() > revision);
        assert_eq!(old[0].try_get(&form, cx).unwrap(), Leaf { a: 1, b: 2 });
        rows.replace_all(&form, rows.baseline(&form, cx), cx)
            .unwrap();
        assert!(old[0].try_get(&form, cx).is_err());
        (form, model_events, sessions)
    });
    cx.run_until_parked();
    assert_eq!(model_events.get(), 0);
    assert_eq!(sessions.get(), 2);
}
