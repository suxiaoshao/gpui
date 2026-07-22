use gpui::{AppContext as _, TestAppContext};
use gpui_form::typed::{
    AsyncValidationIssue, FieldPath, FieldPathSegment, FormFieldError, FormFieldId as _,
    FormItemId, FormRevision, FormStore as _, GardePathError, GardePathMapper as _, SubmitError,
    ValidationMessage, ValidationSource, ValidationTrigger,
};

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, Default)]
struct GardeContext;

#[cfg(feature = "garde-adapter")]
struct TestGardeMessageProvider;

#[cfg(feature = "garde-adapter")]
impl gpui_form::typed::GardeMessageProvider for TestGardeMessageProvider {
    fn message(rule: gpui_form::typed::GardeRule) -> ValidationMessage {
        match rule {
            gpui_form::typed::GardeRule::RequiredNotSet => {
                ValidationMessage::key("validation-required").with_param("rule", "required")
            }
            rule => <gpui_form::typed::DefaultGardeMessageProvider as gpui_form::typed::GardeMessageProvider>::message(
                rule,
            ),
        }
    }
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(context(GardeContext))]
#[form(validation(adapter = "garde", messages = TestGardeMessageProvider))]
struct GardeInput {
    #[form(validate(on_submit))]
    #[garde(required)]
    value: Option<String>,
}

#[cfg(feature = "garde-adapter")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(validation(adapter = "garde"))]
struct InvalidGardePathInput {
    value: String,
}

#[cfg(feature = "garde-adapter")]
impl garde::Validate for InvalidGardePathInput {
    type Context = ();

    fn validate_into(
        &self,
        _context: &Self::Context,
        parent: &mut dyn FnMut() -> garde::Path,
        report: &mut garde::Report,
    ) {
        report.append(
            parent().join("unknown"),
            garde::Error::new("unmappable path"),
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
#[form(store = ProfileFormStore)]
struct ProfileInput {
    #[form(required, validate(on_change, on_blur, on_submit))]
    name: String,
    enabled: bool,
    port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
#[form(store = GenericFormStore)]
struct GenericInput<T>
where
    T: Clone + PartialEq + 'static,
{
    value: T,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct ChildInput {
    #[form(required)]
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
struct RowInput {
    row_id: u64,
    #[form(required)]
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
#[form(store = ParentFormStore)]
struct ParentInput {
    #[form(group)]
    child: ChildInput,
    #[form(array(id = "row_id"))]
    rows: Vec<RowInput>,
}

#[gpui::test]
fn derive_generates_one_typed_model_and_typed_fields(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ProfileFormStore::from_value(
                ProfileInput {
                    name: "OpenAI".into(),
                    enabled: true,
                    port: 443,
                },
                cx,
            )
        })
    });
    let name = ProfileFormStore::name_field(&form);
    let port = ProfileFormStore::port_field(&form);

    cx.update(|cx| {
        assert_eq!(name.value(cx).unwrap(), "OpenAI");
        assert_eq!(port.value(cx).unwrap(), 443);
        name.set("Anthropic".into(), cx).unwrap();
        port.set(8443, cx).unwrap();
        let form = form.read(cx);
        assert_eq!(form.value().name, "Anthropic");
        assert_eq!(form.value().port, 8443);
        assert!(form.is_dirty());
        assert!(ProfileInputField::Name.schema().is_required());
        assert_eq!(
            ProfileInputField::ALL
                .iter()
                .map(|field| field.schema().name())
                .collect::<Vec<_>>(),
            vec!["name", "enabled", "port"]
        );
    });
}

#[gpui::test]
fn derive_preserves_generics_where_clauses_and_custom_store_names(cx: &mut TestAppContext) {
    let form =
        cx.update(|cx| cx.new(|cx| GenericFormStore::from_value(GenericInput { value: 7u32 }, cx)));

    cx.update(|cx| {
        let field = GenericFormStore::value_field(&form);
        assert_eq!(field.value(cx).unwrap(), 7);
        field.set(9, cx).unwrap();
        assert_eq!(form.read(cx).value().value, 9);
    });
}

#[gpui::test]
fn replace_reset_and_rebase_use_the_typed_baseline(cx: &mut TestAppContext) {
    let initial = ProfileInput {
        name: "initial".into(),
        enabled: true,
        port: 443,
    };
    let form = cx.update(|cx| cx.new(|cx| ProfileFormStore::from_value(initial.clone(), cx)));

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.replace(
                ProfileInput {
                    name: "replacement".into(),
                    enabled: false,
                    port: 80,
                },
                cx,
            );
            assert!(form.is_dirty());
            form.reset(cx);
            assert_eq!(form.value(), &initial);
            assert!(!form.is_dirty());
            form.rebase(
                ProfileInput {
                    name: "saved".into(),
                    enabled: false,
                    port: 8080,
                },
                cx,
            );
            assert!(!form.is_dirty());
            assert_eq!(form.baseline(), form.value());
        });
    });
}

