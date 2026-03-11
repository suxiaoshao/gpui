use crate::llm::ProviderModel;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, IndexPath, Selectable, Size, StyleSized, h_flex,
    label::Label,
    list::{ListDelegate, ListState},
};
use std::{collections::BTreeMap, rc::Rc};

type OnConfirm = Rc<dyn Fn(ProviderModel, &mut Window, &mut App) + 'static>;
type OnCancel = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Clone, Debug)]
pub(crate) struct ModelPickerSection {
    pub(crate) title: SharedString,
    pub(crate) items: Vec<Rc<ProviderModel>>,
}

impl ModelPickerSection {
    pub(crate) fn from_models(models: &[ProviderModel]) -> Vec<Self> {
        let mut provider_groups = BTreeMap::<String, Vec<Rc<ProviderModel>>>::new();
        for model in models.iter().cloned() {
            provider_groups
                .entry(model.provider_name.clone())
                .or_default()
                .push(Rc::new(model));
        }

        provider_groups
            .into_iter()
            .map(|(provider, mut items)| {
                items.sort_by(|left, right| left.id.cmp(&right.id));
                Self {
                    title: provider.into(),
                    items,
                }
            })
            .collect()
    }
}

#[derive(IntoElement, Clone)]
pub(crate) struct ModelPickerItem {
    model: Rc<ProviderModel>,
    is_selected: bool,
}

impl ModelPickerItem {
    fn new(model: Rc<ProviderModel>) -> Self {
        Self {
            model,
            is_selected: false,
        }
    }
}

impl Selectable for ModelPickerItem {
    fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_selected
    }
}

impl RenderOnce for ModelPickerItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .relative()
            .gap_x_1()
            .py_1()
            .px_2()
            .rounded(cx.theme().radius)
            .text_base()
            .text_color(cx.theme().foreground)
            .items_center()
            .justify_between()
            .input_text_size(Size::Medium)
            .list_size(Size::Medium)
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().accent.alpha(0.7)))
            })
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .truncate()
                    .child(Label::new(&self.model.id).text_sm()),
            )
    }
}

pub(crate) struct ModelPickerDelegate {
    ix: Option<IndexPath>,
    all_sections: Vec<ModelPickerSection>,
    sections: Vec<ModelPickerSection>,
    last_query: String,
    loading: bool,
    empty_label: SharedString,
    on_confirm: OnConfirm,
    on_cancel: OnCancel,
}

impl ModelPickerDelegate {
    pub(crate) fn new(
        sections: Vec<ModelPickerSection>,
        loading: bool,
        empty_label: SharedString,
        on_confirm: OnConfirm,
        on_cancel: OnCancel,
    ) -> Self {
        Self {
            ix: None,
            all_sections: sections.clone(),
            sections,
            last_query: String::new(),
            loading,
            empty_label,
            on_confirm,
            on_cancel,
        }
    }

    pub(crate) fn set_sections(&mut self, sections: Vec<ModelPickerSection>) {
        self.all_sections = sections;
        self.apply_query();
    }

    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub(crate) fn selected_index_for(
        sections: &[ModelPickerSection],
        selected_model: Option<&ProviderModel>,
    ) -> Option<IndexPath> {
        let selected_model = selected_model?;
        sections.iter().enumerate().find_map(|(section_ix, section)| {
            section
                .items
                .iter()
                .position(|model| {
                    model.provider_name == selected_model.provider_name
                        && model.id == selected_model.id
                })
                .map(|row_ix| IndexPath::default().section(section_ix).row(row_ix))
        })
    }

    fn apply_query(&mut self) {
        let query = self.last_query.trim().to_lowercase();
        if query.is_empty() {
            self.sections = self.all_sections.clone();
            return;
        }

        self.sections = self
            .all_sections
            .iter()
            .filter_map(|section| {
                let items = section
                    .items
                    .iter()
                    .filter(|model| {
                        model.id.to_lowercase().contains(&query)
                            || model.provider_name.to_lowercase().contains(&query)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if items.is_empty() {
                    None
                } else {
                    Some(ModelPickerSection {
                        title: section.title.clone(),
                        items,
                    })
                }
            })
            .collect();
    }
}

impl ListDelegate for ModelPickerDelegate {
    type Item = ModelPickerItem;

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.last_query = query.to_string();
        self.apply_query();
        Task::ready(())
    }

    fn sections_count(&self, _cx: &App) -> usize {
        self.sections.len()
    }

    fn items_count(&self, section: usize, _cx: &App) -> usize {
        self.sections.get(section).map_or(0, |section| section.items.len())
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let section = self.sections.get(section)?;
        Some(
            div()
                .py_0p5()
                .px_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(section.title.clone()),
        )
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.sections
            .get(ix.section)
            .and_then(|section| section.items.get(ix.row))
            .cloned()
            .map(ModelPickerItem::new)
    }

    fn render_empty(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        h_flex()
            .justify_center()
            .py_6()
            .text_color(cx.theme().muted_foreground)
            .child(Label::new(self.empty_label.clone()).text_sm())
            .into_any_element()
    }

    fn loading(&self, _cx: &App) -> bool {
        self.loading
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.ix = ix;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(ix) = self.ix else {
            return;
        };
        let Some(model) = self
            .sections
            .get(ix.section)
            .and_then(|section| section.items.get(ix.row))
        else {
            return;
        };
        (self.on_confirm)(model.as_ref().clone(), window, cx);
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        (self.on_cancel)(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelPickerDelegate, ModelPickerSection};
    use crate::llm::{ProviderModel, ProviderModelCapability};

    #[test]
    fn selected_index_for_returns_none_when_model_is_missing() {
        let sections = ModelPickerSection::from_models(&[ProviderModel::new(
            "OpenAI",
            "gpt-4o",
            ProviderModelCapability::Streaming,
        )]);

        let missing = ProviderModel::new("OpenAI", "gpt-5", ProviderModelCapability::Streaming);
        assert_eq!(
            ModelPickerDelegate::selected_index_for(&sections, Some(&missing)),
            None
        );
    }
}
