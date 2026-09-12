use super::{ModelKey, ModelOption, Picker, PickerEvent, Projection};
use gpui_kit::component::IndexPath;
use gpui_kit::component::{list::ListEvent, slider::SliderEvent};
use gpui_kit::{Context, MouseButton, TestAppContext, Window, point, px};
use pi_rpc::protocol::Model;
use std::cell::RefCell;
use std::rc::Rc;

fn source() -> (
    Rc<RefCell<Projection>>,
    impl FnOnce(&mut Window, &mut Context<Picker>) -> Picker,
) {
    let value = Rc::new(RefCell::new(Projection::default()));
    let query = value.clone();
    (value, move |window, cx| {
        Picker::new(move |_| query.borrow().clone(), window, cx)
    })
}
fn sync(
    source: &RefCell<Projection>,
    picker: &mut Picker,
    data: Projection,
    window: &mut Window,
    cx: &mut Context<Picker>,
) {
    *source.borrow_mut() = data;
    picker.sync_controls(window, cx);
}

fn projection(levels: &[&str], current: &str) -> Projection {
    let models: Vec<Model> = serde_json::from_value(serde_json::json!([
        {"provider":"a","id":"alpha","name":"Alpha","reasoning":true},
        {"provider":"b","id":"beta","name":"Beta","reasoning":true}
    ]))
    .unwrap();
    Projection {
        models: models.iter().map(ModelOption::from).collect(),
        selected: Some(ModelKey::from(&models[0])),
        name: "Alpha".into(),
        reasoning: true,
        levels: levels.iter().map(|s| s.to_string()).collect(),
        level: current.into(),
        ..Default::default()
    }
}

#[gpui_kit::test]
fn actions_use_current_business_state_before_controls_are_synchronized(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::foundation::i18n::apply(crate::state::config::AppLanguage::Chinese, cx);
    });
    let (source, new_picker) = source();
    let (picker, cx) = cx.add_window_view(new_picker);
    let events = Rc::new(RefCell::new(0));
    let output = events.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&picker, move |_, _: &PickerEvent, _| {
            *output.borrow_mut() += 1
        })
    });
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            sync(&source, p, projection(&["off", "high"], "high"), window, cx);
            source.borrow_mut().disabled = true;
            p.refresh(window, cx);
            p.commit_level("off".into(), cx);
            p.show_models(window, cx);
            assert!(!p.models_page);
        })
    });
    cx.run_until_parked();
    assert_eq!(*events.borrow(), 0);
}

#[gpui_kit::test]
fn opening_queries_the_source_and_ignored_requests_do_not_leave_loading_flags(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::foundation::i18n::apply(crate::state::config::AppLanguage::Chinese, cx);
    });
    let (source, new_picker) = source();
    let (picker, cx) = cx.add_window_view(new_picker);
    let loads = Rc::new(RefCell::new(0));
    let output = loads.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&picker, move |_, event: &PickerEvent, _| {
            if matches!(event, PickerEvent::Load) {
                *output.borrow_mut() += 1;
            }
        })
    });
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            sync(
                &source,
                p,
                Projection {
                    unloaded: true,
                    models_unloaded: true,
                    ..Default::default()
                },
                window,
                cx,
            );
            p.set_open(true, window, cx);
            p.set_open(false, window, cx);
            p.set_open(true, window, cx);
            assert!(!p.query(cx).models_loading);
        })
    });
    cx.run_until_parked();
    assert_eq!(*loads.borrow(), 2);
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            // An empty successful list is still loaded; reopening does not retry it.
            sync(&source, p, Projection::default(), window, cx);
            p.set_open(false, window, cx);
            p.set_open(true, window, cx);
            assert!(!p.query(cx).models_loading);
        })
    });
    cx.run_until_parked();
    assert_eq!(*loads.borrow(), 2);
}

#[gpui_kit::test]
fn thinking_drag_commits_only_on_release_and_ignores_stale_capabilities(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::foundation::i18n::apply(crate::state::config::AppLanguage::Chinese, cx);
    });
    let (source, new_picker) = source();
    let (picker, cx) = cx.add_window_view(new_picker);
    let writes = Rc::new(RefCell::new(vec![]));
    let output = writes.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&picker, move |_, event: &PickerEvent, _| {
            if let PickerEvent::Thinking(level) = event {
                output.borrow_mut().push(level.clone());
            }
        })
    });
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            sync(
                &source,
                p,
                projection(&["off", "low", "high"], "high"),
                window,
                cx,
            );
            p.open = true;
            for value in [0., 1.] {
                p.slider_event(
                    p.slider.clone(),
                    &SliderEvent::Change(value.into()),
                    window,
                    cx,
                );
            }
            assert_eq!(p.draft_level.as_deref(), Some("low"));
            assert!(writes.borrow().is_empty());
            p.slider_event(
                p.slider.clone(),
                &SliderEvent::Release(1.0.into()),
                window,
                cx,
            );
        })
    });
    cx.run_until_parked();
    assert_eq!(&*writes.borrow(), &["low"]);
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            let mut loading = projection(&["off", "low", "high"], "high");
            loading.disabled = true;
            sync(&source, p, loading, window, cx);
            p.slider_event(
                p.slider.clone(),
                &SliderEvent::Release(2.0.into()),
                window,
                cx,
            );
            sync(&source, p, projection(&["off", "low"], "low"), window, cx);
            assert_eq!(p.slider.read(cx).max_value(), 1.);
            p.slider_event(
                p.slider.clone(),
                &SliderEvent::Change(0.0.into()),
                window,
                cx,
            );
            p.close(window, cx);
            p.slider_event(
                p.slider.clone(),
                &SliderEvent::Release(0.0.into()),
                window,
                cx,
            );
            assert!(p.draft_level.is_none());
        })
    });
    cx.run_until_parked();
    assert_eq!(writes.borrow().len(), 1);
}

