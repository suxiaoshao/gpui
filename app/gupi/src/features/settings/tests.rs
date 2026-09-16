use super::{AppConfig, AppLanguage, ConfigController, PiProbeController, SettingsView};
use gpui_form::Form;
use gpui_kit::component::Root;
use gpui_kit::{AppContext, TestAppContext};

#[gpui_kit::test]
fn shared_settings_layout_renders_without_reentrant_entity_access(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let form = cx.new(|_| {
            Form::new(AppConfig {
                pi_command: Some("/custom/onboarding-pi".into()),
                ..Default::default()
            })
        });
        let controller = cx.new(|cx| ConfigController::new(&form, cx));
        controller.read(cx).pi_form.clone().update(cx, |form, cx| {
            form.rebase(
                super::PiSettings {
                    command: Some("/custom/settings-pi".into()),
                },
                cx,
            )
        });
        let draft = cx.new(|_| PiProbeController::new());
        let applied = cx.new(|_| PiProbeController::new());
        let view = cx.new(|cx| {
            SettingsView::new(
                form,
                controller,
                draft,
                applied,
                cx.focus_handle(),
                window,
                cx,
            )
        });
        assert_eq!(
            view.read(cx).input.read(cx).value().as_ref(),
            "/custom/onboarding-pi"
        );
        assert_eq!(
            view.read(cx).pi_input.read(cx).value().as_ref(),
            "/custom/settings-pi"
        );
        Root::new(view, window, cx)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

#[gpui_kit::test]
fn theme_grid_contributes_height_and_wraps_in_settings(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
    });
    let mut fixture = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let controller = cx.new(|cx| ConfigController::new(&form, cx));
        let view = cx.new(|_| ThemeGridFixture {
            controller,
            width: 640.,
        });
        fixture = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let wide = cx.debug_bounds("theme-grid-measure").unwrap().size.height;
    let first = cx.debug_bounds("light-themes-tile-0").unwrap();
    let second = cx.debug_bounds("light-themes-tile-1").unwrap();
    let last = cx.debug_bounds("light-themes-tile-3").unwrap();
    assert_eq!(
        first.top(),
        second.top(),
        "first frame must already have multiple columns"
    );
    assert!(
        last.top() > first.top(),
        "fixture must include multiple rows"
    );
    assert!(
        (first.size.width - last.size.width).abs() < gpui_kit::px(1.),
        "last row must use the same column width: {first:?}, {last:?}"
    );
    let fixture = fixture.unwrap();
    fixture.update(cx, |view, cx| {
        view.width = 320.;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let narrow = cx.debug_bounds("theme-grid-measure").unwrap().size.height;
    assert!(
        wide > gpui_kit::px(88.),
        "theme rows must contribute to the parent height: {wide:?}"
    );
    assert!(
        narrow > wide,
        "narrow settings must wrap and grow vertically: {wide:?}, {narrow:?}"
    );
}
struct ThemeGridFixture {
    controller: gpui_kit::Entity<ConfigController>,
    width: f32,
}
impl gpui_kit::Render for ThemeGridFixture {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{InteractiveElement, ParentElement, Styled};
        let mut choices = app_theme::theme_choices(
            gpui_kit::component::ThemeRegistry::global(cx),
            gpui_kit::component::ThemeMode::Light,
            &[],
        );
        choices.truncate(4);
        gpui_kit::div()
            .w(gpui_kit::px(self.width))
            .debug_selector(|| "theme-grid-measure".into())
            .child(super::preferences::theme_grid(
                (
                    "light-themes",
                    gpui_kit::component::ThemeMode::Light,
                    choices,
                ),
                &AppConfig::default(),
                &self.controller,
                None,
                false,
                cx,
            ))
    }
}

