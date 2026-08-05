use std::{cell::Cell, rc::Rc};

use gpui::{AppContext as _, TestAppContext};
use gpui_form::{Form, FormSchema, PrepareError, ResolveError, ValidationMessage, Validator};

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Leaf {
    #[form(required, validate(on_mount, on_change, on_submit))]
    value: String,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Group {
    #[form(items)]
    children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Node {
    #[form(child)]
    kind: NodeKind,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
enum NodeKind {
    Leaf(Leaf),
    Group(Group),
    Empty,
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct Root {
    #[form(child)]
    group: Group,
    #[form(child)]
    optional: Option<Leaf>,
}

fn initial_root() -> Root {
    Root {
        group: Group {
            children: vec![
                Node {
                    kind: NodeKind::Leaf(Leaf {
                        value: "first".into(),
                    }),
                },
                Node {
                    kind: NodeKind::Leaf(Leaf {
                        value: "second".into(),
                    }),
                },
            ],
        },
        optional: Some(Leaf {
            value: "optional".into(),
        }),
    }
}

#[gpui::test]
fn typed_recursive_paths_preserve_leaf_types_and_retire_cases(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::try_new(initial_root()).unwrap());
        let children = Root::ROOT.then(Root::GROUP).then(Group::CHILDREN);
        let item = children.items(&form, cx).unwrap().remove(0);
        let kind = item.clone().then(Node::KIND);
        let value = kind
            .clone()
            .try_case(form.read(cx), NodeKind::LEAF)
            .unwrap()
            .then(Leaf::VALUE);

        let current: String = value.try_value(&form, cx).unwrap();
        assert_eq!(current, "first");
        value.try_set(&form, "changed".into(), cx).unwrap();
        assert_eq!(value.try_value(&form, cx).unwrap(), "changed");

        kind.try_set(
            &form,
            NodeKind::Group(Group {
                children: Vec::new(),
            }),
            cx,
        )
        .unwrap();
        kind.try_set(
            &form,
            NodeKind::Leaf(Leaf {
                value: "new".into(),
            }),
            cx,
        )
        .unwrap();
        assert!(matches!(
            value.try_value(&form, cx),
            Err(ResolveError::Retired { .. })
        ));

        let fresh = kind
            .try_case(form.read(cx), NodeKind::LEAF)
            .unwrap()
            .then(Leaf::VALUE);
        assert_eq!(fresh.try_value(&form, cx).unwrap(), "new");
    });
}

#[gpui::test]
fn item_identity_survives_reorder_and_not_replacement(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::try_new(initial_root()).unwrap());
        let children = Root::ROOT.then(Root::GROUP).then(Group::CHILDREN);
        let items = children.items(&form, cx).unwrap();
        let first = items[0].clone();
        let second = items[1].clone();
        let first_key = first.key();

        children.move_before(&form, &second, &first, cx).unwrap();
        assert_eq!(first.key(), first_key);
        let first_value = first
            .clone()
            .then(Node::KIND)
            .try_case(form.read(cx), NodeKind::LEAF)
            .unwrap()
            .then(Leaf::VALUE);
        assert_eq!(first_value.try_value(&form, cx).unwrap(), "first");

        children
            .replace_all(
                &form,
                vec![Node {
                    kind: NodeKind::Leaf(Leaf {
                        value: "replacement".into(),
                    }),
                }],
                cx,
            )
            .unwrap();
        assert!(matches!(
            first.try_value(&form, cx),
            Err(ResolveError::Retired { .. })
        ));
    });
}

#[gpui::test]
fn path_key_converts_to_stable_gpui_element_identity(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::try_new(initial_root()).unwrap());
        let item = Root::ROOT
            .then(Root::GROUP)
            .then(Group::CHILDREN)
            .items(&form, cx)
            .unwrap()
            .remove(0);
        let first: gpui::ElementId = item.key().into();
        let second: gpui::ElementId = (&item.key()).into();
        assert_eq!(first, second);
    });
}

