use std::{cell::RefCell, rc::Rc};

use gpui::{
    App, AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_component::{
    IndexPath,
    combobox::ComboboxEvent,
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::SelectEvent,
};
#[cfg(feature = "form-pipeline")]
use gpui_form::FormField as _;
use gpui_form::FormStore as _;
use gpui_form::macro_support::GeneratedFormStore;

type StringInputBinding = gpui_form_gpui_component::TextInputBinding<String>;
type I32NumberInputBinding = gpui_form_gpui_component::NumberInputBinding<i32>;
type BoolInputBinding = gpui_form_gpui_component::BoolBinding;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ProviderFormStore)]
struct ProviderInput {
    #[form(
        binding = "StringInputBinding",
        validate(on_change, on_blur, on_submit)
    )]
    name: String,
    #[form(binding = "BoolInputBinding")]
    enabled: bool,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = QuantityFormStore)]
struct QuantityInput {
    #[form(binding = "I32NumberInputBinding")]
    amount: i32,
}

struct ProviderFormHarness {
    form: Entity<ProviderFormStore>,
}

struct QuantityFormHarness {
    form: Entity<QuantityFormStore>,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = RequiredBindingFormStore)]
struct RequiredBindingInput {
    #[form(binding = "RequiredFlagBinding", required)]
    secret: String,
    #[form(component = "value")]
    notes: String,
}

struct RequiredFlagState {
    value: String,
    required: bool,
}

struct RequiredFlagBinding;

impl gpui_form::FormComponentBinding<String> for RequiredFlagBinding {
    type State = RequiredFlagState;
    type Draft = String;

    fn new_state(
        initial: &String,
        options: gpui_form::ComponentStateOptions,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let value = initial.clone();
        cx.new(|_| RequiredFlagState {
            value,
            required: options.required,
        })
    }

    fn draft_from_value(value: &String) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).value.clone()
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: gpui_form::FieldPath,
        _trigger: gpui_form::ValidationTrigger,
        _cx: &App,
    ) -> Result<String, Box<gpui_form::FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &String,
        _cause: gpui_form::FieldChangeCause,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.value = value.clone();
        });
    }

    fn set_required(
        state: &Entity<Self::State>,
        required: bool,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.required = required;
        });
    }

    fn focus(_state: &Entity<Self::State>, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }
}

struct RequiredBindingHarness {
    form: Entity<RequiredBindingFormStore>,
}