#[gpui_kit::test]
fn every_settings_page_renders_with_resources_at_narrow_width(cx: &mut TestAppContext) {
    use crate::foundation::pi_resources::{Catalog, Kind, Package, Resource};
    use gpui_operation::{Complete, Load, Transition};
    cx.update(|cx| {
        gpui_kit::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
    });
    for page in 0..8 {
        let (_, window_cx) = cx.add_window_view(|window, cx| {
            let form = cx.new(|_| Form::new(AppConfig::default()));
            let controller = cx.new(|cx| ConfigController::new(&form, cx));
            let draft = cx.new(|_| PiProbeController::new());
            let applied = cx.new(|_| PiProbeController::new());
            let settings = cx.new(|cx| {
                SettingsView::new(
                    form,
                    controller,
                    draft,
                    applied,
                    cx.focus_handle(),
                    window,
                    cx,
                )
            });
            let resources = settings.read(cx).resources.read(cx).controller.clone();
            resources.update(cx, |owner, _| {
                let root = std::path::PathBuf::from("/tmp/settings-layout-fixture");
                owner.catalog.transition(Load(gpui_kit::Task::ready(())));
                owner.catalog.transition(Complete(Ok(Catalog {
                    root: root.clone(),
                    packages: vec![Package {
                        source: "local-review-package".into(),
                        path: root.clone(),
                        version: Some("1.0.0".into()),
                    }],
                    resources: [Kind::Extension, Kind::Skill, Kind::Prompt]
                        .into_iter()
                        .map(|kind| Resource {
                            kind,
                            path: root.join("example.md"),
                            base: root.clone(),
                            package: None,
                            name: "测试资源".into(),
                            description: "用于布局检查的说明".into(),
                            enabled: true,
                            editable: kind != Kind::Extension,
                        })
                        .collect(),
                    warnings: vec![],
                })));
            });
            let view = cx.new(|_| SettingsPageFixture { settings, page });
            Root::new(view, window, cx)
        });
        window_cx.simulate_resize(gpui_kit::size(gpui_kit::px(760.), gpui_kit::px(640.)));
        window_cx.update(|window, cx| window.draw(cx).clear(cx));
        if page == 0 {
            for width in [640., 1600.] {
                window_cx.simulate_resize(gpui_kit::size(gpui_kit::px(width), gpui_kit::px(640.)));
                window_cx.update(|window, cx| window.draw(cx).clear(cx));
                let path = window_cx.debug_bounds("settings-config-path").unwrap();
                let reload = window_cx.debug_bounds("settings-config-reload").unwrap();
                let open = window_cx.debug_bounds("settings-config-open").unwrap();
                assert_eq!(path.center().y, reload.center().y);
                assert_eq!(reload.center().y, open.center().y);
                assert!(path.right() <= reload.left() && reload.right() <= open.left());
                assert!(open.right() <= gpui_kit::px(width));
                if width == 1600. {
                    assert!(
                        path.left() > gpui_kit::px(width / 2.),
                        "configuration controls belong to the right-hand setting field"
                    );
                }
            }
        }
        if page == 3 {
            for width in [760., 1600.] {
                window_cx.simulate_resize(gpui_kit::size(gpui_kit::px(width), gpui_kit::px(640.)));
                window_cx.update(|window, cx| window.draw(cx).clear(cx));
                let bound = window_cx.debug_bounds("key-binding-0").unwrap();
                let unbound = window_cx.debug_bounds("key-binding-7").unwrap();
                assert!(
                    bound.left() >= gpui_kit::px(0.) && bound.right() <= gpui_kit::px(width),
                    "native setting field remains inside the window"
                );
                if width == 1600. {
                    assert_eq!(bound.right(), unbound.right());
                }
            }
        }
    }
}
struct SettingsPageFixture {
    settings: gpui_kit::Entity<SettingsView>,
    page: usize,
}
impl gpui_kit::Render for SettingsPageFixture {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        self.settings.update(cx, |view, cx| {
            view.settings_panel(cx).default_selected_index(
                gpui_kit::component::setting::SelectIndex {
                    page_ix: self.page,
                    group_ix: None,
                },
            )
        })
    }
}
