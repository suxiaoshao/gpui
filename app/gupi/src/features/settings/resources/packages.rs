use super::*;
use gpui_kit::component::{
    Icon,
    collapsible::Collapsible,
    group_box::{GroupBox, GroupBoxVariants},
    tooltip::Tooltip,
};

const RESOURCE_KINDS: [(Kind, &str, IconName); 4] = [
    (
        Kind::Extension,
        "settings-package-kind-extensions",
        IconName::Puzzle,
    ),
    (
        Kind::Skill,
        "settings-package-kind-skills",
        IconName::BookOpen,
    ),
    (
        Kind::Theme,
        "settings-package-kind-themes",
        IconName::Palette,
    ),
    (
        Kind::Prompt,
        "settings-package-kind-prompts",
        IconName::FileText,
    ),
];

impl ResourcesView {
    pub(super) fn render_packages(&self, cx: &Context<Self>) -> AnyElement {
        let controller = self.controller.read(cx);
        let Some(catalog) = controller.catalog.data() else {
            return div().into_any_element();
        };
        let busy = controller.busy() || self.config.read(cx).busy(cx);
        let configured = self.config.read(cx).preferences(cx).pi_command;
        let pi_ready = self
            .applied_pi
            .read(cx)
            .ready_for(configured.as_deref())
            .is_some();
        let mut view = v_flex().gap_3().child(
            h_flex()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .debug_selector(|| "package-heading".into())
                        .child(t(cx, "settings-package-heading")),
                )
                .child(
                    Button::new("package-add")
                        .icon(IconName::Plus)
                        .label(t(cx, "settings-package-install"))
                        .disabled(busy || !pi_ready || self.installing)
                        .debug_selector(|| "package-add".into())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.installing = true;
                            this.source.focus_handle(cx).focus(window, cx);
                            cx.notify();
                        })),
                ),
        );
        if !pi_ready {
            view = view.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(cx, "settings-resource-pi-required")),
            );
        }
        if self.installing {
            view = view
                .child(
                    gpui_kit::component::form::field()
                        .label(t(cx, "settings-package-source"))
                        .description(t(cx, "settings-package-source-help"))
                        .child(Input::new(&self.source).disabled(busy || !pi_ready)),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("package-install")
                                .primary()
                                .label(t(cx, "settings-package-install"))
                                .disabled(
                                    busy || !pi_ready
                                        || self.source.read(cx).value().trim().is_empty(),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.package(
                                        "install",
                                        this.source.read(cx).value().trim().into(),
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new("package-cancel")
                                .ghost()
                                .label(t(cx, "action-cancel"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.installing = false;
                                    cx.notify();
                                })),
                        ),
                );
        }
        if catalog.packages.is_empty() {
            view = view.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(cx, "settings-package-empty")),
            );
        }
        for package in &catalog.packages {
            view = view.child(self.render_package(package, catalog, busy, pi_ready, cx));
        }
        let independent: Vec<_> = catalog
            .resources
            .iter()
            .filter(|resource| resource.kind == Kind::Extension && resource.package.is_none())
            .collect();
        if !independent.is_empty() {
            view = view.child(div().mt_2().child(t(cx, "settings-extension-heading")));
            for resource in independent {
                view = view.child(
                    GroupBox::new()
                        .outline()
                        .content_style(StyleRefinement::default().p_3().bg(cx.theme().background))
                        .child(self.render_package_resource(
                            resource,
                            IconName::Puzzle,
                            true,
                            busy,
                            cx,
                        )),
                );
            }
        }
        view.into_any_element()
    }

    fn render_package(
        &self,
        package: &io::Package,
        catalog: &io::Catalog,
        busy: bool,
        pi_ready: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let source = package.source.clone();
        let expanded = self.expanded_packages.contains(&source);
        let id = |part: &str| {
            ElementId::NamedChild(
                std::sync::Arc::new(ElementId::from(SharedString::from(source.clone()))),
                part.to_owned().into(),
            )
        };
        let toggle = source.clone();
        let reveal = package.path.clone();
        let update = source.clone();
        let remove = source.clone();
        let name = package_name(package);
        let tooltip = source.clone();
        let mut heading = v_flex().min_w_0().flex_1().gap_1().child(
            h_flex()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .id(id("name"))
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .child(name)
                        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx)),
                )
                .children(package.version.clone().map(|version| {
                    div()
                        .flex_shrink_0()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(version)
                })),
        );
        if let Some(counts) = io_counts(catalog, &source, cx) {
            heading = heading.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(counts.join(" · ")),
            );
        }
        let expand = Button::new(id("expand"))
            .ghost()
            .small()
            .icon(if expanded {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .tooltip(t(
                cx,
                if expanded {
                    "settings-package-collapse"
                } else {
                    "settings-package-expand"
                },
            ))
            .accessibility_label(t(
                cx,
                if expanded {
                    "settings-package-collapse"
                } else {
                    "settings-package-expand"
                },
            ))
            .debug_selector({
                let source = source.clone();
                move || format!("package-expand-{source}")
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.expanded_packages.remove(&toggle) {
                    this.expanded_packages.insert(toggle.clone());
                }
                cx.notify();
            }));
        let mut card = Collapsible::new().open(expanded).gap_3().child(
            h_flex().gap_2().child(heading).child(
                h_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .child(
                        Button::new(id("path"))
                            .ghost()
                            .small()
                            .icon(IconName::FolderOpen)
                            .tooltip(t(cx, "settings-resource-location"))
                            .accessibility_label(t(cx, "settings-resource-location"))
                            .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
                    )
                    .child(
                        Button::new(id("update"))
                            .ghost()
                            .small()
                            .icon(IconName::RotateCw)
                            .tooltip(t(cx, "settings-package-update"))
                            .accessibility_label(t(cx, "settings-package-update"))
                            .disabled(busy || !pi_ready)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.package("update", update.clone(), cx);
                            })),
                    )
                    .child(
                        Button::new(id("remove"))
                            .ghost()
                            .small()
                            .icon(IconName::Trash)
                            .tooltip(t(cx, "settings-package-remove"))
                            .accessibility_label(t(cx, "settings-package-remove"))
                            .disabled(busy || !pi_ready)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_remove(None, remove.clone(), window, cx);
                            })),
                    )
                    .child(expand),
            ),
        );
        if expanded {
            let mut contents = v_flex()
                .gap_3()
                .pt_3()
                .border_t_1()
                .border_color(cx.theme().border);
            for (kind, label, icon) in RESOURCE_KINDS {
                let resources: Vec<_> = catalog
                    .resources
                    .iter()
                    .filter(|r| r.package.as_deref() == Some(source.as_str()) && r.kind == kind)
                    .collect();
                if resources.is_empty() {
                    continue;
                }
                contents = contents.child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(t(cx, label)),
                        )
                        .children(resources.into_iter().map(|resource| {
                            self.render_package_resource(resource, icon, false, busy, cx)
                        })),
                );
            }
            card = card.content(contents);
        }
        GroupBox::new()
            .outline()
            .content_style(StyleRefinement::default().p_3().bg(cx.theme().background))
            .child(card)
            .into_any_element()
    }

    fn render_package_resource(
        &self,
        resource: &Resource,
        icon: IconName,
        independent: bool,
        busy: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let path = resource.path.clone();
        let id = |part: &str| {
            ElementId::NamedChild(
                std::sync::Arc::new(ElementId::from(SharedString::from(
                    resource.path.to_string_lossy().into_owned(),
                ))),
                part.to_owned().into(),
            )
        };
        let toggle = resource.clone();
        h_flex()
            .gap_2()
            .debug_selector({
                let path = path.clone();
                move || format!("package-resource-{}", path.display())
            })
            .child(Icon::new(icon).small())
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .id(id("name"))
                            .truncate()
                            .child(resource.name.clone())
                            .tooltip({
                                let path = path.clone();
                                move |window, cx| {
                                    Tooltip::new(path.to_string_lossy().into_owned())
                                        .build(window, cx)
                                }
                            }),
                    )
                    .when(independent, |row| {
                        row.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(path.to_string_lossy().into_owned()),
                        )
                    }),
            )
            .when(independent, |row| {
                row.child(
                    Button::new(id("path"))
                        .ghost()
                        .small()
                        .icon(IconName::FolderOpen)
                        .tooltip(t(cx, "settings-resource-location"))
                        .accessibility_label(t(cx, "settings-resource-location"))
                        .on_click(move |_, _, cx| cx.reveal_path(&path)),
                )
            })
            .child(
                Switch::new(id("enabled"))
                    .checked(resource.enabled)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, enabled, _, cx| {
                        this.change(Change::Toggle(toggle.clone(), *enabled), cx);
                    })),
            )
            .into_any_element()
    }
}