#[gpui::test]
fn form_revision_and_conditional_rebase_follow_the_frozen_contract(cx: &mut TestAppContext) {
    let initial = ProfileInput {
        name: "initial".into(),
        enabled: true,
        port: 443,
    };
    let form = cx.update(|cx| cx.new(|cx| ProfileFormStore::from_value(initial.clone(), cx)));
    let name = ProfileFormStore::name_field(&form);

    cx.update(|cx| {
        assert_eq!(form.read(cx).revision(), FormRevision::INITIAL);

        name.set("initial".into(), cx).unwrap();
        assert_eq!(form.read(cx).revision(), FormRevision::INITIAL);

        form.update(cx, |form, cx| form.replace(initial.clone(), cx));
        assert_eq!(form.read(cx).revision().get(), 1);
        form.update(cx, |form, cx| form.reset(cx));
        assert_eq!(form.read(cx).revision().get(), 2);
        form.update(cx, |form, cx| form.rebase(initial.clone(), cx));
        assert_eq!(form.read(cx).revision().get(), 3);

        let before = form.read(cx).value().clone();
        let stale = FormRevision::INITIAL;
        assert!(!form.update(cx, |form, cx| {
            form.rebase_if_revision(
                stale,
                ProfileInput {
                    name: "stale".into(),
                    enabled: false,
                    port: 80,
                },
                cx,
            )
        }));
        assert_eq!(form.read(cx).revision().get(), 3);
        assert_eq!(form.read(cx).value(), &before);

        let expected = form.read(cx).revision();
        assert!(form.update(cx, |form, cx| {
            form.rebase_if_revision(expected, before.clone(), cx)
        }));
        assert_eq!(form.read(cx).revision().get(), 4);
        assert!(!form.update(cx, |form, cx| {
            form.rebase_if_revision(expected, before, cx)
        }));
    });
}

#[gpui::test]
fn duplicate_identified_items_are_unavailable_and_block_submit(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ParentFormStore::from_value(
                ParentInput {
                    child: ChildInput {
                        value: "child".into(),
                    },
                    rows: vec![
                        RowInput {
                            row_id: 41,
                            value: "first".into(),
                        },
                        RowInput {
                            row_id: 41,
                            value: "second".into(),
                        },
                    ],
                },
                cx,
            )
        })
    });
    let row = ParentFormStore::rows_item(&form, FormItemId::new(41));

    cx.update(|cx| {
        assert!(matches!(
            row.value(cx),
            Err(FormFieldError::ValueUnavailable)
        ));
        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(matches!(result, Err(SubmitError::Validation(_))));
        assert!(
            form.read(cx)
                .validation_report()
                .issues()
                .iter()
                .any(|issue| issue.code == "duplicate_item_id")
        );
    });
}

#[test]
fn garde_paths_map_array_indices_to_stable_ids_after_reorder() {
    let mut input = ParentInput {
        child: ChildInput {
            value: "child".into(),
        },
        rows: vec![
            RowInput {
                row_id: 41,
                value: "first".into(),
            },
            RowInput {
                row_id: 99,
                value: "second".into(),
            },
        ],
    };

    assert_eq!(
        input.map_garde_path("child.value").unwrap(),
        FieldPath::field("child").join_field("value")
    );
    assert_eq!(
        input.map_garde_path("rows[1].value").unwrap().segments(),
        &[
            FieldPathSegment::Field("rows".into()),
            FieldPathSegment::Item(99u64.into()),
            FieldPathSegment::Field("value".into()),
        ]
    );

    input.rows.swap(0, 1);
    assert_eq!(
        input.map_garde_path("rows[0].value").unwrap().segments(),
        &[
            FieldPathSegment::Field("rows".into()),
            FieldPathSegment::Item(99u64.into()),
            FieldPathSegment::Field("value".into()),
        ]
    );
    assert_eq!(
        input.map_garde_path("rows[1].value").unwrap().segments(),
        &[
            FieldPathSegment::Field("rows".into()),
            FieldPathSegment::Item(41u64.into()),
            FieldPathSegment::Field("value".into()),
        ]
    );
    assert!(matches!(
        input.map_garde_path("unknown"),
        Err(GardePathError::UnknownField { .. })
    ));
    assert!(matches!(
        input.map_garde_path("rows[not-an-index].value"),
        Err(GardePathError::InvalidIndex { .. })
    ));
    assert!(matches!(
        input.map_garde_path("rows[2].value"),
        Err(GardePathError::IndexOutOfBounds { .. })
    ));
}

