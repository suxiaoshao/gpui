use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use gpui::{
    AppContext as _, Context, IntoElement, Render, TestAppContext, VisualTestContext, Window, div,
};
use gpui_form::{
    ControlProjection, FieldMerge, Form, FormEvent, FormSchema, LifecycleError, PrepareError,
};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct ModelOverride {
    model: String,
    value: i32,
    unknown: BTreeMap<String, String>,
}

fn entry(model: &str, value: i32) -> ModelOverride {
    ModelOverride {
        model: model.into(),
        value,
        unknown: BTreeMap::new(),
    }
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Overrides {
    #[form(items)]
    entries: Vec<ModelOverride>,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Settings {
    #[form(child, merge = domain::merge_overrides)]
    overrides: Overrides,
    #[form(items)]
    other: Vec<ModelOverride>,
    generation: u32,
}

fn settings(entries: Vec<ModelOverride>, generation: u32) -> Settings {
    Settings {
        overrides: Overrides { entries },
        other: vec![entry("unrelated", 0)],
        generation,
    }
}

mod domain {
    use std::collections::BTreeSet;

    use super::{FieldMerge, ModelOverride, Overrides};

    pub(super) fn merge_overrides(
        base: &Overrides,
        local: &Overrides,
        remote: &Overrides,
    ) -> FieldMerge<Overrides> {
        let keys: BTreeSet<_> = base
            .entries
            .iter()
            .chain(&local.entries)
            .chain(&remote.entries)
            .map(|entry| &entry.model)
            .collect();
        let find = |set: &Overrides, key: &String| -> Option<ModelOverride> {
            set.entries
                .iter()
                .find(|entry| &entry.model == key)
                .cloned()
        };
        let mut conflicted = false;
        let mut entries = Vec::new();
        for key in keys {
            let (base, local, remote) = (find(base, key), find(local, key), find(remote, key));
            let selected = if local == base {
                remote
            } else {
                conflicted |= remote != base && local != remote;
                local
            };
            entries.extend(selected);
        }
        if conflicted {
            FieldMerge::conflicted(Overrides { entries })
        } else {
            FieldMerge::merged(Overrides { entries })
        }
    }
}

#[gpui::test]
fn domain_keys_merge_atomically_with_field_conflicts_and_the_remote_baseline(
    cx: &mut TestAppContext,
) {
    let events = Rc::new(RefCell::new(Vec::new()));
    let _form = cx.update(|cx| {
        let mut opaque = entry("future-model", 0);
        opaque
            .unknown
            .insert("unreadable".into(), "{raw token}".into());
        let initial = settings(vec![entry("a", 1), entry("b", 2), entry("c", 3), opaque], 0);
        let form = cx.new(|_| Form::new(initial.clone()));
        let entries = Settings::ROOT
            .then(Settings::OVERRIDES)
            .then(Overrides::ENTRIES);
        let old = entries.items(&form, cx);
        let unrelated = Settings::ROOT.then(Settings::OTHER).items(&form, cx)[0].clone();
        old[0]
            .clone()
            .then(ModelOverride::VALUE)
            .try_set(&form, 10, cx)
            .unwrap();
        old[2]
            .clone()
            .then(ModelOverride::VALUE)
            .try_set(&form, 30, cx)
            .unwrap();
        Settings::GENERATION.set(&form, 1, cx).unwrap();
        let mut remote = initial;
        remote.overrides.entries[1].value = 20;
        remote.overrides.entries[2].value = 300;
        remote.generation = 9;
        let mut expected = remote.clone();
        expected.overrides.entries[0].value = 10;
        expected.overrides.entries[2].value = 30;
        expected.generation = 1;

        let observed = events.clone();
        let expected_event = expected.clone();
        let remote_event = remote.clone();
        cx.subscribe(&form, move |form, event: &FormEvent<Settings>, cx| {
            if let FormEvent::ModelChanged(change) = event
                && change.kind() == gpui_form::ModelChangeKind::Rebase
            {
                assert_eq!(Settings::ROOT.get(&form, cx), expected_event);
                assert_eq!(Settings::ROOT.baseline(&form, cx), remote_event);
                observed.borrow_mut().push(form.read(cx).revision());
            }
        })
        .detach();
        let revision = form.read(cx).revision();
        let report = form
            .update(cx, |form, cx| {
                form.rebase_preserving_edits(remote.clone(), cx)
            })
            .unwrap();
        assert_eq!(Settings::ROOT.get(&form, cx), expected);
        assert_eq!(Settings::OVERRIDES.baseline(&form, cx), remote.overrides);
        assert!(Settings::OVERRIDES.is_dirty(&form, cx));
        assert_eq!(
            report.conflicts(),
            &[
                Settings::ROOT.then(Settings::OVERRIDES).key(form.read(cx)),
                Settings::ROOT.then(Settings::GENERATION).key(form.read(cx)),
            ]
        );
        assert_eq!(form.read(cx).revision().get(), revision.get() + 1);
        assert!(form.read(cx).is_current(report.version()));
        assert!(old.iter().all(|path| path.try_get(&form, cx).is_err()));
        assert_eq!(unrelated.try_get(&form, cx).unwrap(), entry("unrelated", 0));
        for path in entries.items(&form, cx) {
            assert!(path.try_baseline(&form, cx).unwrap().is_none());
        }
        form
    });
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 1);
}

#[gpui::test]
fn hook_candidates_preserve_or_retire_identity_and_map_only_known_baselines(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        for (local, remote, expected, conflict, baseline) in [
            (2, 1, 2, false, Some(1)),
            (2, 2, 2, false, Some(2)),
            (2, 3, 2, true, None),
            (1, 2, 2, false, Some(2)),
        ] {
            let form = cx.new(|_| Form::new(settings(vec![entry("model", 1)], 0)));
            let entries = Settings::ROOT
                .then(Settings::OVERRIDES)
                .then(Overrides::ENTRIES);
            let old = entries.items(&form, cx)[0].clone();
            old.clone()
                .then(ModelOverride::VALUE)
                .try_set(&form, local, cx)
                .unwrap();
            let report = form
                .update(cx, |form, cx| {
                    form.rebase_preserving_edits(settings(vec![entry("model", remote)], 0), cx)
                })
                .unwrap();
            assert_eq!(!report.conflicts().is_empty(), conflict);
            let current = entries.items(&form, cx)[0].clone();
            assert_eq!(current.try_get(&form, cx).unwrap().value, expected);
            assert_eq!(current.key() == old.key(), expected == local);
            assert_eq!(old.try_get(&form, cx).is_err(), expected != local);
            assert_eq!(
                current
                    .then(ModelOverride::VALUE)
                    .try_baseline(&form, cx)
                    .unwrap(),
                baseline
            );
            assert_eq!(Settings::OVERRIDES.is_dirty(&form, cx), expected != remote);
        }
    });
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct DraftSettings {
    #[form(items, merge = must_skip)]
    overrides: Vec<ModelOverride>,
    #[form(merge = accept_generation)]
    generation: u32,
}