impl RequiredBindingHarness {
    fn new(
        input: RequiredBindingInput,
        capture: Rc<RefCell<Option<Entity<RequiredBindingFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| RequiredBindingFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

impl Render for RequiredBindingHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl ProviderFormHarness {
    fn new(
        input: ProviderInput,
        capture: Rc<RefCell<Option<Entity<ProviderFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| ProviderFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

impl QuantityFormHarness {
    fn new(
        input: QuantityInput,
        capture: Rc<RefCell<Option<Entity<QuantityFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| QuantityFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

impl Render for ProviderFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Render for QuantityFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn create_form(
    cx: &mut TestAppContext,
    input: ProviderInput,
) -> (Entity<ProviderFormStore>, WindowHandle<ProviderFormHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| ProviderFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

fn create_quantity_form(
    cx: &mut TestAppContext,
    input: QuantityInput,
) -> (Entity<QuantityFormStore>, WindowHandle<QuantityFormHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| QuantityFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

fn create_required_binding_form(
    cx: &mut TestAppContext,
    input: RequiredBindingInput,
) -> (
    Entity<RequiredBindingFormStore>,
    WindowHandle<RequiredBindingHarness>,
) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| RequiredBindingHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[gpui::test]
fn derive_generates_component_field_store(cx: &mut TestAppContext) {
    let (form, _window) = create_form(
        cx,
        ProviderInput {
            name: "OpenAI".to_string(),
            enabled: true,
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert_eq!(form.field_paths().len(), 2);
        assert_eq!(form.draft().name, "OpenAI");
        assert!(form.draft().enabled);
        assert_eq!(form.name_value(), "OpenAI");
        assert!(form.enabled_value());
        assert_eq!(form.name.core().subscriptions().len(), 1);
        assert_eq!(form.name_state().read(cx).value().as_ref(), "OpenAI");
    });
}

#[gpui::test]
fn required_metadata_is_generated_and_can_update_binding_state(cx: &mut TestAppContext) {
    let (form, window) = create_required_binding_form(
        cx,
        RequiredBindingInput {
            secret: "token".to_string(),
            notes: "optional".to_string(),
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert!(form.secret_required());
        assert!(!form.notes_required());
        assert!(gpui_form::FormField::is_required(&form.secret));
        assert!(form.secret_state().read(cx).required);
        assert!(form.meta().is_pristine());
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("form harness root");
    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.set_secret_required(false, window, cx);
                form.set_notes_required(true, window, cx);
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(!form.secret_required());
        assert!(form.notes_required());
        assert!(!gpui_form::FormField::is_required(&form.secret));
        assert!(!form.secret_state().read(cx).required);
        assert_eq!(form.draft().secret, "token");
        assert!(form.meta().is_pristine());
    });
}

#[gpui::test]
fn bool_field_state_tracks_write_draft(cx: &mut TestAppContext) {
    let (form, window) = create_form(
        cx,
        ProviderInput {
            name: "OpenAI".to_string(),
            enabled: true,
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        let state = gpui_form::FormField::component_state(&form.enabled).expect("bool state");
        assert!(state.read(cx).value());
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.write_draft(
                ProviderInput {
                    name: "OpenAI".to_string(),
                    enabled: false,
                },
                gpui_form::FieldChangeCause::External,
                window,
                cx,
            );
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(!form.enabled_value());
        let state = gpui_form::FormField::component_state(&form.enabled).expect("bool state");
        assert!(!state.read(cx).value());
    });
}

#[gpui::test]
fn submit_rejects_unparsable_number_input(cx: &mut TestAppContext) {
    let (form, window) = create_quantity_form(cx, QuantityInput { amount: 12 });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("quantity form root");
    let amount = cx.update(|_window, cx| form.read(cx).amount_state());
    let initial_revision = cx.update(|_window, cx| form.read(cx).amount.core().revision());

    cx.update(|window, cx| {
        amount.update(cx, |input, cx| {
            input.set_value("-", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().amount, 12);
        assert_eq!(form.amount.draft().as_str(), "-");
        assert!(form.amount.core().revision() > initial_revision);
        assert!(gpui_form::FormField::meta(&form.amount).is_dirty);
        assert!(form.meta().is_dirty);
    });

    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });

    let report = result.expect_err("unparsable number should reject submit");
    assert_eq!(report.field_errors().len(), 1);
    assert_eq!(report.field_errors()[0].code.as_ref(), "parse");
    assert_eq!(
        report.field_errors()[0].source,
        gpui_form::ValidationSource::Internal
    );
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().amount, 12);
        assert_eq!(amount.read(cx).value(), "-");
    });
}

#[gpui::test]
fn number_raw_edit_with_same_typed_value_stays_dirty(cx: &mut TestAppContext) {
    let (form, window) = create_quantity_form(cx, QuantityInput { amount: 12 });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let amount = cx.update(|_window, cx| form.read(cx).amount_state());
    let initial_revision = cx.update(|_window, cx| form.read(cx).amount.core().revision());

    cx.update(|window, cx| {
        amount.update(cx, |input, cx| {
            input.set_value("012", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        let _number_input = gpui_form_gpui_component::number_input::<i32>(&form.amount_state());
        assert_eq!(form.draft().amount, 12);
        assert_eq!(form.amount.default_draft().as_str(), "12");
        assert_eq!(form.amount.draft().as_str(), "012");
        assert!(form.amount.core().revision() > initial_revision);
        assert!(gpui_form::FormField::meta(&form.amount).is_dirty);
        assert!(form.meta().is_dirty);
    });
}

#[gpui::test]
fn number_new_state_applies_type_specific_input_policy(cx: &mut TestAppContext) {
    let (_form, window) = create_quantity_form(cx, QuantityInput { amount: 12 });
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    let (signed, unsigned, float) = cx.update(|window, cx| {
        (
            <gpui_form_gpui_component::NumberInputBinding<i32> as gpui_form::FormComponentBinding<
                i32,
            >>::new_state(&0, gpui_form::ComponentStateOptions::default(), window, cx),
            <gpui_form_gpui_component::NumberInputBinding<u32> as gpui_form::FormComponentBinding<
                u32,
            >>::new_state(&0, gpui_form::ComponentStateOptions::default(), window, cx),
            <gpui_form_gpui_component::NumberInputBinding<f64> as gpui_form::FormComponentBinding<
                f64,
            >>::new_state(&0., gpui_form::ComponentStateOptions::default(), window, cx),
        )
    });

    cx.update(|window, cx| {
        signed.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.insert("-", window, cx);
            assert_eq!(input.value().as_ref(), "-");
            input.insert(".", window, cx);
            assert_eq!(input.value().as_ref(), "-");
        });

        unsigned.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.insert("-", window, cx);
            assert_eq!(input.value().as_ref(), "");
            input.insert("1", window, cx);
            input.insert(".", window, cx);
            assert_eq!(input.value().as_ref(), "1");
        });

        float.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.insert(".", window, cx);
            assert_eq!(input.value().as_ref(), ".");
        });
    });
}

#[gpui::test]
fn number_reset_restores_raw_default(cx: &mut TestAppContext) {
    let (form, window) = create_quantity_form(cx, QuantityInput { amount: 12 });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("quantity form root");
    let amount = cx.update(|_window, cx| form.read(cx).amount_state());

    cx.update(|window, cx| {
        amount.update(cx, |input, cx| {
            input.set_value("012", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
        });
    });

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.reset(window, cx));
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().amount, 12);
        assert_eq!(form.amount.default_draft().as_str(), "12");
        assert_eq!(form.amount.draft().as_str(), "12");
        assert_eq!(amount.read(cx).value(), "12");
        assert!(gpui_form::FormField::errors(&form.amount).is_empty());
        assert!(!gpui_form::FormField::meta(&form.amount).is_dirty);
        assert!(!form.meta().is_dirty);
    });
}

#[gpui::test]
fn number_normalize_writeback_recomputes_raw_dirty(cx: &mut TestAppContext) {
    let (form, window) = create_quantity_form(cx, QuantityInput { amount: 12 });
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("quantity form root");
    let amount = cx.update(|_window, cx| form.read(cx).amount_state());

    cx.update(|window, cx| {
        amount.update(cx, |input, cx| {
            input.set_value("012", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().amount, 12);
        assert_eq!(form.amount.draft().as_str(), "012");
        assert!(gpui_form::FormField::meta(&form.amount).is_dirty);
        assert!(form.meta().is_dirty);
    });

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.write_draft(
                    QuantityInput { amount: 12 },
                    gpui_form::FieldChangeCause::NormalizeOnSubmit,
                    window,
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().amount, 12);
        assert_eq!(form.amount.default_draft().as_str(), "12");
        assert_eq!(form.amount.draft().as_str(), "12");
        assert_eq!(amount.read(cx).value(), "12");
        assert!(!gpui_form::FormField::meta(&form.amount).is_dirty);
        assert!(!form.meta().is_dirty);
    });
}