#[gpui::test]
fn prepare_submit_validates_the_current_typed_model(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ProfileFormStore::from_value(
                ProfileInput {
                    name: String::new(),
                    enabled: true,
                    port: 443,
                },
                cx,
            )
        })
    });

    cx.update(|cx| {
        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(result.is_err());
        assert_eq!(
            ProfileFormStore::name_field(&form).errors(cx).unwrap()[0].message,
            ValidationMessage::key("gpui-form-error-required")
        );
    });
}

#[gpui::test]
fn prepare_submit_validates_required_fields_inside_groups_and_arrays(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ParentFormStore::from_value(
                ParentInput {
                    child: ChildInput {
                        value: String::new(),
                    },
                    rows: vec![RowInput {
                        row_id: 41,
                        value: String::new(),
                    }],
                },
                cx,
            )
        })
    });

    cx.update(|cx| {
        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(matches!(result, Err(SubmitError::Validation(_))));

        let child_value = ChildInputFormStore::value_in(ParentFormStore::child_field(&form));
        let row_value =
            RowInputFormStore::value_in(ParentFormStore::rows_item(&form, FormItemId::new(41)));
        assert_eq!(
            child_value.errors(cx).unwrap()[0].message,
            ValidationMessage::key("gpui-form-error-required")
        );
        assert_eq!(
            row_value.errors(cx).unwrap()[0].message,
            ValidationMessage::key("gpui-form-error-required")
        );
    });
}

#[gpui::test]
fn typed_user_setter_runs_on_change_validation(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ProfileFormStore::from_value(
                ProfileInput {
                    name: "initial".into(),
                    enabled: true,
                    port: 443,
                },
                cx,
            )
        })
    });
    let name = ProfileFormStore::name_field(&form);

    cx.update(|cx| {
        name.set_user_value(String::new(), cx).unwrap();
        let errors = name.errors(cx).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            ValidationMessage::key("gpui-form-error-required")
        );
    });
}

#[gpui::test]
fn async_validation_replaces_the_previous_task_and_result(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            ProfileFormStore::from_value(
                ProfileInput {
                    name: "OpenAI".into(),
                    enabled: true,
                    port: 443,
                },
                cx,
            )
        })
    });
    let field = ProfileFormStore::name_field(&form);

    cx.update(|cx| {
        field
            .start_async_validation(
                "availability",
                ValidationTrigger::Change,
                |_| std::future::pending::<Result<(), AsyncValidationIssue>>(),
                cx,
            )
            .unwrap();
        field
            .start_async_validation(
                "availability",
                ValidationTrigger::Change,
                |_| async {
                    Err(AsyncValidationIssue::new(
                        "name_taken",
                        ValidationMessage::key("name-taken"),
                    ))
                },
                cx,
            )
            .unwrap();
        assert!(form.read(cx).is_validating());
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!form.read(cx).is_validating());
        assert_eq!(field.errors(cx).unwrap()[0].code, "name_taken");
    });
}

#[cfg(feature = "garde-adapter")]
#[gpui::test]
fn garde_uses_typed_context_and_stores_semantic_messages(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            GardeInputFormStore::from_value_with_validation_context(
                GardeInput { value: None },
                GardeContext,
                cx,
            )
        })
    });

    cx.update(|cx| {
        form.update(cx, |form, cx| {
            form.validate(
                ValidationTrigger::Submit,
                gpui_form::typed::ValidationScope::Form,
                cx,
            );
        });
        assert_eq!(
            form.read(cx).validation_report().issues()[0].message,
            ValidationMessage::key("validation-required").with_param("rule", "required")
        );
    });
}

#[cfg(feature = "garde-adapter")]
#[gpui::test]
fn invalid_garde_paths_become_blocking_internal_issues(cx: &mut TestAppContext) {
    let form = cx.update(|cx| {
        cx.new(|cx| {
            InvalidGardePathInputFormStore::from_value_with_validation_context(
                InvalidGardePathInput {
                    value: "value".into(),
                },
                (),
                cx,
            )
        })
    });

    cx.update(|cx| {
        let result = form.update(cx, |form, cx| form.prepare_submit(cx));
        assert!(result.is_err());
        let form_state = form.read(cx);
        let report = form_state.validation_report();
        let issue = &report.issues()[0];
        assert_eq!(issue.source, ValidationSource::Internal);
        assert_eq!(issue.code, "garde_path_mapping");
        assert!(matches!(
            &issue.message,
            ValidationMessage::Key { key, params }
                if key == "gpui-form-error-internal"
                    && params.contains_key("path")
                    && params.contains_key("reason")
        ));
    });
}