#[gpui_kit::test]
fn model_search_survives_refresh_and_cancel_does_not_change_model(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::foundation::i18n::apply(crate::state::config::AppLanguage::Chinese, cx);
    });
    let (source, new_picker) = source();
    let (picker, cx) = cx.add_window_view(new_picker);
    let writes = Rc::new(RefCell::new(0));
    let output = writes.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&picker, move |_, _: &PickerEvent, _| {
            *output.borrow_mut() += 1
        })
    });
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            sync(&source, p, projection(&["off", "high"], "high"), window, cx);
            p.open = true;
            p.show_models(window, cx);
            p.list
                .update(cx, |list, cx| list.set_query("beta", window, cx));
            let mut data = projection(&["off", "high"], "high");
            data.disabled = true;
            sync(&source, p, data, window, cx);
            assert_eq!(
                p.list
                    .read(cx)
                    .delegate()
                    .item(IndexPath::default())
                    .unwrap()
                    .title,
                "Beta"
            );
            assert_eq!(p.query(cx).selected.as_ref().unwrap().id, "alpha");
            p.list.update(cx, |_, cx| cx.emit(ListEvent::Cancel));
        })
    });
    cx.run_until_parked();
    assert_eq!(*writes.borrow(), 0);
}

#[gpui_kit::test]
fn rendered_slider_drag_sends_one_final_level(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::foundation::i18n::apply(crate::state::config::AppLanguage::Chinese, cx);
    });
    let (source, new_picker) = source();
    let (picker, cx) = cx.add_window_view(new_picker);
    let writes = Rc::new(RefCell::new(Vec::new()));
    let output = writes.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&picker, move |_, event: &PickerEvent, _| {
            if let PickerEvent::Thinking(level) = event {
                output.borrow_mut().push(level.clone());
            }
        })
    });
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            sync(
                &source,
                p,
                projection(&["off", "low", "high"], "high"),
                window,
                cx,
            );
            p.open = true;
        });
        window.draw(cx).clear(cx);
    });
    let bounds = cx.update(|_, cx| picker.read(cx).slider.read(cx).bounds());
    assert!(bounds.size.width > px(0.));
    let start = point(bounds.right(), bounds.center().y);
    cx.simulate_mouse_move(start, None, Default::default());
    cx.simulate_mouse_down(start, MouseButton::Left, Default::default());
    for fraction in [0.95, 0.9, 0.75, 0.5] {
        cx.simulate_mouse_move(
            point(bounds.left() + bounds.size.width * fraction, start.y),
            MouseButton::Left,
            Default::default(),
        );
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    assert!(writes.borrow().is_empty());
    cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Default::default());
    cx.run_until_parked();
    assert_eq!(&*writes.borrow(), &["low"]);
}

#[gpui_kit::test]
fn refresh_preserves_model_and_level_and_never_submits_a_draft(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::foundation::i18n::apply(crate::state::config::AppLanguage::Chinese, cx);
    });
    let (source, new_picker) = source();
    let (picker, cx) = cx.add_window_view(new_picker);
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&picker, move |_, event: &PickerEvent, _| {
            output.borrow_mut().push(match event {
                PickerEvent::Refresh => "refresh",
                PickerEvent::Load => "load",
                _ => "write",
            });
        })
    });
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            sync(&source, p, projection(&["off", "high"], "high"), window, cx);
            p.open = true;
            p.draft_level = Some("off".into());
            p.refresh(window, cx);
            let mut loading = projection(&["off", "high"], "high");
            loading.models_loading = true;
            loading.thinking_loading = true;
            sync(&source, p, loading, window, cx);
            p.refresh(window, cx);
            assert!(p.draft_level.is_none());
            assert_eq!(p.query(cx).level, "high");
            assert_eq!(p.query(cx).selected.as_ref().unwrap().id, "alpha");
            assert!(p.query(cx).models_loading);
            p.slider_event(
                p.slider.clone(),
                &SliderEvent::Release(0.0.into()),
                window,
                cx,
            );
        })
    });
    cx.run_until_parked();
    assert_eq!(&*events.borrow(), &["refresh"]);
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            sync(&source, p, projection(&["off", "high"], "high"), window, cx);
            assert!(p.query(cx).can_think());
            assert_eq!(p.slider.read(cx).value().start(), 1.);
        })
    });
}