#[gpui::test]
fn optional_incarnation_and_prepared_cas_are_session_bound(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| Form::try_new(initial_root()).unwrap());
        let optional = Root::ROOT.then(Root::OPTIONAL);
        let value = optional
            .clone()
            .try_some(form.read(cx))
            .unwrap()
            .then(Leaf::VALUE);
        optional.set(&form, None, cx);
        optional.set(
            &form,
            Some(Leaf {
                value: "rebuilt".into(),
            }),
            cx,
        );
        assert!(matches!(
            value.try_value(&form, cx),
            Err(ResolveError::Retired { .. })
        ));

        let prepared = form.update(cx, |form, cx| form.prepare(cx)).unwrap();
        let revision = prepared.revision();
        Root::ROOT
            .then(Root::GROUP)
            .then(Group::CHILDREN)
            .append(
                &form,
                Node {
                    kind: NodeKind::Empty,
                },
                cx,
            )
            .unwrap();
        assert!(!form.update(cx, |form, cx| {
            form.rebase_if_revision(revision, initial_root(), cx)
        }));
    });
}

struct RejectForbidden;

impl Validator<Leaf> for RejectForbidden {
    fn validate(
        &self,
        model: &Leaf,
        request: gpui_form::ValidationRequest<'_, Leaf>,
        out: &mut gpui_form::ValidationSink<'_, Leaf>,
    ) {
        if request.includes(&Leaf::VALUE) && model.value == "forbidden" {
            out.at(Leaf::VALUE)
                .error("forbidden", ValidationMessage::key("value-forbidden"));
        }
    }
}

#[gpui::test]
fn validation_is_field_scoped_and_prepare_uses_the_same_snapshot(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| {
            Form::try_new_with_validator(
                Leaf {
                    value: String::new(),
                },
                RejectForbidden,
            )
            .unwrap()
        });
        assert_eq!(Leaf::VALUE.errors(&form, cx).len(), 1);
        assert!(matches!(
            form.update(cx, |form, cx| form.prepare(cx)),
            Err(PrepareError::Validation(_))
        ));

        Leaf::VALUE.set(&form, "forbidden".into(), cx);
        let errors = Leaf::VALUE.errors(&form, cx);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "forbidden");

        Leaf::VALUE.set(&form, "allowed".into(), cx);
        let prepared = form.update(cx, |form, cx| form.prepare(cx)).unwrap();
        assert_eq!(prepared.value().value, "allowed");
    });
}

struct RejectSecondLeaf;

impl gpui_form::Validator<Root> for RejectSecondLeaf {
    fn validate(
        &self,
        model: &Root,
        request: gpui_form::ValidationRequest<'_, Root>,
        out: &mut gpui_form::ValidationSink<'_, Root>,
    ) {
        let children = Root::ROOT.then(Root::GROUP).then(Group::CHILDREN);
        let Ok(items) = request.items(model, &children) else {
            return;
        };
        for item in items {
            let kind = item.then(Node::KIND);
            let Ok(NodeKind::Leaf(_)) = request.value(model, &kind) else {
                continue;
            };
            let Ok(leaf) = request.try_case(model, kind, NodeKind::LEAF) else {
                continue;
            };
            let value = leaf.then(Leaf::VALUE);
            if request
                .value(model, &value)
                .is_ok_and(|value| value == "second")
            {
                out.at(value)
                    .error("second", ValidationMessage::key("second-not-allowed"));
            }
        }
    }
}

#[gpui::test]
fn validation_resolver_enumerates_recursive_items_on_one_snapshot(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form =
            cx.new(|_| Form::try_new_with_validator(initial_root(), RejectSecondLeaf).unwrap());
        let children = Root::ROOT.then(Root::GROUP).then(Group::CHILDREN);
        let items = children.items(&form, cx).unwrap();
        let second = items[1]
            .clone()
            .then(Node::KIND)
            .try_case(form.read(cx), NodeKind::LEAF)
            .unwrap()
            .then(Leaf::VALUE);

        form.update(cx, |form, cx| {
            form.validate(gpui_form::ValidationTrigger::Submit, cx)
        });
        let errors = second.try_errors(&form, cx).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "second");
    });
}

