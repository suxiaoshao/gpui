use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, WindowHandle, div,
};
use gpui_component::input::{InputEvent, InputState};
use gpui_form::SubscriptionSet;
use gpui_form_gpui_component::bind_input;

#[derive(Clone, Debug, PartialEq, gpui_form::FormStore)]
#[form(store = AdapterFormStore)]
struct AdapterInput {
    name: String,
}

struct AdapterHarness {
    form: Entity<AdapterFormStore>,
    input: Entity<InputState>,
    subscriptions: SubscriptionSet,
}

impl AdapterHarness {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let form = cx.new(|cx| {
            AdapterFormStore::from_value(
                AdapterInput {
                    name: "initial".to_string(),
                },
                window,
                cx,
            )
        });
        let input = cx.new(|cx| InputState::new(window, cx));
        let mut subscriptions = SubscriptionSet::new();
        subscriptions.extend(
            bind_input(AdapterFormStore::name_handle(&form), &input, window, cx)
                .expect("form and input entities are alive"),
        );

        Self {
            form,
            input,
            subscriptions,
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

    cx.update(|_, cx| {
        assert_eq!(form.read(cx).name_draft(), "initial");
        assert_eq!(input.read(cx).value().as_ref(), "initial");
    });

    // InputState::set_value is intentionally silent. Emitting Change here is
    // the same boundary used by the component's user-edit path, and the
    // adapter must defer the form write until the input update is complete.
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("user", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();

    cx.update(|_, cx| {
        assert_eq!(form.read(cx).name_draft(), "user");
        assert_eq!(input.read(cx).value().as_ref(), "user");
    });

    cx.update(|window, cx| {
        form.update(cx, |form, cx| {
            form.set_name_value(
                "external".to_string(),
                gpui_form::FieldChangeCause::External,
                window,
                cx,
            );
        });
    });

    cx.update(|_, cx| {
        assert_eq!(form.read(cx).name_draft(), "external");
        assert_eq!(input.read(cx).value().as_ref(), "external");
    });
}

#[gpui::test]
fn clearing_caller_subscription_set_stops_component_to_form_sync(cx: &mut TestAppContext) {
    let window = open_harness(cx);
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let root = window.root(&mut cx).expect("adapter harness root");
    let (form, input) = entities(&root, &mut cx);

    cx.update(|_, cx| {
        root.update(cx, |root, _| root.subscriptions.clear());
    });

    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("after-clear", window, cx);
            cx.emit(InputEvent::Change);
        });
    });
    cx.run_until_parked();

    cx.update(|_, cx| {
        assert_eq!(form.read(cx).name_draft(), "initial");
        assert_eq!(input.read(cx).value().as_ref(), "after-clear");
    });
}