#[gpui::test]
fn generated_field_setter_updates_one_field_and_component_state(cx: &mut TestAppContext) {
    let (form, window) = create_form(
        cx,
        ProviderInput {
            name: "OpenAI".to_string(),
            enabled: true,
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("form harness root");
    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.set_enabled_value(false, gpui_form::FieldChangeCause::UserInput, window, cx);
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.name_value(), "OpenAI");
        assert!(!form.enabled_value());
        assert!(gpui_form::FormField::meta(&form.enabled).is_dirty);
        let state = gpui_form::FormField::component_state(&form.enabled).expect("bool state");
        assert!(!state.read(cx).value());
    });
}

#[gpui::test]
fn generated_error_helpers_apply_and_clear_field_errors(cx: &mut TestAppContext) {
    let (form, window) = create_form(
        cx,
        ProviderInput {
            name: "OpenAI".to_string(),
            enabled: true,
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("form harness root");
    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.apply_field_error(
                    ProviderFormField::Name,
                    gpui_form::FieldError::new_for_field(
                        "name",
                        gpui_form::ValidationTrigger::Submit,
                        gpui_form::ValidationSource::App("derive-test".into()),
                        "required",
                        "name-required",
                    ),
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(gpui_form::FormField::errors(&form.name).len(), 1);
        assert!(gpui_form::FormField::meta(&form.name).is_touched);
        assert!(
            gpui_form::FormField::errors(&form.name)
                .iter()
                .any(gpui_form::FieldError::is_error)
        );
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.clear_field_errors(ProviderFormField::Name, cx);
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(gpui_form::FormField::errors(&form.name).is_empty());
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.apply_field_error(
                    ProviderFormField::Name,
                    gpui_form::FieldError::new_for_field(
                        "name",
                        gpui_form::ValidationTrigger::Submit,
                        gpui_form::ValidationSource::App("derive-test".into()),
                        "required",
                        "name-required",
                    ),
                    cx,
                );
                form.clear_all_errors(cx);
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(gpui_form::FormField::errors(&form.name).is_empty());
    });
}

#[gpui::test]
fn derive_emits_typed_field_events(cx: &mut TestAppContext) {
    let (form, window) = create_form(
        cx,
        ProviderInput {
            name: "OpenAI".to_string(),
            enabled: true,
        },
    );

    assert_eq!(ProviderFormField::Name.key(), "name");
    assert_eq!(
        ProviderFormField::from_key("enabled"),
        Some(ProviderFormField::Enabled)
    );
    assert_eq!(ProviderFormField::from_key("missing"), None);
    assert_eq!(
        ProviderFormEvent::FieldChanged(ProviderFormField::Name).field(),
        ProviderFormField::Name
    );

    let events = Rc::new(RefCell::new(Vec::new()));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("form harness root");
    let _subscription = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let events = events.clone();
            cx.subscribe_in(
                &root.form,
                window,
                move |_this, _form, event: &ProviderFormEvent, _window, _cx| {
                    events.borrow_mut().push(*event);
                },
            )
        })
    });

    let name = cx.update(|_window, cx| form.read(cx).name_state());
    cx.update(|window, cx| {
        name.update(cx, |input, cx| {
            cx.emit(gpui_component::input::InputEvent::Focus);
            input.set_value("Anthropic", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
            cx.emit(gpui_component::input::InputEvent::Blur);
        });
    });

    assert_eq!(
        events.borrow().as_slice(),
        &[
            ProviderFormEvent::FieldFocused(ProviderFormField::Name),
            ProviderFormEvent::FieldChanged(ProviderFormField::Name),
            ProviderFormEvent::FieldBlurred(ProviderFormField::Name),
        ]
    );
}

#[gpui::test]
fn generated_form_store_trait_uses_same_draft(cx: &mut TestAppContext) {
    let (form, _window) = create_form(
        cx,
        ProviderInput {
            name: "Local".to_string(),
            enabled: false,
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert_eq!(
            <ProviderFormStore as GeneratedFormStore<ProviderInput>>::draft(form),
            ProviderInput {
                name: "Local".to_string(),
                enabled: false,
            }
        );
    });
}

#[gpui::test]
fn write_draft_updates_component_state_with_normalize_cause(cx: &mut TestAppContext) {
    let (form, window) = create_form(
        cx,
        ProviderInput {
            name: " OpenAI ".to_string(),
            enabled: true,
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("form harness root");
    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.write_draft(
                    ProviderInput {
                        name: "OpenAI".to_string(),
                        enabled: false,
                    },
                    gpui_form::FieldChangeCause::NormalizeOnSubmit,
                    window,
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().name, "OpenAI");
        assert!(!form.draft().enabled);
        assert_eq!(form.name_state().read(cx).value().as_ref(), "OpenAI");
    });
}

struct BindingTextState {
    value: String,
    disabled: bool,
}

#[derive(Clone, Copy, Debug)]
enum BindingTextEvent {
    Change,
}

impl gpui::EventEmitter<BindingTextEvent> for BindingTextState {}

struct BindingTextBinding;

impl gpui_form::FormComponentBinding<String> for BindingTextBinding {
    type State = BindingTextState;
    type Draft = String;

