use std::{cell::RefCell, rc::Rc};

use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_form::{FormField as _, FormStore as _};

#[derive(Clone, Debug, PartialEq, Eq)]
struct PortCodec;

impl gpui_form::FieldCodec<u16> for PortCodec {
    type Draft = String;

    fn draft_from_value(value: &u16) -> Self::Draft {
        value.to_string()
    }

    fn parse(draft: &Self::Draft) -> Result<u16, gpui_form::FieldCodecError> {
        draft
            .parse::<u16>()
            .map_err(|_| gpui_form::FieldCodecError::new("parse", "test-port-parse"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
#[form(store = ProfileFormStore)]
struct ProfileInput {
    #[form(required, validate(on_change, on_blur, on_submit))]
    name: String,
    enabled: bool,
    #[form(codec = "PortCodec")]
    port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
#[form(store = ChildFormStore)]
struct ChildInput {
    value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, gpui_form::FormStore)]
#[form(store = ParentFormStore)]
struct ParentInput {
    #[form(group(store = "ChildFormStore"))]
    child: ChildInput,
    #[form(array(store = "ChildFormStore"))]
    rows: Vec<ChildInput>,
}

struct ProfileHarness {
    form: Entity<ProfileFormStore>,
}

struct ParentHarness {
    _form: Entity<ParentFormStore>,
}

impl ProfileHarness {
    fn new(
        input: ProfileInput,
        capture: Rc<RefCell<Option<Entity<ProfileFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| ProfileFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

impl ParentHarness {
    fn new(
        input: ParentInput,
        capture: Rc<RefCell<Option<Entity<ParentFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| ParentFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { _form: form }
    }
}

impl Render for ProfileHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Render for ParentHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn create_profile_form(
    cx: &mut TestAppContext,
    input: ProfileInput,
) -> (Entity<ProfileFormStore>, WindowHandle<ProfileHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| ProfileHarness::new(input, capture_for_window, window, cx))
        })
        .unwrap()
    });
    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

fn create_parent_form(
    cx: &mut TestAppContext,
    input: ParentInput,
) -> (Entity<ParentFormStore>, WindowHandle<ParentHarness>) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| ParentHarness::new(input, capture_for_window, window, cx))
        })
        .unwrap()
    });
    (
        capture.borrow().as_ref().expect("form captured").clone(),
        window,
    )
}

#[gpui::test]
fn derive_exposes_pure_values_drafts_and_handles(cx: &mut TestAppContext) {
    let (form, window) = create_profile_form(
        cx,
        ProfileInput {
            name: "OpenAI".into(),
            enabled: true,
            port: 443,
        },
    );

    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let (name_handle, port_handle) = cx.update(|_window, _cx| {
        (
            ProfileFormStore::name_handle(&form),
            ProfileFormStore::port_handle(&form),
        )
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.name_value(), "OpenAI");
        assert_eq!(form.name_draft(), "OpenAI");
        assert_eq!(form.port_value(), 443);
        assert_eq!(form.port_draft(), "443");
        assert_eq!(name_handle.draft(cx).unwrap(), "OpenAI");
        assert_eq!(port_handle.draft(cx).unwrap(), "443");
        assert!(form.name_required());
    });

    cx.update(|_window, cx| {
        name_handle.set_user_draft("Anthropic".into(), cx).unwrap();
        port_handle.set_user_draft("8443".into(), cx).unwrap();
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.name_value(), "Anthropic");
        assert_eq!(form.name_draft(), "Anthropic");
        assert_eq!(form.port_value(), 8443);
        assert_eq!(form.port_draft(), "8443");
        assert!(form.meta().is_dirty);
    });
}

#[gpui::test]
fn invalid_codec_draft_stays_in_the_form_without_creating_domain_value(cx: &mut TestAppContext) {
    let (form, window) = create_profile_form(
        cx,
        ProfileInput {
            name: "OpenAI".into(),
            enabled: true,
            port: 443,
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("profile harness root");
    let port_handle = cx.update(|_window, _cx| ProfileFormStore::port_handle(&form));

    cx.update(|_window, cx| {
        port_handle.set_user_draft("-".into(), cx).unwrap();
        let form = form.read(cx);
        assert_eq!(form.port_draft(), "-");
        assert_eq!(form.port_value(), 443);
        assert!(form.port.parse_error().is_some());
    });

    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });
    assert!(result.is_err());
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.port_draft(), "-");
        assert_eq!(form.port_value(), 443);
    });
}