#[gpui::test]
fn topology_handles_deep_and_large_recursive_trees(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut nested = Node {
            kind: NodeKind::Leaf(Leaf {
                value: "bottom".into(),
            }),
        };
        for _ in 0..128 {
            nested = Node {
                kind: NodeKind::Group(Group {
                    children: vec![nested],
                }),
            };
        }
        let mut root = initial_root();
        root.group.children = vec![nested];
        let form = cx.new(|_| Form::try_new(root).unwrap());
        let root_children = Root::ROOT.then(Root::GROUP).then(Group::CHILDREN);
        let mut node = root_children.items(&form, cx).unwrap().remove(0);
        for _ in 0..128 {
            let group = node
                .then(Node::KIND)
                .try_case(form.read(cx), NodeKind::GROUP)
                .unwrap();
            node = group
                .then(Group::CHILDREN)
                .try_items(&form, cx)
                .unwrap()
                .remove(0);
        }
        let value = node
            .then(Node::KIND)
            .try_case(form.read(cx), NodeKind::LEAF)
            .unwrap()
            .then(Leaf::VALUE)
            .try_value(&form, cx)
            .unwrap();
        assert_eq!(value, "bottom");

        root_children
            .replace_all(
                &form,
                (0..10_000)
                    .map(|index| Node {
                        kind: NodeKind::Leaf(Leaf {
                            value: index.to_string(),
                        }),
                    })
                    .collect(),
                cx,
            )
            .unwrap();
        assert_eq!(root_children.items(&form, cx).unwrap().len(), 10_000);
    });
}

#[gpui::test]
fn dynamic_paths_reject_wrong_sessions_and_parent_cycles(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut root = initial_root();
        root.group.children = vec![Node {
            kind: NodeKind::Group(Group {
                children: Vec::new(),
            }),
        }];
        let form = cx.new(|_| Form::try_new(root.clone()).unwrap());
        let other = cx.new(|_| Form::try_new(root).unwrap());
        let children = Root::ROOT.then(Root::GROUP).then(Group::CHILDREN);
        let group_item = children.items(&form, cx).unwrap().remove(0);
        assert!(matches!(
            group_item.try_value(&other, cx),
            Err(ResolveError::WrongSession { .. })
        ));

        let nested_children = group_item
            .clone()
            .then(Node::KIND)
            .try_case(form.read(cx), NodeKind::GROUP)
            .unwrap()
            .then(Group::CHILDREN);
        assert!(matches!(
            group_item.move_to(&form, nested_children, gpui_form::Position::End, cx,),
            Err(gpui_form::MutationError::Topology(
                gpui_form::TopologyError::MoveIntoDescendant { .. }
            ))
        ));
        assert_eq!(children.items(&form, cx).unwrap().len(), 1);
    });
}

#[gpui::test]
fn async_validation_blocks_prepare_and_discards_intersecting_work(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|_| {
            Form::try_new(Leaf {
                value: "remote".into(),
            })
            .unwrap()
        })
    });
    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.start_async_validation(
                Leaf::VALUE,
                "remote",
                |_| async {
                    Err(gpui_form::AsyncValidationIssue::new(
                        "unavailable",
                        ValidationMessage::key("value-unavailable"),
                    ))
                },
                cx,
            )
            .unwrap();
            assert!(form.is_validating());
            assert_eq!(form.prepare(cx), Err(PrepareError::ValidationPending));
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!form.read(cx).is_validating());
        assert_eq!(Leaf::VALUE.errors(&form, cx)[0].code(), "unavailable");

        form.update(cx, |form, cx| {
            form.start_async_validation(Leaf::VALUE, "pending", |_| std::future::pending(), cx)
                .unwrap();
        });
        Leaf::VALUE.set(&form, "changed".into(), cx);
        assert!(!form.read(cx).is_validating());
        assert!(Leaf::VALUE.errors(&form, cx).is_empty());
    });
}

struct SubmitOnly;

impl Validator<Leaf> for SubmitOnly {
    fn validate(
        &self,
        _model: &Leaf,
        request: gpui_form::ValidationRequest<'_, Leaf>,
        out: &mut gpui_form::ValidationSink<'_, Leaf>,
    ) {
        if request.trigger() == gpui_form::ValidationTrigger::Submit {
            out.at(Leaf::VALUE)
                .error("submit-only", ValidationMessage::literal("submit only"));
        }
    }
}