    fn new_state(
        initial: &String,
        _options: gpui_form::ComponentStateOptions,
        _window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self::State> {
        let value = initial.clone();
        cx.new(|_| BindingTextState {
            value,
            disabled: false,
        })
    }

    fn draft_from_value(value: &String) -> Self::Draft {
        value.clone()
    }

    fn read_draft(state: &Entity<Self::State>, cx: &App) -> Self::Draft {
        state.read(cx).value.clone()
    }

    fn parse_draft(
        draft: &Self::Draft,
        _path: gpui_form::FieldPath,
        _trigger: gpui_form::ValidationTrigger,
        _cx: &App,
    ) -> Result<String, Box<gpui_form::FieldError>> {
        Ok(draft.clone())
    }

    fn write_value(
        state: &Entity<Self::State>,
        value: &String,
        _cause: gpui_form::FieldChangeCause,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.value = value.clone();
        });
    }

    fn set_disabled(
        state: &Entity<Self::State>,
        disabled: bool,
        _window: &mut Window,
        cx: &mut App,
    ) {
        state.update(cx, |state, _| {
            state.disabled = disabled;
        });
    }

    fn focus(_state: &Entity<Self::State>, _window: &mut Window, _cx: &mut App) -> bool {
        false
    }

    fn install_subscriptions<Form>(
        state: Entity<Self::State>,
        sink: gpui_form::FormComponentEventSink<Form>,
        window: &mut Window,
        cx: &mut Context<Form>,
    ) -> gpui_form::SubscriptionSet
    where
        Form: 'static,
    {
        let mut subscriptions = gpui_form::SubscriptionSet::new();
        subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |form, _state, event: &BindingTextEvent, window, cx| match event {
                BindingTextEvent::Change => sink.emit(
                    form,
                    gpui_form::FormComponentEvent::Change(gpui_form::FieldChangeCause::UserInput),
                    window,
                    cx,
                ),
            },
        ));
        subscriptions
    }
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = BindingFormStore)]
struct BindingInput {
    #[form(binding = "BindingTextBinding")]
    token: String,
}

struct BindingFormHarness {
    _form: Entity<BindingFormStore>,
}

impl BindingFormHarness {
    fn new(
        input: BindingInput,
        capture: Rc<RefCell<Option<Entity<BindingFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| BindingFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { _form: form }
    }
}