#[gpui::test]
fn required_setter_changes_form_metadata_without_component_state(cx: &mut TestAppContext) {
    let (form, window) = create_profile_form(
        cx,
        ProfileInput {
            name: "OpenAI".into(),
            enabled: true,
            port: 443,
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("profile harness root");

    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.set_name_required(false, cx);
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert!(!form.name_required());
        assert_eq!(form.name_value(), "OpenAI");
        assert!(form.name.errors().is_empty());
    });
}

#[gpui::test]
fn replace_from_value_rebases_draft_and_baseline(cx: &mut TestAppContext) {
    let (form, window) = create_profile_form(
        cx,
        ProfileInput {
            name: "OpenAI".into(),
            enabled: true,
            port: 443,
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("profile harness root");

    cx.update(|_window, cx| {
        let handle = ProfileFormStore::name_handle(&form);
        handle.set_user_draft("local edit".into(), cx).unwrap();
    });
    cx.update(|_window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| {
                form.replace_from_value(
                    ProfileInput {
                        name: "Remote".into(),
                        enabled: false,
                        port: 80,
                    },
                    cx,
                );
            });
        });
    });

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.draft().name, "Remote");
        assert_eq!(form.name_draft(), "Remote");
        assert_eq!(form.port_draft(), "80");
        assert!(form.meta().is_pristine());
    });
}

#[gpui::test]
fn group_and_array_keep_child_form_entities_and_values(cx: &mut TestAppContext) {
    let (form, window) = create_parent_form(
        cx,
        ParentInput {
            child: ChildInput {
                value: "one".into(),
            },
            rows: vec![
                ChildInput {
                    value: "two".into(),
                },
                ChildInput {
                    value: "three".into(),
                },
            ],
        },
    );
    let mut cx = VisualTestContext::from_window(window.into(), cx);

    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.child_value().value, "one");
        assert_eq!(form.rows_value()[1].value, "three");
        assert_eq!(form.rows_items().len(), 2);
        assert_eq!(form.child_store().read(cx).draft().value, "one");
    });
}

#[cfg(feature = "form-pipeline")]
#[derive(
    Clone, Debug, PartialEq, Eq, gpui_form::FormStore, garde::Validate, validify::Validify,
)]
#[garde(allow_unvalidated)]
#[form(
    store = PipelineFormStore,
    validation(adapter = "garde"),
    transform(adapter = "validify")
)]
struct PipelineInput {
    #[garde(length(min = 1))]
    #[modify(trim)]
    name: String,
}

#[cfg(feature = "form-pipeline")]
struct PipelineHarness {
    form: Entity<PipelineFormStore>,
}

#[cfg(feature = "form-pipeline")]
impl PipelineHarness {
    fn new(
        input: PipelineInput,
        capture: Rc<RefCell<Option<Entity<PipelineFormStore>>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let form = cx.new(|cx| PipelineFormStore::from_value(input, window, cx));
        capture.borrow_mut().replace(form.clone());
        Self { form }
    }
}

#[cfg(feature = "form-pipeline")]
impl Render for PipelineHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[cfg(feature = "form-pipeline")]
#[gpui::test]
fn pipeline_uses_pure_form_draft_for_transform_and_validation(cx: &mut TestAppContext) {
    let capture = Rc::new(RefCell::new(None));
    let capture_for_window = capture.clone();
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| {
                PipelineHarness::new(
                    PipelineInput { name: "   ".into() },
                    capture_for_window,
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    let form = capture.borrow().as_ref().expect("form captured").clone();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("pipeline harness root");

    let result = cx.update(|window, cx| {
        root.update(cx, |root, cx| {
            root.form.update(cx, |form, cx| form.submit(window, cx))
        })
    });
    assert!(result.is_err());
    cx.update(|_window, cx| {
        let form = form.read(cx);
        assert_eq!(form.name_draft(), "");
        assert_eq!(form.draft().name, "");
    });
}
