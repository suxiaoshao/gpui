use super::*;
use gpui_kit::component::{
    IndexPath,
    list::{ListDelegate, ListItem},
};
use pi_rpc::protocol::Model;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::features::home) struct ModelKey {
    pub provider: String,
    pub id: String,
}
impl From<&Model> for ModelKey {
    fn from(model: &Model) -> Self {
        Self {
            provider: model.provider.clone(),
            id: model.id.clone(),
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ModelOption {
    pub key: ModelKey,
    pub title: String,
    search: String,
    reasoning: bool,
    vision: bool,
}
impl From<&Model> for ModelOption {
    fn from(model: &Model) -> Self {
        Self {
            key: ModelKey::from(model),
            title: model.name.clone(),
            search: format!("{} {} {}", model.provider, model.id, model.name).to_lowercase(),
            reasoning: model.reasoning,
            vision: model
                .extra
                .get("input")
                .and_then(|v| v.as_array())
                .is_some_and(|v| v.iter().any(|v| v == "image")),
        }
    }
}
#[derive(Default)]
pub(super) struct ModelList {
    all: Vec<ModelOption>,
    groups: Vec<(String, Vec<ModelOption>)>,
    query: String,
    current: Option<ModelKey>,
    disabled: bool,
}
impl ModelList {
    pub fn replace(&mut self, all: Vec<ModelOption>, current: Option<ModelKey>, disabled: bool) {
        self.all = all;
        self.current = current;
        self.disabled = disabled;
        self.filter();
    }
    fn filter(&mut self) {
        let mut groups = BTreeMap::<String, Vec<ModelOption>>::new();
        for m in self.all.iter().filter(|m| m.search.contains(&self.query)) {
            groups
                .entry(m.key.provider.clone())
                .or_default()
                .push(m.clone());
        }
        for items in groups.values_mut() {
            items.sort_by(|a, b| a.title.cmp(&b.title));
        }
        self.groups = groups.into_iter().collect();
    }
    pub fn item(&self, ix: IndexPath) -> Option<&ModelOption> {
        self.groups.get(ix.section)?.1.get(ix.row)
    }
    pub fn position(&self, key: &ModelKey) -> Option<IndexPath> {
        self.groups
            .iter()
            .enumerate()
            .find_map(|(section, (_, items))| {
                items
                    .iter()
                    .position(|m| &m.key == key)
                    .map(|row| IndexPath::new(section).row(row))
            })
    }
}
impl ListDelegate for ModelList {
    type Item = ListItem;
    fn sections_count(&self, _: &App) -> usize {
        self.groups.len()
    }
    fn items_count(&self, section: usize, _: &App) -> usize {
        self.groups.get(section).map_or(0, |g| g.1.len())
    }
    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
    }
    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.query = query.to_lowercase();
        self.filter();
        cx.notify();
        Task::ready(())
    }
    fn render_section_header(
        &mut self,
        section: usize,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        Some(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(self.groups.get(section)?.0.clone()),
        )
    }
    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let model = self.item(ix)?;
        let tags = [
            model
                .reasoning
                .then(|| t(cx, "conversation-model-reasoning")),
            model.vision.then(|| t(cx, "conversation-model-vision")),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        Some(
            ListItem::new(format!("model-{}-{}", model.key.provider, model.key.id))
                .h(px(54.))
                .disabled(self.disabled)
                .confirmed(self.current.as_ref() == Some(&model.key))
                .child(
                    h_flex()
                        .gap_2()
                        .min_w_0()
                        .child(
                            crate::foundation::assets::provider_icon(&model.key.provider)
                                .size_4()
                                .flex_none(),
                        )
                        .child(
                            v_flex()
                                .min_w_0()
                                .gap_0p5()
                                .child(div().text_sm().truncate().child(model.title.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate()
                                        .child(tags),
                                ),
                        ),
                ),
        )
    }
    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .p_3()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(t(cx, "conversation-model-empty"))
    }
}
