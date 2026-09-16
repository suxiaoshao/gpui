use super::*;
use gpui_kit::component::{
    collapsible::Collapsible,
    group_box::{GroupBox, GroupBoxVariants},
    setting::SettingItem,
    tag::Tag,
    text::TextView,
    tooltip::Tooltip,
};

impl ResourcesView {
    pub(super) fn filtered_skills(&self, cx: &App) -> Vec<Resource> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        self.controller
            .read(cx)
            .catalog
            .data()
            .into_iter()
            .flat_map(|catalog| &catalog.resources)
            .filter(|resource| resource.kind == Kind::Skill)
            .filter(|resource| {
                format!(
                    "{} {} {} {}",
                    resource.name,
                    resource.description,
                    resource.path.display(),
                    self.resource_source(resource, cx)
                )
                .to_lowercase()
                .contains(&query)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn skill_items(owner: &Entity<Self>, cx: &App) -> Vec<SettingItem> {
        owner
            .read(cx)
            .filtered_skills(cx)
            .into_iter()
            .map(|resource| {
                let owner = owner.downgrade();
                let keywords = vec![
                    resource.name.clone(),
                    resource.description.clone(),
                    resource.path.display().to_string(),
                    "Skill 技能".into(),
                ];
                SettingItem::render(move |_, _, cx| {
                    owner
                        .update(cx, |this, cx| this.render_skill(&resource, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .keywords(keywords)
            })
            .collect()
    }

    fn render_skill(&self, resource: &Resource, cx: &Context<Self>) -> AnyElement {
        let busy = self.controller.read(cx).busy() || self.config.read(cx).busy(cx);
        let path = resource.path.clone();
        let id = std::sync::Arc::new(ElementId::from(SharedString::from(
            path.to_string_lossy().into_owned(),
        )));
        let preview = self.previews.get(&path);
        let expanded = preview.is_some();
        let toggle = resource.clone();
        let reveal = path.clone();
        let edit = Open {
            path: path.clone(),
            kind: Kind::Skill,
            editable: true,
            create: false,
        };
        let remove = resource.clone();
        let toggle_path = path.clone();
        let toggle_label = t(
            cx,
            if expanded {
                "settings-skill-collapse"
            } else {
                "settings-resource-view"
            },
        );
        let actions = h_flex()
            .gap_1()
            .child(
                Button::new(ElementId::NamedChild(id.clone(), "preview".into()))
                    .ghost()
                    .small()
                    .icon(if expanded {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .label(toggle_label)
                    .debug_selector(|| "skill-preview-toggle".into())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.previews.remove(&toggle_path).is_none() {
                            this.load_preview(toggle_path.clone(), cx);
                        }
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .when(resource.editable, |row| {
                row.child(
                    Button::new(ElementId::NamedChild(id.clone(), "edit".into()))
                        .ghost()
                        .small()
                        .icon(IconName::SquarePen)
                        .tooltip(t(cx, "settings-resource-edit"))
                        .accessibility_label(t(cx, "settings-resource-edit"))
                        .debug_selector(|| "skill-edit".into())
                        .disabled(busy || self.open.is_running())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.request_open(edit.clone(), window, cx);
                        })),
                )
            })
            .child(
                Button::new(ElementId::NamedChild(id.clone(), "reveal".into()))
                    .ghost()
                    .small()
                    .icon(IconName::FolderOpen)
                    .tooltip(t(cx, "settings-resource-location"))
                    .accessibility_label(t(cx, "settings-resource-location"))
                    .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
            )
            .when(resource.editable, |row| {
                row.child(
                    Button::new(ElementId::NamedChild(id.clone(), "delete".into()))
                        .ghost()
                        .small()
                        .icon(IconName::Trash2)
                        .tooltip(t(cx, "settings-resource-delete"))
                        .accessibility_label(t(cx, "settings-resource-delete"))
                        .debug_selector(|| "skill-delete".into())
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_remove(
                                Some(remove.clone()),
                                remove.name.clone(),
                                window,
                                cx,
                            );
                        })),
                )
            });

        let mut card = Collapsible::new()
            .open(expanded)
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(gpui_kit::component::Icon::new(IconName::BookOpen).small())
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .child(resource.name.clone()),
                    )
                    .child(
                        Tag::secondary()
                            .small()
                            .outline()
                            .child(self.resource_source(resource, cx)),
                    )
                    .when(!resource.editable, |row| {
                        row.child(
                            Tag::secondary()
                                .small()
                                .outline()
                                .child(t(cx, "settings-resource-readonly-badge")),
                        )
                    })
                    .child(
                        Switch::new(ElementId::NamedChild(id.clone(), "enabled".into()))
                            .checked(resource.enabled)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, active, _, cx| {
                                this.change(Change::Toggle(toggle.clone(), *active), cx);
                            })),
                    ),
            )
            .when(!resource.description.is_empty(), |card| {
                card.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(resource.description.clone()),
                )
            })
            .child(
                div()
                    .id(ElementId::NamedChild(id.clone(), "path".into()))
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(path.to_string_lossy().into_owned())
                    .tooltip(move |window, cx| {
                        Tooltip::new(path.to_string_lossy().into_owned()).build(window, cx)
                    }),
            )
            .child(actions);
        if let Some(preview) = preview {
            let mut content = v_flex()
                .min_w_0()
                .gap_2()
                .debug_selector(|| "skill-preview-content".into());
            if preview.is_running() {
                content = content.child(gpui_kit::component::spinner::Spinner::new().small());
            }
            if let Some(error) = preview.problem() {
                content = content.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.to_string()),
                );
            }
            if let Some(text) = preview.data() {
                content = content.child(TextView::markdown(
                    ElementId::NamedChild(id, "markdown".into()),
                    preview_body(text).to_owned(),
                ));
            }
            card = card.content(content);
        }
        GroupBox::new()
            .outline()
            .content_style(StyleRefinement::default().p_3().bg(cx.theme().background))
            .child(card)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{ResourcesView, preview_body};
    use crate::{
        foundation::pi_resources::{self as io, Kind, Resource},
        pi::PiProbeController,
        state::config::{AppConfig, AppLanguage, ConfigController},
    };
    use gpui_form::Form;
    use gpui_kit::component::{
        Root,
        group_box::GroupBoxVariant,
        setting::{SettingGroup, SettingPage, Settings},
    };
    use gpui_kit::{
        AppContext, Context, Entity, IntoElement, Modifiers, Render, Subscription, Task,
        TestAppContext, Window, px, size,
    };
    use gpui_operation::{Complete, Load, Transition};

    #[test]
    fn preview_omits_only_complete_frontmatter() {
        assert_eq!(preview_body("---\nname: demo\n---\n\n# Body"), "# Body");
        assert_eq!(preview_body("---\r\nname: demo\r\n---\r\n# Body"), "# Body");
        assert_eq!(preview_body("# Body\n---\ntext"), "# Body\n---\ntext");
        assert_eq!(preview_body("---\nunfinished"), "---\nunfinished");
    }

    struct Fixture {
        resources: Entity<ResourcesView>,
        _subscription: Subscription,
    }
    impl Render for Fixture {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            Settings::new("skill-fixture")
                .with_group_variant(GroupBoxVariant::Normal)
                .page(SettingPage::new("Skill").group(
                    SettingGroup::new().items(ResourcesView::skill_items(&self.resources, cx)),
                ))
        }
    }

    #[gpui_kit::test]
    async fn skill_preview_stays_in_list_and_reloads_from_disk(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SKILL.md");
        std::fs::write(
            &path,
            "---\nname: demo\n---\n# Preview\n\nSome **content**.",
        )
        .unwrap();
        let mut resources = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let form = cx.new(|_| Form::new(AppConfig::default()));
            let config = cx.new(|cx| ConfigController::new(&form, cx));
            let pi = cx.new(|_| PiProbeController::new());
            let view = cx.new(|cx| ResourcesView::new(config, pi, window, cx));
            view.read(cx)
                .controller
                .clone()
                .update(cx, |controller, _| {
                    controller.catalog.transition(Load(Task::ready(())));
                    controller.catalog.transition(Complete(Ok(io::Catalog {
                        root: dir.path().to_owned(),
                        resources: vec![Resource {
                            kind: Kind::Skill,
                            path: path.clone(),
                            base: dir.path().to_owned(),
                            package: Some("npm:demo-skills".into()),
                            name: "demo".into(),
                            description: "Example skill".into(),
                            enabled: true,
                            editable: false,
                        }],
                        ..Default::default()
                    })));
                });
            resources = Some(view.clone());
            let fixture = cx.new(|cx| Fixture {
                _subscription: cx.observe(&view, |_, _, cx| cx.notify()),
                resources: view,
            });
            Root::new(fixture, window, cx)
        });
        let resources = resources.unwrap();
        cx.simulate_resize(size(px(900.), px(700.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("skill-edit").is_none());
        assert!(cx.debug_bounds("skill-delete").is_none());
        assert!(cx.debug_bounds("skill-preview-content").is_none());
        let button = cx.debug_bounds("skill-preview-toggle").unwrap();
        cx.simulate_click(button.center(), Modifiers::default());
        cx.condition(&resources, |view, _| {
            view.previews.get(&path).is_some_and(|p| p.data().is_some())
        })
        .await;
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            cx.debug_bounds("skill-preview-content")
                .unwrap()
                .size
                .height
                > px(30.)
        );
        cx.update(|_, cx| assert!(resources.read(cx).editor.is_none()));
        std::fs::write(&path, "# Updated").unwrap();
        cx.update(|_, cx| resources.update(cx, |view, cx| view.load_preview(path.clone(), cx)));
        cx.condition(&resources, |view, _| {
            view.previews
                .get(&path)
                .and_then(|p| p.data())
                .is_some_and(|s| s == "# Updated")
        })
        .await;
        std::fs::remove_file(&path).unwrap();
        cx.update(|_, cx| resources.update(cx, |view, cx| view.load_preview(path.clone(), cx)));
        cx.condition(&resources, |view, _| {
            view.previews
                .get(&path)
                .is_some_and(|preview| preview.problem().is_some())
        })
        .await;
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let button = cx.debug_bounds("skill-preview-toggle").unwrap();
        cx.simulate_click(button.center(), Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("skill-preview-content").is_none());
        cx.update(|window, cx| {
            for query in ["demo-skills", path.to_str().unwrap()] {
                resources.update(cx, |view, cx| {
                    view.search
                        .update(cx, |input, cx| input.set_value(query, window, cx));
                    assert_eq!(view.filtered_skills(cx).len(), 1);
                });
            }
        });
    }
}