impl Render for BindingFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn create_binding_form(
    cx: &mut TestAppContext,
    input: BindingInput,
) -> (Entity<BindingFormStore>, WindowHandle<BindingFormHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| BindingFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[gpui::test]
fn derive_installs_binding_component_subscriptions(cx: &mut TestAppContext) {
    let (form, _window) = create_binding_form(
        cx,
        BindingInput {
            token: "secret".to_string(),
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert_eq!(form.token.core().subscriptions().len(), 1);
        assert_eq!(form.token_value(), "secret");
        assert_eq!(form.token_state().read(cx).value, "secret");
    });

    cx.update(|cx| {
        let token_state = form.read(cx).token_state();
        token_state.update(cx, |state, cx| {
            state.value = "changed".to_string();
            cx.emit(BindingTextEvent::Change);
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(form.read(cx).token_value(), "changed");
    });
}

type ProviderChoiceBinding =
    gpui_form_gpui_component::SelectBinding<Option<String>, ProviderChoices>;
type TagsChoiceBinding = gpui_form_gpui_component::ComboboxBinding<Vec<String>, TagsChoices>;

#[derive(Clone, Debug)]
struct ProviderChoices(Vec<String>);

impl Default for ProviderChoices {
    fn default() -> Self {
        Self(vec!["OpenAI".to_string(), "Anthropic".to_string()])
    }
}

impl SearchableListDelegate for ProviderChoices {
    type Item = String;

    fn items_count(&self, section: usize) -> usize {
        self.0.items_count(section)
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.0.item(ix)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.0.position(value)
    }
}

#[derive(Clone, Debug)]
struct TagsChoices(Vec<String>);

impl Default for TagsChoices {
    fn default() -> Self {
        Self(vec!["fast".to_string(), "cheap".to_string()])
    }
}

impl SearchableListDelegate for TagsChoices {
    type Item = String;

    fn items_count(&self, section: usize) -> usize {
        self.0.items_count(section)
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.0.item(ix)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.0.position(value)
    }
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ChoiceFormStore)]
struct ChoiceInput {
    #[form(binding = "ProviderChoiceBinding")]
    provider: Option<String>,
    #[form(binding = "TagsChoiceBinding")]
    tags: Vec<String>,
}

struct ChoiceFormHarness {
    form: Entity<ChoiceFormStore>,
}

impl ChoiceFormHarness {
    fn new(
        input: ChoiceInput,
        capture: Rc<RefCell<Option<Entity<ChoiceFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| ChoiceFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

impl Render for ChoiceFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn create_choice_form(
    cx: &mut TestAppContext,
    input: ChoiceInput,
) -> (Entity<ChoiceFormStore>, WindowHandle<ChoiceFormHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| ChoiceFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[gpui::test]
fn derive_generates_select_and_combobox_field_stores(cx: &mut TestAppContext) {
    let (form, window) = create_choice_form(
        cx,
        ChoiceInput {
            provider: Some("OpenAI".to_string()),
            tags: vec!["fast".to_string()],
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert_eq!(form.provider.core().subscriptions().len(), 1);
        assert_eq!(form.tags.core().subscriptions().len(), 1);
        assert_eq!(form.provider_value(), Some("OpenAI".to_string()));
        assert_eq!(form.tags_value(), vec!["fast".to_string()]);
        assert_eq!(
            form.provider_state().read(cx).selected_value(),
            Some(&"OpenAI".to_string())
        );
        assert_eq!(
            form.tags_state().read(cx).selected_values(),
            vec!["fast".to_string()]
        );
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let provider = cx.update(|_window, cx| form.read(cx).provider_state());
    cx.update(|window, cx| {
        provider.update(cx, |select, cx| {
            select.set_selected_value(&"Anthropic".to_string(), window, cx);
            cx.emit(SelectEvent::<ProviderChoices>::Confirm(Some(
                "Anthropic".to_string(),
            )));
        });
    });

    let tags = cx.update(|_window, cx| form.read(cx).tags_state());
    cx.update(|window, cx| {
        tags.update(cx, |combobox, cx| {
            combobox.set_selected_indices(vec![IndexPath::default().row(1)], window, cx);
            cx.emit(ComboboxEvent::<TagsChoices>::Change(vec![
                "cheap".to_string(),
            ]));
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().provider, Some("Anthropic".to_string()));
        assert_eq!(form.draft().tags, vec!["cheap".to_string()]);
    });
}

#[gpui::test]
fn write_draft_updates_select_and_combobox_component_state(cx: &mut TestAppContext) {
    let (form, window) = create_choice_form(
        cx,
        ChoiceInput {
            provider: Some("OpenAI".to_string()),
            tags: vec!["fast".to_string()],
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("choice form harness root");
    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.write_draft(
                    ChoiceInput {
                        provider: Some("Anthropic".to_string()),
                        tags: vec!["cheap".to_string()],
                    },
                    gpui_form::FieldChangeCause::NormalizeOnSubmit,
                    window,
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(
            form.provider_state().read(cx).selected_value(),
            Some(&"Anthropic".to_string())
        );
        assert_eq!(
            form.tags_state().read(cx).selected_values(),
            vec!["cheap".to_string()]
        );
    });
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = ProfileFormStore)]
struct ProfileInput {
    #[form(binding = "StringInputBinding")]
    nickname: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = AccountFormStore)]
struct AccountInput {
    #[form(component = "group", store = "ProfileFormStore")]
    profile: ProfileInput,
    enabled: bool,
}

struct AccountFormHarness {
    _form: Entity<AccountFormStore>,
}

impl AccountFormHarness {
    fn new(
        input: AccountInput,
        capture: Rc<RefCell<Option<Entity<AccountFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| AccountFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { _form: form }
    }
}

impl Render for AccountFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn create_account_form(
    cx: &mut TestAppContext,
    input: AccountInput,
) -> (Entity<AccountFormStore>, WindowHandle<AccountFormHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| AccountFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[gpui::test]
fn group_store_tracks_child_draft_and_subscriptions(cx: &mut TestAppContext) {
    let (form, window) = create_account_form(
        cx,
        AccountInput {
            profile: ProfileInput {
                nickname: "Ada".to_string(),
            },
            enabled: true,
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert_eq!(form.profile.subscriptions().len(), 1);
        assert_eq!(form.draft().profile.nickname, "Ada");
        assert!(form.draft().enabled);
        assert_eq!(
            form.profile_value(),
            ProfileInput {
                nickname: "Ada".to_string()
            }
        );
        assert!(form.enabled_value());
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let child = cx.update(|_window, cx| form.read(cx).profile_store());
    let nickname = cx.update(|_window, cx| child.read(cx).nickname_state());
    cx.update(|window, cx| {
        nickname.update(cx, |input, cx| {
            input.set_value("Grace", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().profile.nickname, "Grace");
        assert!(form.profile.field_meta().is_dirty);
        assert!(form.meta().is_dirty);
    });
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = HeaderFormStore)]
struct HeaderInput {
    #[form(binding = "StringInputBinding")]
    key: String,
}

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = HeaderListFormStore)]
struct HeaderListInput {
    #[form(component = "array", store = "HeaderFormStore")]
    headers: Vec<HeaderInput>,
}

struct HeaderListHarness {
    form: Entity<HeaderListFormStore>,
}

impl HeaderListHarness {
    fn new(
        input: HeaderListInput,
        capture: Rc<RefCell<Option<Entity<HeaderListFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| HeaderListFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

impl Render for HeaderListHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn create_header_list_form(
    cx: &mut TestAppContext,
    input: HeaderListInput,
) -> (Entity<HeaderListFormStore>, WindowHandle<HeaderListHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| HeaderListHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[gpui::test]
fn array_store_tracks_child_drafts_and_preserves_ids_on_reorder(cx: &mut TestAppContext) {
    let (form, window) = create_header_list_form(
        cx,
        HeaderListInput {
            headers: vec![
                HeaderInput {
                    key: "a".to_string(),
                },
                HeaderInput {
                    key: "b".to_string(),
                },
            ],
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert_eq!(
            form.headers.ids(),
            vec![gpui_form::FormItemId::new(1), gpui_form::FormItemId::new(2)]
        );
        assert_eq!(form.headers_items()[0].subscriptions().len(), 1);
        assert_eq!(form.headers_value()[0].key, "a");
        assert_eq!(form.draft().headers[0].key, "a");
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let first_child = cx.update(|_window, cx| form.read(cx).headers_items()[0].item.store());
    let first_key = cx.update(|_window, cx| first_child.read(cx).key_state());
    cx.update(|window, cx| {
        first_key.update(cx, |input, cx| {
            input.set_value("aa", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
        });
    });
    cx.update(|_window, cx| {
        assert_eq!(form.read(cx).draft().headers[0].key, "aa");
        assert!(form.read(cx).headers.meta().is_dirty);
    });

    let root = window.root(&mut cx).expect("header list root");
    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            let appended = root.form.update(cx, |form, cx| {
                form.headers_append(
                    HeaderInput {
                        key: "c".to_string(),
                    },
                    window,
                    cx,
                )
            });
            assert_eq!(appended, gpui_form::FormItemId::new(3));
        });
    });
    cx.update(|_window, cx| {
        assert_eq!(
            form.read(cx).headers.ids(),
            vec![
                gpui_form::FormItemId::new(1),
                gpui_form::FormItemId::new(2),
                gpui_form::FormItemId::new(3)
            ]
        );
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.headers_move(0, 2, cx).unwrap();
            });
        });
    });
    cx.update(|_window, cx| {
        assert_eq!(
            form.read(cx).headers.ids(),
            vec![
                gpui_form::FormItemId::new(2),
                gpui_form::FormItemId::new(3),
                gpui_form::FormItemId::new(1)
            ]
        );
        assert_eq!(form.read(cx).draft().headers[2].key, "aa");
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            let removed = root
                .form
                .update(cx, |form, cx| form.headers_remove(1, cx).unwrap());
            assert_eq!(removed.id, gpui_form::FormItemId::new(3));
        });
    });
    cx.update(|_window, cx| {
        assert_eq!(
            form.read(cx).headers.ids(),
            vec![gpui_form::FormItemId::new(2), gpui_form::FormItemId::new(1)]
        );
    });
}

#[gpui::test]
fn array_store_removes_by_id_and_returns_values_with_id(cx: &mut TestAppContext) {
    let (form, window) = create_header_list_form(
        cx,
        HeaderListInput {
            headers: vec![
                HeaderInput {
                    key: "a".to_string(),
                },
                HeaderInput {
                    key: "b".to_string(),
                },
            ],
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert_eq!(
            form.headers_values_with_id(),
            vec![
                gpui_form::FormRowValue {
                    id: gpui_form::FormItemId::new(1),
                    value: HeaderInput {
                        key: "a".to_string()
                    }
                },
                gpui_form::FormRowValue {
                    id: gpui_form::FormItemId::new(2),
                    value: HeaderInput {
                        key: "b".to_string()
                    }
                }
            ]
        );
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("header list root");
    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            let removed = root.form.update(cx, |form, cx| {
                form.headers_remove_id(gpui_form::FormItemId::new(1), cx)
                    .expect("row removed by id")
            });
            assert_eq!(removed.id, gpui_form::FormItemId::new(1));
            assert!(
                root.form
                    .update(cx, |form, cx| form
                        .headers_remove_id(gpui_form::FormItemId::new(99), cx))
                    .is_none()
            );
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.headers.ids(), vec![gpui_form::FormItemId::new(2)]);
        assert_eq!(
            form.headers_values_with_id(),
            vec![gpui_form::FormRowValue {
                id: gpui_form::FormItemId::new(2),
                value: HeaderInput {
                    key: "b".to_string()
                }
            }]
        );
    });
}

#[gpui::test]
fn array_store_tracks_structural_dirty_against_default_values(cx: &mut TestAppContext) {
    let (form, window) = create_header_list_form(
        cx,
        HeaderListInput {
            headers: vec![
                HeaderInput {
                    key: "a".to_string(),
                },
                HeaderInput {
                    key: "b".to_string(),
                },
            ],
        },
    );

    cx.update(|cx| {
        let form = form.read(cx);
        assert!(form.headers.meta().is_pristine());
        assert!(form.headers.meta().is_default_value);
        assert!(form.meta().is_pristine());
    });

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("header list root");

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.headers_append(HeaderInput { key: String::new() }, window, cx);
            });
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(form.headers.meta().is_dirty);
        assert!(form.meta().is_dirty);
        assert!(!form.headers.meta().is_default_value);
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form
                .update(cx, |form, cx| form.headers_remove(2, cx).unwrap());
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(form.headers.meta().is_pristine());
        assert!(form.headers.meta().is_touched);
        assert!(form.headers.meta().is_default_value);
        assert!(form.meta().is_pristine());
        assert!(form.meta().is_touched);
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.headers_move(0, 1, cx).unwrap();
            });
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.headers_value()[0].key, "b");
        assert!(form.headers.meta().is_dirty);
        assert!(form.meta().is_dirty);
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.headers_move(1, 0, cx).unwrap();
            });
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.headers_value()[0].key, "a");
        assert!(form.headers.meta().is_pristine());
        assert!(form.headers.meta().is_touched);
        assert!(form.meta().is_pristine());
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form
                .update(cx, |form, cx| form.headers_remove(0, cx).unwrap());
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.headers_value()[0].key, "b");
        assert!(form.headers.meta().is_dirty);
        assert!(form.meta().is_dirty);
    });

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.headers_reset_items(
                    vec![HeaderInput {
                        key: "z".to_string(),
                    }],
                    window,
                    cx,
                );
            });
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.headers_value()[0].key, "z");
        assert!(form.headers.meta().is_pristine());
        assert!(!form.headers.meta().is_touched);
        assert!(form.headers.meta().is_default_value);
        assert!(form.meta().is_pristine());
    });

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.headers_replace(Vec::<HeaderInput>::new(), window, cx);
            });
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(form.headers_value().is_empty());
        assert!(form.headers.meta().is_dirty);
        assert!(form.headers.meta().is_touched);
        assert!(!form.headers.meta().is_default_value);
        assert!(form.meta().is_dirty);
    });

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.headers_replace(
                    vec![HeaderInput {
                        key: "z".to_string(),
                    }],
                    window,
                    cx,
                );
            });
        });
    });
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.headers_value()[0].key, "z");
        assert!(form.headers.meta().is_pristine());
        assert!(form.headers.meta().is_touched);
        assert!(form.headers.meta().is_default_value);
        assert!(form.meta().is_pristine());
    });
}

