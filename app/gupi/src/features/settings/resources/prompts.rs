use super::*;
use gpui_kit::component::{
    Icon, Selectable,
    collapsible::Collapsible,
    group_box::{GroupBox, GroupBoxVariants},
    setting::SettingItem,
    tag::Tag,
    text::TextView,
    tooltip::Tooltip,
};

impl ResourcesView {
    pub(super) fn filtered_prompts(&self, cx: &App) -> Vec<Resource> {
        let query = self.prompt_search.read(cx).value().trim().to_lowercase();
        self.controller
            .read(cx)
            .catalog
            .data()
            .into_iter()
            .flat_map(|catalog| &catalog.resources)
            .filter(|resource| resource.kind == Kind::Prompt)
            .filter(|resource| {
                format!(
                    "/{} {} {}",
                    resource.name,
                    resource.description,
                    self.resource_source(resource, cx)
                )
                .to_lowercase()
                .contains(&query)
            })
            .cloned()
            .collect()
    }

    pub(crate) fn prompt_items(owner: &Entity<Self>, cx: &App) -> Vec<SettingItem> {
        owner
            .read(cx)
            .filtered_prompts(cx)
            .into_iter()
            .map(|resource| {
                let owner = owner.downgrade();
                let keywords = vec![
                    resource.name.clone(),
                    resource.description.clone(),
                    "prompt template 模板 提示词".into(),
                ];
                SettingItem::render(move |_, _, cx| {
                    owner
                        .update(cx, |this, cx| this.render_prompt(&resource, cx))
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .keywords(keywords)
            })
            .collect()
    }

    pub(crate) fn system_prompt_items(owner: &Entity<Self>, cx: &App) -> Vec<SettingItem> {
        [
            (
                "SYSTEM.md",
                "settings-system-replace",
                "settings-system-replace-help",
            ),
            (
                "APPEND_SYSTEM.md",
                "settings-system-append",
                "settings-system-append-help",
            ),
        ]
        .into_iter()
        .map(|(file, title, help)| {
            let owner = owner.downgrade();
            SettingItem::render(move |_, _, cx| {
                owner
                    .update(cx, |this, cx| {
                        this.render_system_prompt(file, title, help, cx)
                    })
                    .unwrap_or_else(|_| div().into_any_element())
            })
            .keywords(vec![
                t(cx, title),
                t(cx, help),
                file.into(),
                "system prompt 系统提示词".into(),
            ])
        })
        .collect()
    }

    fn render_system_prompt(
        &self,
        file: &'static str,
        title: &str,
        help: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let controller = self.controller.read(cx);
        let Some(catalog) = controller.catalog.data() else {
            return div().into_any_element();
        };
        let busy = controller.busy() || self.config.read(cx).busy(cx) || self.open.is_running();
        let path = catalog.root.join(file);
        let tooltip = path.display().to_string();
        let open = Open {
            path,
            kind: Kind::Prompt,
            editable: true,
            create: false,
        };
        GroupBox::new()
            .outline()
            .content_style(StyleRefinement::default().p_3().bg(cx.theme().background))
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .id(file)
                                    .child(t(cx, title))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(tooltip.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t(cx, help)),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{file}-edit")))
                            .small()
                            .icon(IconName::SquarePen)
                            .tooltip(t(cx, "settings-resource-edit"))
                            .accessibility_label(t(cx, "settings-resource-edit"))
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.request_open(open.clone(), window, cx)
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_prompt(&self, resource: &Resource, cx: &Context<Self>) -> AnyElement {
        let busy = self.controller.read(cx).busy() || self.config.read(cx).busy(cx);
        let id = |part: &'static str| {
            ElementId::NamedChild(
                std::sync::Arc::new(ElementId::from(SharedString::from(
                    resource.path.to_string_lossy().into_owned(),
                ))),
                part.into(),
            )
        };
        let preview = self.previews.get(&resource.path);
        let expanded = preview.is_some();
        let toggle_path = resource.path.clone();
        let reveal = resource.path.clone();
        let edit = Open {
            path: resource.path.clone(),
            kind: Kind::Prompt,
            editable: resource.editable,
            create: false,
        };
        let remove = resource.clone();
        let toggle = resource.clone();
        let preview_label = t(
            cx,
            if expanded {
                "settings-template-collapse"
            } else {
                "settings-resource-view"
            },
        );
        let actions = h_flex()
            .flex_shrink_0()
            .gap_1()
            .debug_selector(|| "template-actions".into())
            .child(
                Button::new(id("preview"))
                    .ghost()
                    .small()
                    .icon(IconName::Eye)
                    .selected(expanded)
                    .tooltip(preview_label.clone())
                    .accessibility_label(preview_label)
                    .debug_selector(|| "template-preview-toggle".into())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.previews.remove(&toggle_path).is_none() {
                            this.load_preview(toggle_path.clone(), cx);
                        }
                        cx.notify();
                    })),
            )
            .child(
                Button::new(id("reveal"))
                    .ghost()
                    .small()
                    .icon(IconName::FolderOpen)
                    .tooltip(t(cx, "settings-resource-location"))
                    .accessibility_label(t(cx, "settings-resource-location"))
                    .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
            )
            .when(resource.editable, |row| {
                row.child(
                    Button::new(id("edit"))
                        .ghost()
                        .small()
                        .icon(IconName::SquarePen)
                        .tooltip(t(cx, "settings-resource-edit"))
                        .accessibility_label(t(cx, "settings-resource-edit"))
                        .debug_selector(|| "template-edit".into())
                        .disabled(busy || self.open.is_running())
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.request_open(edit.clone(), window, cx)
                        })),
                )
                .child(
                    Button::new(id("delete"))
                        .ghost()
                        .small()
                        .icon(IconName::Trash)
                        .tooltip(t(cx, "settings-resource-delete"))
                        .accessibility_label(t(cx, "settings-resource-delete"))
                        .debug_selector(|| "template-delete".into())
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_remove(
                                Some(remove.clone()),
                                remove.name.clone(),
                                window,
                                cx,
                            )
                        })),
                )
            })
            .child(
                Switch::new(id("enabled"))
                    .checked(resource.enabled)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, active, _, cx| {
                        this.change(Change::Toggle(toggle.clone(), *active), cx)
                    })),
            );
        let source = self.resource_source(resource, cx);
        let source_label = resource
            .package
            .as_ref()
            .and_then(|source| {
                self.controller
                    .read(cx)
                    .catalog
                    .data()?
                    .packages
                    .iter()
                    .find(|package| &package.source == source)
            })
            .map(packages::package_name)
            .unwrap_or_else(|| source.strip_prefix("npm:").unwrap_or(&source).to_owned());
        let source_help = if resource.editable {
            source.clone()
        } else {
            format!("{} · {source}", t(cx, "settings-resource-readonly-badge"))
        };
        let path_help = resource.path.display().to_string();
        let mut card = Collapsible::new()
            .open(expanded)
            .gap_2()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .debug_selector(|| "template-card-header".into())
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w(px(160.))
                            .gap_2()
                            .child(Icon::new(IconName::FileText).small())
                            .child(
                                div()
                                    .id(id("name"))
                                    .min_w_0()
                                    .truncate()
                                    .child(format!("/{}", resource.name))
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(path_help.clone()).build(window, cx)
                                    }),
                            ),
                    )
                    .child(actions),
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
                h_flex().child(
                    div()
                        .id(id("source"))
                        .max_w_full()
                        .truncate()
                        .tooltip(move |window, cx| {
                            Tooltip::new(source_help.clone()).build(window, cx)
                        })
                        .child(
                            Tag::secondary().small().outline().child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Icon::new(if resource.editable {
                                            IconName::UserRound
                                        } else {
                                            IconName::LockKeyhole
                                        })
                                        .xsmall(),
                                    )
                                    .child(source_label),
                            ),
                        ),
                ),
            );
        if let Some(preview) = preview {
            let mut content = v_flex()
                .min_w_0()
                .gap_2()
                .pt_3()
                .border_t_1()
                .border_color(cx.theme().border)
                .debug_selector(|| "template-preview-content".into());
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
                    id("markdown"),
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
    use super::ResourcesView;
    use crate::{
        features::settings::resources::Section,
        foundation::pi_resources::{Catalog, Kind, Resource},
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
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let resources = self.resources.clone();
            let catalog = SettingItem::render(move |_, _, cx| {
                resources.update(cx, |view, cx| {
                    view.render_page(Kind::Prompt, Section::Catalog, cx)
                })
            });
            let page = SettingPage::new("提示词")
                .group(
                    SettingGroup::new()
                        .title("系统提示词")
                        .items(ResourcesView::system_prompt_items(&self.resources, cx)),
                )
                .group(
                    SettingGroup::new()
                        .item(catalog)
                        .items(ResourcesView::prompt_items(&self.resources, cx)),
                );
            Settings::new("prompt-fixture")
                .with_group_variant(GroupBoxVariant::Normal)
                .page(page)
        }
    }

    #[gpui_kit::test]
    async fn template_search_preview_and_edit_respect_resource_ownership(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
        });
        let dir = tempfile::tempdir().unwrap();
        let packaged = dir.path().join("parallel-review.md");
        let personal = dir.path().join("daily-summary.md");
        std::fs::write(&packaged, "---\ndescription: Parallel review\n---\n# Review\n\nReview $@ without expanding arguments.").unwrap();
        std::fs::write(&personal, "Summarize today.").unwrap();
        let mut owner = None;
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
                        root: dir.path().to_owned(),
                        resources: vec![
                            Resource {
                                kind: Kind::Prompt,
                                path: packaged.clone(),
                                base: dir.path().to_owned(),
                                package: Some("npm:pi-subagents@0.49.0".into()),
                                name: "parallel-review".into(),
                                description: "Independent review".into(),
                                enabled: true,
                                editable: false,
                            },
                            Resource {
                                kind: Kind::Prompt,
                                path: personal.clone(),
                                base: dir.path().to_owned(),
                                package: None,
                                name: "daily-summary".into(),
                                description: "汇总今天的改动与待办".into(),
                                enabled: true,
                                editable: true,
                            },
                        ],
                        ..Default::default()
                    })));
                });
            resources.update(cx, |view, cx| {
                view.search.update(cx, |input, cx| {
                    input.set_value("unrelated skill query", window, cx)
                });
                view.prompt_search
                    .update(cx, |input, cx| input.set_value("/parallel", window, cx));
            });
            owner = Some(resources.clone());
            let fixture = cx.new(|cx| Fixture {
                _subscription: cx.observe(&resources, |_, _, cx| cx.notify()),
                resources,
            });
            Root::new(fixture, window, cx)
        });
        let owner = owner.unwrap();
        cx.simulate_resize(size(px(760.), px(1000.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("template-edit").is_none());
        assert!(cx.debug_bounds("template-delete").is_none());
        assert!(cx.debug_bounds("template-preview-content").is_none());
        let preview = cx.debug_bounds("template-preview-toggle").unwrap();
        cx.simulate_click(preview.center(), Modifiers::default());
        cx.condition(&owner, |view, _| {
            view.previews
                .get(&packaged)
                .is_some_and(|preview| preview.data().is_some())
        })
        .await;
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            cx.debug_bounds("template-preview-content")
                .unwrap()
                .size
                .height
                > px(30.)
        );
        cx.update(|_, cx| assert!(owner.read(cx).editor.is_none()));
        let preview = cx.debug_bounds("template-preview-toggle").unwrap();
        cx.simulate_click(preview.center(), Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("template-preview-content").is_none());
        cx.update(|window, cx| {
            owner.update(cx, |view, cx| {
                assert!(view.previews.is_empty());
                for (query, expected) in [
                    ("pi-subagents", 1),
                    ("Independent", 1),
                    ("汇总", 1),
                    ("not-found", 0),
                    ("", 2),
                    ("daily-summary", 1),
                ] {
                    view.prompt_search
                        .update(cx, |input, cx| input.set_value(query, window, cx));
                    assert_eq!(view.filtered_prompts(cx).len(), expected, "query: {query}");
                }
                assert_eq!(
                    view.search.read(cx).value().as_ref(),
                    "unrelated skill query"
                );
            })
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("template-delete").is_some());
        let header = cx.debug_bounds("template-card-header").unwrap();
        let actions = cx.debug_bounds("template-actions").unwrap();
        assert!(actions.right() <= header.right() && actions.left() >= header.left());
        let edit = cx.debug_bounds("template-edit").unwrap();
        cx.simulate_click(edit.center(), Modifiers::default());
        cx.condition(&owner, |view, _| {
            view.editor
                .as_ref()
                .is_some_and(|editor| editor.path == personal && editor.editable)
        })
        .await;
    }
}
