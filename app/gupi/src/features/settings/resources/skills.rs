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
                        .icon(IconName::Trash)
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
    use super::preview_body;

    #[test]
    fn preview_omits_only_complete_frontmatter() {
        assert_eq!(preview_body("---\nname: demo\n---\n\n# Body"), "# Body");
        assert_eq!(preview_body("---\r\nname: demo\r\n---\r\n# Body"), "# Body");
        assert_eq!(preview_body("# Body\n---\ntext"), "# Body\n---\ntext");
        assert_eq!(preview_body("---\nunfinished"), "---\nunfinished");
    }
}
