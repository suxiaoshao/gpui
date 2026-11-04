use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_form::{
    ControlMapping, ControlProjection, EditProblem, EditResult, Form, FormEvent, FormSchema,
    MappedControlBinding, MappedControlWriter, PrepareError, bind_mapped_control_in,
};

#[derive(Default)]
struct Editor {
    values: Vec<ControlProjection<String>>,
    admission_counts: Vec<usize>,
}
impl Render for Editor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
fn open(cx: &mut TestAppContext) -> WindowHandle<Editor> {
    cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| cx.new(|_| Editor::default()))
            .unwrap()
    })
}

type Tokens = BTreeMap<String, u64>;
#[derive(Clone, PartialEq, FormSchema)]
struct TokenModel {
    tokens: Tokens,
}
struct TokenMapping {
    key: &'static str,
    whole: bool,
    calls: Rc<Cell<usize>>,
}
impl ControlMapping<Tokens, String> for TokenMapping {
    type Context = u64;
    fn format(&self, stored: &Tokens, inherited: &u64) -> String {
        stored.get(self.key).unwrap_or(inherited).to_string()
    }
    fn parse(
        &self,
        display: String,
        stored: &Tokens,
        baseline: Option<&Tokens>,
        inherited: &u64,
    ) -> EditResult<Tokens, String> {
        let Ok(value) = display.parse::<u64>() else {
            return EditResult::draft(display, "syntax", "integer required");
        };
        let baseline = baseline.and_then(|base| base.get(self.key)).copied();
        let value = if value == baseline.unwrap_or(*inherited) {
            baseline
        } else {
            Some(value)
        };
        let key = self.key.to_owned();
        if self.whole {
            let mut next = stored.clone();
            if let Some(value) = value {
                next.insert(key, value);
            } else {
                next.remove(&key);
            }
            return EditResult::Parsed(next);
        }
        let calls = self.calls.clone();
        EditResult::update(move |latest: &Tokens| {
            calls.set(calls.get() + 1);
            if latest.contains_key("locked") {
                return Err(EditProblem::new("locked", "overrides are locked"));
            }
            let mut next = latest.clone();
            if let Some(value) = value {
                next.insert(key, value);
            } else {
                next.remove(&key);
            }
            Ok(next)
        })
    }
}
type TokenWriter = MappedControlWriter<TokenModel, Tokens, String, TokenMapping>;
#[gpui::test]
fn editor_discard_cancels_only_its_tickets_and_preserves_peer_input(cx: &mut TestAppContext) {
    let window = open(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let calls = Rc::new(Cell::new(0));
    let (form, a, aw, b, bw, saved) = cx.update(|window, cx| {
        let form = cx.new(|_| {
            Form::new(TokenModel {
                tokens: Tokens::new(),
            })
        });
        let (a, aw) = bind_token(
            &form,
            TokenMapping {
                key: "reserve",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        let (b, bw) = bind_token(
            &form,
            TokenMapping {
                key: "keep",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        let saved = form.update(cx, |form, cx| form.prepare(cx)).unwrap();
        let revision = form.read(cx).revision();
        root.update(cx, |_, cx| aw.defer_edit("20".into(), window, cx));
        assert!(
            !aw.editor_status(cx).unwrap().has_draft(),
            "pending input is not an accepted draft"
        );
        a.discard_draft(cx).unwrap();
        assert_eq!(form.read(cx).revision().get(), revision.get() + 1);
        root.update(cx, |_, cx| bw.defer_edit("10".into(), window, cx));
        assert!(matches!(
            form.update(cx, |form, cx| form.prepare(cx)),
            Err(PrepareError::EditsPending)
        ));
        (form, a, aw, b, bw, saved)
    });
    cx.run_until_parked();
    let events = Rc::new(RefCell::new(Vec::new()));
    cx.update(|window, cx| {
        assert_eq!(calls.get(), 1, "only the peer's equal update was admitted");
        assert!(TokenModel::TOKENS.get(&form, cx).is_empty());
        assert!(!form.update(cx, |form, cx| form.rebase_if_current(
            saved.version(),
            saved.value().clone(),
            cx
        )));
        let prepared = form.update(cx, |form, cx| form.prepare(cx)).unwrap();
        let revision = form.read(cx).revision();
        a.discard_draft(cx).unwrap();
        assert_eq!(
            form.read(cx).revision(),
            revision,
            "empty discard is a no-op"
        );
        assert!(form.read(cx).is_current(prepared.version()));
        let observed = events.clone();
        cx.subscribe(&form, move |_, event: &FormEvent<TokenModel>, _| {
            if let FormEvent::DraftChanged(change) = event {
                observed.borrow_mut().push(change.revision());
            }
        })
        .detach();
        root.update(cx, |_, cx| {
            aw.defer_edit("bad-a".into(), window, cx);
            bw.defer_edit("bad-b".into(), window, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(a.editor_status(cx).unwrap().has_draft());
        let peer = b.editor_status(cx).unwrap();
        assert!(peer.has_draft());
        events.borrow_mut().clear();
        root.update(cx, |_, cx| {
            bw.defer_edit("30".into(), window, cx);
            aw.defer_edit("20".into(), window, cx);
        });
        let revision = form.read(cx).revision();
        aw.discard_draft(cx).unwrap();
        assert_eq!(form.read(cx).revision().get(), revision.get() + 1);
        assert_eq!(
            b.editor_status(cx).unwrap(),
            peer,
            "peer draft and issue survive the command"
        );
        assert!(!a.editor_status(cx).unwrap().has_draft());
        assert!(a.editor_status(cx).unwrap().issues().is_empty());
        // Both the peer's later input and new source input must survive the queued discard notice.
        root.update(cx, |_, cx| {
            bw.defer_edit("50".into(), window, cx);
            aw.defer_edit("40".into(), window, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(calls.get(), 4, "cancelled source updates never execute");
        assert_eq!(
            TokenModel::TOKENS.get(&form, cx),
            Tokens::from([("reserve".into(), 40), ("keep".into(), 50)])
        );
        assert_eq!(
            events.borrow().len(),
            2,
            "one source discard and the peer's later successful clear"
        );
        assert!(!a.editor_status(cx).unwrap().has_draft());
        assert!(!b.editor_status(cx).unwrap().has_draft());
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok());
    });
}

struct CatalogMapping {
    problem: &'static str,
    parses: Rc<Cell<usize>>,
}
impl ControlMapping<Tokens, String> for CatalogMapping {
    type Context = Option<u64>;
    fn format(&self, _: &Tokens, context: &Option<u64>) -> String {
        context.map_or_else(|| "missing".into(), |value| value.to_string())
    }
    fn parse(
        &self,
        display: String,
        _: &Tokens,
        _: Option<&Tokens>,
        _: &Option<u64>,
    ) -> EditResult<Tokens, String> {
        self.parses.set(self.parses.get() + 1);
        EditResult::draft(display, self.problem, self.problem)
    }
}

#[gpui::test]
fn editor_status_is_read_only_and_retains_the_right_issue_after_context_loss_and_unmount(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let parses = Rc::new(Cell::new(0));
    let (form, a, aw, b, bw) = cx.update(|window, cx| {
        let form = cx.new(|_| {
            Form::new(TokenModel {
                tokens: Tokens::new(),
            })
        });
        let (a, aw) = bind_mapped_control_in(
            &form,
            TokenModel::TOKENS,
            "reserve",
            CatalogMapping {
                problem: "reserve-invalid",
                parses: parses.clone(),
            },
            Some(10),
            &root,
            |_, _, _, _| {},
            window,
            cx,
        )
        .unwrap();
        let (b, bw) = bind_mapped_control_in(
            &form,
            TokenModel::TOKENS,
            "keep",
            CatalogMapping {
                problem: "keep-invalid",
                parses: parses.clone(),
            },
            Some(30),
            &root,
            |_, _, _, _| {},
            window,
            cx,
        )
        .unwrap();
        root.update(cx, |_, cx| {
            aw.defer_edit("bad-a".into(), window, cx);
            bw.defer_edit("bad-b".into(), window, cx);
        });
        (form, a, aw, b, bw)
    });
    cx.run_until_parked();
    cx.update(|_, cx| a.refresh(None, cx));
    cx.run_until_parked();
    let revision = cx.update(|_, cx| {
        let revision = form.read(cx).revision();
        let status = a.editor_status(cx).unwrap();
        assert!(status.has_draft());
        assert_eq!(status.issues().len(), 1);
        assert_eq!(status.issues()[0].code(), "reserve-invalid");
        assert_eq!(
            b.editor_status(cx).unwrap().issues()[0].code(),
            "keep-invalid"
        );
        let absent = form
            .read(cx)
            .editor_status(TokenModel::TOKENS, "absent")
            .unwrap();
        assert!(!absent.has_draft());
        assert!(absent.issues().is_empty());
        assert_eq!(form.read(cx).revision(), revision);
        assert_eq!(
            parses.get(),
            2,
            "losing the inherited key never reparses retained input"
        );
        revision
    });
    drop((a, aw, b, bw));
    cx.update(|_, cx| {
        let status = form
            .read(cx)
            .editor_status(TokenModel::TOKENS, "reserve")
            .unwrap();
        assert!(status.has_draft());
        assert_eq!(status.issues()[0].code(), "reserve-invalid");
        assert_eq!(form.read(cx).revision(), revision);
        form.update(cx, |form, cx| {
            form.discard_editor_draft(TokenModel::TOKENS, "reserve", cx)
        })
        .unwrap();
        assert!(
            !form
                .read(cx)
                .editor_status(TokenModel::TOKENS, "reserve")
                .unwrap()
                .has_draft()
        );
        let peer = form
            .read(cx)
            .editor_status(TokenModel::TOKENS, "keep")
            .unwrap();
        assert!(peer.has_draft());
        assert_eq!(peer.issues()[0].code(), "keep-invalid");
        assert_eq!(form.read(cx).revision().get(), revision.get() + 1);
    });
}

#[gpui::test]
fn stable_editor_keys_own_their_issues_even_with_cloned_writers(cx: &mut TestAppContext) {
    let window = open(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let (form, _binding, a, _b) = cx.update(|window, cx| {
        let form = cx.new(|_| {
            Form::new(TokenModel {
                tokens: Tokens::new(),
            })
        });
        let (binding, writer) =
            TokenModel::TOKENS.bind_control_in(&form, &root, |_, _, _, _| {}, window, cx);
        let a = writer.clone().with_editor_key("reserve", cx);
        let b = writer.with_editor_key("keep", cx);
        root.update(cx, |_, cx| {
            a.defer_draft(
                "bad-a".to_owned(),
                "a",
                "invalid reserve".into(),
                window,
                cx,
            );
            b.defer_draft("bad-b".to_owned(), "b", "invalid keep".into(), window, cx);
        });
        (form, binding, a, b)
    });
    cx.run_until_parked();
    let revision = cx.update(|window, cx| {
        let revision = form.read(cx).revision();
        assert_eq!(a.editor_status(cx).unwrap().issues()[0].code(), "a");
        root.update(cx, |_, cx| a.defer_clear_issue(window, cx));
        revision
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            form.read(cx).revision(),
            revision,
            "issue-only changes do not advance editing revision"
        );
        assert!(a.editor_status(cx).unwrap().has_draft());
        assert!(a.editor_status(cx).unwrap().issues().is_empty());
        let peer = form
            .read(cx)
            .editor_status(TokenModel::TOKENS, "keep")
            .unwrap();
        a.discard_draft(cx).unwrap();
        assert_eq!(
            form.read(cx)
                .editor_status(TokenModel::TOKENS, "keep")
                .unwrap(),
            peer
        );
        root.update(cx, |_, cx| {
            a.defer_draft(
                "bad-a".to_owned(),
                "a",
                "invalid reserve".into(),
                window,
                cx,
            );
            a.defer_set(Tokens::new(), window, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(!a.editor_status(cx).unwrap().has_draft());
        let peer = form
            .read(cx)
            .editor_status(TokenModel::TOKENS, "keep")
            .unwrap();
        assert!(peer.has_draft());
        assert_eq!(peer.issues()[0].code(), "b");
    });
}

fn bind_token(
    form: &Entity<Form<TokenModel>>,
    mapping: TokenMapping,
    owner: &Entity<Editor>,
    window: &mut Window,
    cx: &mut gpui::App,
) -> (MappedControlBinding<u64>, TokenWriter) {
    let calls = mapping.calls.clone();
    bind_mapped_control_in(
        form,
        TokenModel::TOKENS,
        mapping.key,
        mapping,
        10,
        owner,
        move |owner, value, _, _| {
            owner.values.push(value);
            owner.admission_counts.push(calls.get());
        },
        window,
        cx,
    )
    .unwrap()
}

#[gpui::test]
fn queued_partial_updates_keep_both_keys_and_restore_only_the_target_baseline(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let calls = Rc::new(Cell::new(0));
    let (form, a, aw, _b, bw) = cx.update(|window, cx| {
        let form = cx.new(|_| {
            Form::new(TokenModel {
                tokens: Tokens::new(),
            })
        });
        let (a, aw) = bind_token(
            &form,
            TokenMapping {
                key: "reserve",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        let (b, bw) = bind_token(
            &form,
            TokenMapping {
                key: "keep",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        let prepared = form.update(cx, |form, cx| form.prepare(cx)).unwrap();
        root.update(cx, |_, cx| {
            aw.defer_edit("20".into(), window, cx);
            bw.defer_edit("30".into(), window, cx);
        });
        assert_eq!(calls.get(), 0);
        assert!(matches!(
            form.update(cx, |form, cx| form.prepare(cx)),
            Err(PrepareError::EditsPending)
        ));
        assert!(!form.update(cx, |form, cx| form.rebase_if_current(
            prepared.version(),
            prepared.value().clone(),
            cx
        )));
        (form, a, aw, b, bw)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            TokenModel::TOKENS.get(&form, cx),
            Tokens::from([("reserve".into(), 20), ("keep".into(), 30)])
        );
        let mut current = TokenModel::TOKENS.get(&form, cx);
        current.insert("future".into(), 7);
        TokenModel::TOKENS.set(&form, current, cx).unwrap();
        root.update(cx, |editor, _| editor.admission_counts.clear());
        // Keep a drain scheduled before native admission, then replace its value below.
        a.refresh(10, cx);
        root.update(cx, |_, cx| {
            aw.defer_edit("40".into(), window, cx);
            aw.defer_edit("10".into(), window, cx);
            bw.defer_edit("50".into(), window, cx);
        });
        a.refresh(99, cx);
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            TokenModel::TOKENS.get(&form, cx),
            Tokens::from([("keep".into(), 50), ("future".into(), 7)])
        );
        assert!(TokenModel::TOKENS.baseline(&form, cx).is_empty());
        assert!(form.read(cx).is_dirty());
        assert_eq!(calls.get(), 5);
        assert!(
            root.read(cx)
                .admission_counts
                .iter()
                .all(|count| *count >= 5),
            "pending input must not receive an earlier projection"
        );
    });
}

#[gpui::test]
fn rejected_update_restores_original_text_and_equal_success_clears_only_its_editor(
    cx: &mut TestAppContext,
) {
    let window = open(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let calls = Rc::new(Cell::new(0));
    let (form, a, aw, _b, bw) = cx.update(|window, cx| {
        let form = cx.new(|_| {
            Form::new(TokenModel {
                tokens: Tokens::new(),
            })
        });
        let (a, aw) = bind_token(
            &form,
            TokenMapping {
                key: "reserve",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        let (b, bw) = bind_token(
            &form,
            TokenMapping {
                key: "keep",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        root.update(cx, |_, cx| {
            bw.defer_edit("--".into(), window, cx);
            aw.defer_edit("0020".into(), window, cx);
        });
        TokenModel::TOKENS
            .set(&form, Tokens::from([("locked".into(), 1)]), cx)
            .unwrap();
        (form, a, aw, b, bw)
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            TokenModel::TOKENS.get(&form, cx),
            Tokens::from([("locked".into(), 1)])
        );
        assert_eq!(calls.get(), 1);
        assert!(
            form.read(cx)
                .validation_report()
                .issues()
                .iter()
                .any(|issue| issue.code() == "locked")
        );
        assert!(matches!(
            form.update(cx, |form, cx| form.prepare(cx)),
            Err(PrepareError::UnresolvedDrafts)
        ));
        a.refresh(99, cx);
    });
    cx.run_until_parked();
    assert_eq!(calls.get(), 1, "refresh must never retry an update");
    drop((a, aw));
    let (_restored, writer) = cx.update(|window, cx| {
        let result = bind_token(
            &form,
            TokenMapping {
                key: "reserve",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        assert_eq!(
            root.read(cx).values.last(),
            Some(&ControlProjection::Value("0020".into()))
        );
        TokenModel::TOKENS.set(&form, Tokens::new(), cx).unwrap();
        result
    });
    let revision = cx.update(|window, cx| {
        let revision = form.read(cx).revision();
        root.update(cx, |_, cx| writer.defer_edit("10".into(), window, cx));
        revision
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(form.read(cx).revision().get(), revision.get() + 1);
        assert!(TokenModel::TOKENS.get(&form, cx).is_empty());
        let report = form.read(cx).validation_report();
        assert_eq!(report.issues().len(), 1);
        assert_eq!(report.issues()[0].code(), "syntax");
        assert!(
            form.read(cx).is_dirty(),
            "the peer's draft still counts as dirty"
        );
        root.update(cx, |_, cx| bw.defer_edit("30".into(), window, cx));
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok()));
}

#[gpui::test]
fn parsed_replaces_the_whole_value_and_cancelled_updates_never_run(cx: &mut TestAppContext) {
    let window = open(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).unwrap();
    let calls = Rc::new(Cell::new(0));
    let (form, a, aw, _b, bw) = cx.update(|window, cx| {
        let form = cx.new(|_| {
            Form::new(TokenModel {
                tokens: Tokens::new(),
            })
        });
        let (a, aw) = bind_token(
            &form,
            TokenMapping {
                key: "reserve",
                whole: false,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        let (b, bw) = bind_token(
            &form,
            TokenMapping {
                key: "keep",
                whole: true,
                calls: calls.clone(),
            },
            &root,
            window,
            cx,
        );
        root.update(cx, |_, cx| {
            aw.defer_edit("20".into(), window, cx);
            bw.defer_edit("30".into(), window, cx);
        });
        (form, a, aw, b, bw)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            TokenModel::TOKENS.get(&form, cx),
            Tokens::from([("keep".into(), 30)])
        );
        root.update(cx, |_, cx| {
            bw.defer_edit("50".into(), window, cx);
            aw.defer_edit("40".into(), window, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert_eq!(
            TokenModel::TOKENS.get(&form, cx),
            Tokens::from([("reserve".into(), 40), ("keep".into(), 50)])
        );
        root.update(cx, |_, cx| aw.defer_edit("99".into(), window, cx));
        form.update(cx, |form, cx| form.discard_drafts(TokenModel::TOKENS, cx))
            .unwrap();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        root.update(cx, |_, cx| aw.defer_edit("100".into(), window, cx));
        drop(a);
    });
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(calls.get(), 2);
        assert_eq!(
            TokenModel::TOKENS.get(&form, cx),
            Tokens::from([("reserve".into(), 40), ("keep".into(), 50)])
        );
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok());
    });
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Row {
    value: u64,
}
#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Rows {
    #[form(items)]
    rows: Vec<Row>,
}
struct RowsMapping(bool);
impl ControlMapping<Rows, String> for RowsMapping {
    type Context = ();
    fn format(&self, stored: &Rows, _: &()) -> String {
        stored.rows[0].value.to_string()
    }
    fn parse(
        &self,
        display: String,
        stored: &Rows,
        _: Option<&Rows>,
        _: &(),
    ) -> EditResult<Rows, String> {
        let Ok(value) = display.parse() else {
            return EditResult::draft(display, "number", "integer required");
        };
        if self.0 {
            let mut next = stored.clone();
            next.rows[0].value = value;
            return EditResult::Parsed(next);
        }
        EditResult::update(move |latest: &Rows| {
            let mut next = latest.clone();
            next.rows[0].value = value;
            Ok(next)
        })
    }
}

#[gpui::test]
fn structural_rejection_keeps_both_drafts_and_success_shares_one_revision(cx: &mut TestAppContext) {
    for whole in [false, true] {
        let window = open(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).unwrap();
        let (form, old, _child, binding, writer) = cx.update(|window, cx| {
            let form = cx.new(|_| {
                Form::new(Rows {
                    rows: vec![Row { value: 1 }],
                })
            });
            let old = Rows::ROOT.then(Rows::ROWS).items(&form, cx)[0]
                .clone()
                .then(Row::VALUE);
            let (child, child_writer) = old
                .try_bind_control_in(&form, &root, |_, _, _, _| {}, window, cx)
                .unwrap();
            let (binding, writer) = bind_mapped_control_in(
                &form,
                Rows::ROOT,
                "rows",
                RowsMapping(whole),
                (),
                &root,
                |owner, value, _, _| owner.values.push(value),
                window,
                cx,
            )
            .unwrap();
            root.update(cx, |_, cx| {
                child_writer.defer_draft("--".to_owned(), "child", "bad child".into(), window, cx)
            });
            (form, old, child, binding, writer)
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            root.update(cx, |_, cx| writer.defer_edit("0020".into(), window, cx))
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(old.try_get(&form, cx).unwrap(), 1);
            assert_eq!(Rows::ROOT.get(&form, cx), Rows::ROOT.baseline(&form, cx));
            assert!(
                form.read(cx)
                    .validation_report()
                    .issues()
                    .iter()
                    .any(|issue| issue.code() == "form-edit-would-retire-drafts")
            );
        });
        drop((binding, writer));
        let (_binding, writer) = cx.update(|window, cx| {
            let binding = bind_mapped_control_in(
                &form,
                Rows::ROOT,
                "rows",
                RowsMapping(whole),
                (),
                &root,
                |owner, value, _, _| owner.values.push(value),
                window,
                cx,
            )
            .unwrap();
            assert_eq!(
                root.read(cx).values.last(),
                Some(&ControlProjection::Value("0020".into()))
            );
            form.update(cx, |form, cx| form.discard_drafts(old.clone(), cx))
                .unwrap();
            binding
        });
        cx.run_until_parked();
        let events = Rc::new(RefCell::new(Vec::new()));
        let revision = cx.update(|window, cx| {
            let observed = events.clone();
            cx.subscribe(&form, move |_, event: &FormEvent<Rows>, _| match event {
                FormEvent::ModelChanged(change) | FormEvent::DraftChanged(change) => {
                    observed.borrow_mut().push(change.revision())
                }
                _ => {}
            })
            .detach();
            let revision = form.read(cx).revision();
            root.update(cx, |_, cx| writer.defer_edit("30".into(), window, cx));
            revision
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(form.read(cx).revision().get(), revision.get() + 1);
            assert_eq!(*events.borrow(), vec![form.read(cx).revision(); 2]);
            assert!(old.try_get(&form, cx).is_err());
            assert_eq!(Rows::ROOT.get(&form, cx).rows[0].value, 30);
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_ok());
        });
    }
}