fn must_skip(
    _: &Vec<ModelOverride>,
    _: &Vec<ModelOverride>,
    _: &Vec<ModelOverride>,
) -> FieldMerge<Vec<ModelOverride>> {
    panic!("pending admission and unresolved drafts must skip the hook")
}

fn accept_generation(_: &u32, _: &u32, remote: &u32) -> FieldMerge<u32> {
    FieldMerge::merged(*remote)
}

#[derive(Default)]
struct Editor {
    projections: Vec<ControlProjection<i32>>,
}

impl Render for Editor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
fn pending_edits_reject_atomically_and_drafts_skip_only_their_field_hook(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| cx.new(|_| Editor::default()))
            .unwrap()
    });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let editor = window.root(&mut cx).unwrap();
    let initial = DraftSettings {
        overrides: vec![entry("model", 1)],
        generation: 0,
    };
    let remote = DraftSettings {
        overrides: vec![entry("remote-model", 2)],
        generation: 9,
    };
    let (form, path, _binding, writer) = cx.update(|window, cx| {
        let form = cx.new(|_| Form::new(initial.clone()));
        let path = DraftSettings::ROOT
            .then(DraftSettings::OVERRIDES)
            .items(&form, cx)[0]
            .clone()
            .then(ModelOverride::VALUE);
        let (binding, writer) = path
            .try_bind_control_in(
                &form,
                &editor,
                |editor, projection, _, _| editor.projections.push(projection),
                window,
                cx,
            )
            .unwrap();
        let revision = form.read(cx).revision();
        editor.update(cx, |_, cx| {
            writer.defer_draft("--".to_owned(), "number", "invalid".into(), window, cx)
        });
        assert!(matches!(
            form.update(cx, |form, cx| form
                .rebase_preserving_edits(remote.clone(), cx)),
            Err(LifecycleError::EditsPending)
        ));
        assert_eq!(form.read(cx).revision(), revision);
        assert_eq!(DraftSettings::ROOT.get(&form, cx), initial);
        assert_eq!(DraftSettings::ROOT.baseline(&form, cx), initial);
        (form, path, binding, writer)
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        let report = form
            .update(cx, |form, cx| {
                form.rebase_preserving_edits(initial.clone(), cx)
            })
            .unwrap();
        assert!(report.conflicts().is_empty());
        assert!(
            DraftSettings::ROOT
                .then(DraftSettings::OVERRIDES)
                .is_dirty(&form, cx)
        );
        let report = form
            .update(cx, |form, cx| {
                form.rebase_preserving_edits(remote.clone(), cx)
            })
            .unwrap();
        assert_eq!(report.conflicts().len(), 1);
        assert_eq!(
            DraftSettings::ROOT.get(&form, cx).overrides,
            initial.overrides
        );
        assert_eq!(DraftSettings::GENERATION.get(&form, cx), 9);
        assert_eq!(
            DraftSettings::ROOT
                .then(DraftSettings::OVERRIDES)
                .baseline(&form, cx),
            remote.overrides
        );
        assert_eq!(path.try_get(&form, cx).unwrap(), 1);
        assert_eq!(writer.draft::<String>(cx).unwrap(), Some("--".into()));
        assert!(matches!(
            form.update(cx, |form, cx| form.prepare(cx)),
            Err(PrepareError::UnresolvedDrafts)
        ));
        assert!(form.read(cx).is_current(report.version()));
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(
            !editor
                .read(cx)
                .projections
                .iter()
                .any(|projection| matches!(projection, ControlProjection::Retired))
        )
    });
}
