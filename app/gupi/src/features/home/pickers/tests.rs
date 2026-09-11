use super::{ModelKey, ModelOption, Picker, PickerEvent, Projection};
use gpui_kit::component::IndexPath;
use gpui_kit::component::{list::ListEvent, slider::SliderEvent};
use gpui_kit::{AppContext as _, MouseButton, TestAppContext, VisualTestContext, point, px};
use pi_rpc::protocol::Model;
use std::cell::RefCell;
use std::rc::Rc;

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
fn thinking_drag_commits_only_on_release_and_ignores_stale_capabilities(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::foundation::i18n::apply(crate::state::config::AppLanguage::Chinese, cx);
    });
    let window = cx
        .update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| Picker::new(window, cx))
            })
        })
        .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let picker = window.root(&mut cx).unwrap();
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
            p.sync_projection(projection(&["off", "low", "high"], "high"), window, cx);
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
            p.sync_projection(loading, window, cx);
            p.slider_event(
                p.slider.clone(),
                &SliderEvent::Release(2.0.into()),
                window,
                cx,
            );
            p.sync_projection(projection(&["off", "low"], "low"), window, cx);
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
    let window = cx
        .update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| Picker::new(window, cx))
            })
        })
        .unwrap();
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let picker = window.root(&mut cx).unwrap();
    let writes = Rc::new(RefCell::new(0));
    let output = writes.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&picker, move |_, _: &PickerEvent, _| {
            *output.borrow_mut() += 1
        })
    });
    cx.update(|window, cx| {
        picker.update(cx, |p, cx| {
            p.sync_projection(projection(&["off", "high"], "high"), window, cx);
            p.open = true;
            p.show_models(window, cx);
            p.list
                .update(cx, |list, cx| list.set_query("beta", window, cx));
            let mut data = projection(&["off", "high"], "high");
            data.disabled = true;
            p.sync_projection(data, window, cx);
            assert_eq!(
                p.list
                    .read(cx)
                    .delegate()
                    .item(IndexPath::default())
                    .unwrap()
                    .title,
                "Beta"
            );
            assert_eq!(p.data.selected.as_ref().unwrap().id, "alpha");
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
    let (picker, cx) = cx.add_window_view(Picker::new);
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
            p.sync_projection(projection(&["off", "low", "high"], "high"), window, cx);
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
    let (picker, cx) = cx.add_window_view(Picker::new);
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
            p.sync_projection(projection(&["off", "high"], "high"), window, cx);
            p.open = true;
            p.draft_level = Some("off".into());
            p.refresh(window, cx);
            p.refresh(window, cx);
            assert!(p.draft_level.is_none());
            assert_eq!(p.data.level, "high");
            assert_eq!(p.data.selected.as_ref().unwrap().id, "alpha");
            assert!(p.data.disabled);
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
            p.sync_projection(projection(&["off", "high"], "high"), window, cx);
            assert!(p.can_think());
            assert_eq!(p.slider.read(cx).value().start(), 1.);
        })
    });
}