#[gpui::test]
fn applying_validation_report_preserves_internal_array_errors(cx: &mut TestAppContext) {
    let (form, window) = create_header_list_form(
        cx,
        HeaderListInput {
            headers: vec![HeaderInput {
                key: "a".to_string(),
            }],
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("header list root");
    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.write_draft(
                    HeaderListInput {
                        headers: vec![
                            HeaderInput {
                                key: "a".to_string(),
                            },
                            HeaderInput {
                                key: "b".to_string(),
                            },
                        ],
                    },
                    gpui_form::FieldChangeCause::NormalizeOnSubmit,
                    window,
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(
            form.headers
                .errors()
                .iter()
                .any(|error| error.code.as_ref() == "array_length_changed")
        );
    });

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                <HeaderListFormStore as GeneratedFormStore<HeaderListInput>>::apply_validation_report(
                    form,
                    &gpui_form::FormValidationReport::empty(),
                    &gpui_form::ValidationScope::Form,
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(
            form.headers
                .errors()
                .iter()
                .any(|error| error.code.as_ref() == "array_length_changed")
        );
    });
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate, validify::Validify)]
#[garde(allow_unvalidated)]
#[form(
    store = NormalizedFormStore,
    validation(adapter = "garde"),
    transform(adapter = "validify")
)]
struct NormalizedInput {
    #[form(binding = "StringInputBinding")]
    #[modify(trim)]
    #[garde(length(min = 1))]
    name: String,
}