pub(super) fn package_name(package: &io::Package) -> String {
    let name = package
        .source
        .strip_prefix("npm:")
        .unwrap_or(&package.source);
    package
        .version
        .as_ref()
        .filter(|_| package.source.starts_with("npm:"))
        .and_then(|version| name.strip_suffix(&format!("@{version}")))
        .unwrap_or(name)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::ResourcesView;
    use crate::{
        foundation::pi_resources::{Catalog, Kind, Package, Resource},
        pi::PiProbeController,
        state::config::{AppConfig, AppLanguage, ConfigController},
    };
    use gpui_form::Form;
    use gpui_kit::component::{
        Root,
        group_box::GroupBoxVariant,
        setting::{SettingGroup, SettingItem, SettingPage, Settings},
    };
    use gpui_kit::{
        AppContext, Context, Entity, IntoElement, Modifiers, Render, Subscription, Task,
        TestAppContext, Window, px, size,
    };
    use gpui_operation::{Complete, Load, Transition};

    struct Fixture {
        resources: Entity<ResourcesView>,
        _subscription: Subscription,
    }
    impl Render for Fixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let resources = self.resources.clone();
            Settings::new("packages-fixture")
                .with_group_variant(GroupBoxVariant::Normal)
                .page(SettingPage::new("插件").group(SettingGroup::new().item(
                    SettingItem::render(move |_, _, cx| {
                        resources.update(cx, |view, cx| view.render_packages(cx))
                    }),
                )))
        }
    }

    #[gpui_kit::test]
    fn package_resources_expand_without_duplicating_extensions(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
        });
        let root = std::path::Path::new("/fixtures/pi");
        let source = "npm:demo@1.0.0";
        let mut rows: Vec<_> = [Kind::Extension, Kind::Skill, Kind::Theme, Kind::Prompt]
            .into_iter()
            .enumerate()
            .map(|(index, kind)| Resource {
                kind,
                path: root.join(index.to_string()),
                base: root.to_owned(),
                package: Some(source.into()),
                name: format!("resource-{index}"),
                description: String::new(),
                enabled: true,
                editable: false,
            })
            .collect();
        rows.push(Resource {
            kind: Kind::Extension,
            path: root.join("standalone.ts"),
            base: root.to_owned(),
            package: None,
            name: "standalone".into(),
            description: String::new(),
            enabled: false,
            editable: false,
        });
        let selectors = [
            "package-resource-/fixtures/pi/0",
            "package-resource-/fixtures/pi/1",
            "package-resource-/fixtures/pi/2",
            "package-resource-/fixtures/pi/3",
            "package-resource-/fixtures/pi/standalone.ts",
        ];
        let (_, cx) = cx.add_window_view(|window, cx| {
            let form = cx.new(|_| Form::new(AppConfig::default()));
            let config = cx.new(|cx| ConfigController::new(&form, cx));
            let pi = cx.new(|_| PiProbeController::new());
            let resources = cx.new(|cx| ResourcesView::new(config, pi, window, cx));
            resources
                .read(cx)
                .controller
                .clone()
                .update(cx, |controller, _| {
                    controller.catalog.transition(Load(Task::ready(())));
                    controller.catalog.transition(Complete(Ok(Catalog {
                        root: root.to_owned(),
                        packages: vec![Package {
                            source: source.into(),
                            path: root.to_owned(),
                            version: Some("1.0.0".into()),
                        }],
                        resources: rows,
                        warnings: Vec::new(),
                    })));
                });
            let fixture = cx.new(|cx| Fixture {
                _subscription: cx.observe(&resources, |_, _, cx| cx.notify()),
                resources,
            });
            Root::new(fixture, window, cx)
        });
        for width in [1100., 760.] {
            cx.simulate_resize(size(px(width), px(900.)));
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let heading = cx.debug_bounds("package-heading").unwrap();
            let install = cx.debug_bounds("package-add").unwrap();
            assert!((heading.center().y - install.center().y).abs() < px(1.));
            for selector in &selectors[..4] {
                assert!(
                    cx.debug_bounds(selector).is_none(),
                    "packaged rows must start collapsed"
                );
            }
            assert!(cx.debug_bounds(selectors[4]).is_some());
            let toggle = cx.debug_bounds("package-expand-npm:demo@1.0.0").unwrap();
            cx.simulate_click(toggle.center(), Modifiers::default());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            for selector in &selectors {
                let row = cx
                    .debug_bounds(selector)
                    .expect("all four resource kinds and standalone extension must appear");
                assert!(row.left() >= heading.left() && row.right() <= install.right());
            }
            let toggle = cx.debug_bounds("package-expand-npm:demo@1.0.0").unwrap();
            cx.simulate_click(toggle.center(), Modifiers::default());
            cx.update(|window, cx| window.draw(cx).clear(cx));
            for selector in &selectors[..4] {
                assert!(
                    cx.debug_bounds(selector).is_none(),
                    "packaged extensions must not remain in a second list"
                );
            }
            assert!(cx.debug_bounds(selectors[4]).is_some());
        }
    }
}