#[gpui::test]
fn validation_keeps_independent_trigger_buckets(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| {
            Form::try_new_with_validator(
                Leaf {
                    value: "value".into(),
                },
                SubmitOnly,
            )
            .unwrap()
        });
        assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
        Leaf::VALUE.validate(&form, gpui_form::ValidationTrigger::Blur, cx);
        let errors = Leaf::VALUE.errors(&form, cx);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].trigger(), gpui_form::ValidationTrigger::Submit);
    });
}

#[derive(Clone, Debug, PartialEq, FormSchema)]
struct MoveRoot {
    #[form(child)]
    source: Group,
    #[form(child)]
    destination: Group,
}

#[gpui::test]
fn cross_parent_move_revalidates_source_and_destination_once(cx: &mut TestAppContext) {
    let events = Rc::new(Cell::new(0));
    let observed = Rc::new(Cell::new(0));
    let (form, events, observed) = cx.update(|cx| {
        let form = cx.new(|_| {
            Form::try_new(MoveRoot {
                source: Group {
                    children: vec![Node {
                        kind: NodeKind::Leaf(Leaf {
                            value: String::new(),
                        }),
                    }],
                },
                destination: Group { children: vec![] },
            })
            .unwrap()
        });
        let event_count = events.clone();
        cx.subscribe(&form, move |_, _: &gpui_form::FormEvent, _| {
            event_count.set(event_count.get() + 1);
        })
        .detach();
        let observed_count = observed.clone();
        cx.observe(&form, move |_, _| {
            observed_count.set(observed_count.get() + 1);
        })
        .detach();
        (form, events.clone(), observed.clone())
    });
    cx.update(|cx| {
        let source = MoveRoot::ROOT.then(MoveRoot::SOURCE).then(Group::CHILDREN);
        let destination = MoveRoot::ROOT
            .then(MoveRoot::DESTINATION)
            .then(Group::CHILDREN);
        let old = source.items(&form, cx).unwrap().remove(0);
        let moved = old
            .clone()
            .move_to(&form, destination.clone(), gpui_form::Position::End, cx)
            .unwrap();
        assert!(matches!(
            old.try_value(&form, cx),
            Err(ResolveError::Retired { .. })
        ));
        let value = moved
            .then(Node::KIND)
            .try_case(form.read(cx), NodeKind::LEAF)
            .unwrap()
            .then(Leaf::VALUE);
        assert_eq!(value.try_errors(&form, cx).unwrap().len(), 1);
        assert!(source.items(&form, cx).unwrap().is_empty());
        assert_eq!(destination.items(&form, cx).unwrap().len(), 1);
    });
    cx.run_until_parked();
    assert_eq!(events.get(), 1);
    assert_eq!(observed.get(), 1);
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, PartialEq, FormSchema, garde::Validate)]
struct GardeRow {
    #[form(validate(on_submit))]
    #[garde(length(min = 1))]
    label: String,
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, PartialEq, FormSchema, garde::Validate)]
struct GardeRoot {
    #[form(items)]
    #[garde(dive)]
    rows: Vec<GardeRow>,
}

#[cfg(feature = "garde-adapter")]
#[gpui::test]
fn garde_positions_map_to_runtime_item_paths(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let form = cx.new(|_| {
            Form::try_new_with_validator(
                GardeRoot {
                    rows: vec![
                        GardeRow {
                            label: "valid".into(),
                        },
                        GardeRow {
                            label: String::new(),
                        },
                    ],
                },
                gpui_form::GardeValidator::<GardeRoot>::new(()),
            )
            .unwrap()
        });
        let rows = GardeRoot::ROWS.items(&form, cx).unwrap();
        let second_label = rows[1].clone().then(GardeRow::LABEL);
        assert!(second_label.try_errors(&form, cx).unwrap().is_empty());

        assert!(matches!(
            form.update(cx, |form, cx| form.prepare(cx)),
            Err(PrepareError::Validation(_))
        ));
        let errors = second_label.try_errors(&form, cx).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code(), "garde");
        assert!(
            rows[0]
                .clone()
                .then(GardeRow::LABEL)
                .try_errors(&form, cx)
                .unwrap()
                .is_empty()
        );
    });
}