#[cfg(feature = "form-pipeline")]
struct NormalizedFormHarness {
    form: Entity<NormalizedFormStore>,
}

#[cfg(feature = "form-pipeline")]
impl NormalizedFormHarness {
    fn new(
        input: NormalizedInput,
        capture: Rc<RefCell<Option<Entity<NormalizedFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| NormalizedFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

#[cfg(feature = "form-pipeline")]
impl Render for NormalizedFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(feature = "form-pipeline")]
fn create_normalized_form(
    cx: &mut TestAppContext,
    input: NormalizedInput,
) -> (
    Entity<NormalizedFormStore>,
    WindowHandle<NormalizedFormHarness>,
) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| NormalizedFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn submit_runs_validify_writeback_before_garde_validation(cx: &mut TestAppContext) {
    let (form, window) = create_normalized_form(
        cx,
        NormalizedInput {
            name: "   ".to_string(),
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("normalized form harness root");
    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });

    assert!(result.is_err());
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().name, "");
        assert_eq!(form.name_state().read(cx).value().as_ref(), "");
    });
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = LiveValidationFormStore, validation(adapter = "garde"))]
struct LiveValidationInput {
    #[form(
        binding = "StringInputBinding",
        validate(on_change, on_blur, on_submit)
    )]
    #[garde(length(min = 3))]
    name: String,
    #[form(binding = "StringInputBinding", validate(on_submit))]
    #[garde(length(min = 3))]
    title: String,
}

#[cfg(feature = "form-pipeline")]
struct LiveValidationFormHarness {
    form: Entity<LiveValidationFormStore>,
}

#[cfg(feature = "form-pipeline")]
impl LiveValidationFormHarness {
    fn new(
        input: LiveValidationInput,
        capture: Rc<RefCell<Option<Entity<LiveValidationFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| LiveValidationFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

#[cfg(feature = "form-pipeline")]
impl Render for LiveValidationFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(feature = "form-pipeline")]
fn create_live_validation_form(
    cx: &mut TestAppContext,
    input: LiveValidationInput,
) -> (
    Entity<LiveValidationFormStore>,
    WindowHandle<LiveValidationFormHarness>,
) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| LiveValidationFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn change_validation_writes_only_the_changed_field_errors(cx: &mut TestAppContext) {
    let (form, window) = create_live_validation_form(
        cx,
        LiveValidationInput {
            name: "valid".to_string(),
            title: "".to_string(),
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let name = cx.update(|_window, cx| form.read(cx).name_state());
    cx.update(|window, cx| {
        name.update(cx, |input, cx| {
            input.set_value("a", window, cx);
            cx.emit(gpui_component::input::InputEvent::Change);
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().name, "a");
        assert_eq!(form.name.errors().len(), 1);
        assert!(form.title.errors().is_empty());
        assert!(
            form.name
                .errors()
                .iter()
                .any(gpui_form::FieldError::is_error)
        );
    });
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn submit_validation_writes_field_errors(cx: &mut TestAppContext) {
    let (form, window) = create_live_validation_form(
        cx,
        LiveValidationInput {
            name: "".to_string(),
            title: "".to_string(),
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("live validation form root");
    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });

    assert!(result.is_err());
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.name.errors().len(), 1);
        assert_eq!(form.title.errors().len(), 1);
        assert!(
            form.name
                .errors()
                .iter()
                .any(gpui_form::FieldError::is_error)
        );
        assert!(
            form.title
                .errors()
                .iter()
                .any(gpui_form::FieldError::is_error)
        );
    });
    cx.update(|window, cx| {
        assert!(root.update(cx, |root, cx| {
            root.form
                .update(cx, |form, cx| form.focus_first_error(window, cx))
        }));
    });
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn reset_clears_form_level_errors(cx: &mut TestAppContext) {
    let (form, window) = create_live_validation_form(
        cx,
        LiveValidationInput {
            name: "valid".to_string(),
            title: "valid".to_string(),
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("live validation form root");
    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                let report = gpui_form::FormValidationReport::new(
                    Vec::new(),
                    vec![gpui_form::FormError::new(
                        gpui_form::ValidationTrigger::Submit,
                        gpui_form::ValidationSource::App("derive-test".into()),
                        "server",
                        "form-server-error",
                    )],
                );
                <LiveValidationFormStore as GeneratedFormStore<LiveValidationInput>>::apply_validation_report(
                    form,
                    &report,
                    &gpui_form::ValidationScope::Form,
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.form_errors().len(), 1);
        assert!(
            form.form_errors()
                .iter()
                .any(gpui_form::FormError::is_error)
        );
    });

    cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.reset(window, cx);
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(form.form_errors().is_empty());
        assert!(form.meta().is_pristine());
    });
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = RequiredProfileFormStore, validation(adapter = "garde"))]
struct RequiredProfileInput {
    #[form(binding = "StringInputBinding", validate(on_submit))]
    #[garde(length(min = 3))]
    nickname: String,
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = RequiredAccountFormStore, validation(adapter = "garde"))]
struct RequiredAccountInput {
    #[form(component = "group", store = "RequiredProfileFormStore")]
    #[garde(dive)]
    profile: RequiredProfileInput,
}

#[cfg(feature = "form-pipeline")]
struct RequiredAccountFormHarness {
    form: Entity<RequiredAccountFormStore>,
}

#[cfg(feature = "form-pipeline")]
impl RequiredAccountFormHarness {
    fn new(
        input: RequiredAccountInput,
        capture: Rc<RefCell<Option<Entity<RequiredAccountFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| RequiredAccountFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

#[cfg(feature = "form-pipeline")]
impl Render for RequiredAccountFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(feature = "form-pipeline")]
fn create_required_account_form(
    cx: &mut TestAppContext,
    input: RequiredAccountInput,
) -> (
    Entity<RequiredAccountFormStore>,
    WindowHandle<RequiredAccountFormHarness>,
) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| RequiredAccountFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn group_submit_validation_writes_child_field_errors(cx: &mut TestAppContext) {
    let (form, window) = create_required_account_form(
        cx,
        RequiredAccountInput {
            profile: RequiredProfileInput {
                nickname: "".to_string(),
            },
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("required account form root");
    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });

    assert!(result.is_err());
    cx.update(|_window, cx| {
        let form = form.read(cx);
        let child = form.profile.store();
        assert_eq!(child.read(cx).nickname.errors().len(), 1);
        assert!(
            child
                .read(cx)
                .nickname
                .errors()
                .iter()
                .any(gpui_form::FieldError::is_error)
        );
    });
    cx.update(|window, cx| {
        assert!(root.update(cx, |root, cx| {
            root.form
                .update(cx, |form, cx| form.focus_first_error(window, cx))
        }));
    });
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = OverlappingProfileFormStore, validation(adapter = "garde"))]
struct OverlappingProfileInput {
    #[form(binding = "StringInputBinding", validate(on_submit))]
    #[garde(length(min = 3))]
    name: String,
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = OverlappingAccountFormStore, validation(adapter = "garde"))]
struct OverlappingAccountInput {
    #[form(binding = "StringInputBinding", validate(on_submit))]
    #[garde(length(min = 3))]
    name: String,
    #[form(component = "group", store = "OverlappingProfileFormStore")]
    #[garde(dive)]
    profile: OverlappingProfileInput,
}

#[cfg(feature = "form-pipeline")]
struct OverlappingAccountFormHarness {
    form: Entity<OverlappingAccountFormStore>,
}

#[cfg(feature = "form-pipeline")]
impl OverlappingAccountFormHarness {
    fn new(
        input: OverlappingAccountInput,
        capture: Rc<RefCell<Option<Entity<OverlappingAccountFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| OverlappingAccountFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

#[cfg(feature = "form-pipeline")]
impl Render for OverlappingAccountFormHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(feature = "form-pipeline")]
fn create_overlapping_account_form(
    cx: &mut TestAppContext,
    input: OverlappingAccountInput,
) -> (
    Entity<OverlappingAccountFormStore>,
    WindowHandle<OverlappingAccountFormHarness>,
) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| OverlappingAccountFormHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn group_validation_does_not_copy_sibling_errors_into_child(cx: &mut TestAppContext) {
    let (form, window) = create_overlapping_account_form(
        cx,
        OverlappingAccountInput {
            name: "".to_string(),
            profile: OverlappingProfileInput {
                name: "valid".to_string(),
            },
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("overlapping account form root");
    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });

    assert!(result.is_err());
    cx.update(|_window, cx| {
        let form = form.read(cx);
        let child = form.profile.store();
        assert_eq!(form.name.errors().len(), 1);
        assert!(child.read(cx).name.errors().is_empty());
    });
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = RequiredHeaderFormStore, validation(adapter = "garde"))]
struct RequiredHeaderInput {
    #[form(binding = "StringInputBinding", validate(on_submit))]
    #[garde(length(min = 1))]
    key: String,
}

#[cfg(feature = "form-pipeline")]
#[derive(Clone, Debug, PartialEq, gpui_form::FormStore, garde::Validate)]
#[garde(allow_unvalidated)]
#[form(store = RequiredHeaderListFormStore, validation(adapter = "garde"))]
struct RequiredHeaderListInput {
    #[form(component = "array", store = "RequiredHeaderFormStore")]
    #[garde(dive)]
    headers: Vec<RequiredHeaderInput>,
}

#[cfg(feature = "form-pipeline")]
struct RequiredHeaderListHarness {
    form: Entity<RequiredHeaderListFormStore>,
}

#[cfg(feature = "form-pipeline")]
impl RequiredHeaderListHarness {
    fn new(
        input: RequiredHeaderListInput,
        capture: Rc<RefCell<Option<Entity<RequiredHeaderListFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| RequiredHeaderListFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

#[cfg(feature = "form-pipeline")]
impl Render for RequiredHeaderListHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(feature = "form-pipeline")]
fn create_required_header_list_form(
    cx: &mut TestAppContext,
    input: RequiredHeaderListInput,
) -> (
    Entity<RequiredHeaderListFormStore>,
    WindowHandle<RequiredHeaderListHarness>,
) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();

    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let capture = capture_for_window.clone();
            cx.new(|cx| RequiredHeaderListHarness::new(input, capture, window, cx))
        })
        .unwrap()
    });

    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn array_submit_validation_writes_indexed_child_errors(cx: &mut TestAppContext) {
    let (form, window) = create_required_header_list_form(
        cx,
        RequiredHeaderListInput {
            headers: vec![
                RequiredHeaderInput {
                    key: "".to_string(),
                },
                RequiredHeaderInput {
                    key: "ok".to_string(),
                },
            ],
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("required header list root");
    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });

    assert!(result.is_err());
    cx.update(|_window, cx| {
        let form = form.read(cx);
        let first = form.headers.items()[0].item.store();
        let second = form.headers.items()[1].item.store();
        assert_eq!(first.read(cx).key.errors().len(), 1);
        assert!(
            first
                .read(cx)
                .key
                .errors()
                .iter()
                .any(gpui_form::FieldError::is_error)
        );
        assert!(second.read(cx).key.errors().is_empty());
    });
    cx.update(|window, cx| {
        assert!(root.update(cx, |root, cx| {
            root.form
                .update(cx, |form, cx| form.focus_first_error(window, cx))
        }));
    });
}
