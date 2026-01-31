use crate::database::ConversationTemplate;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Selectable, h_flex, label::Label, list::ListDelegate, tag::Tag};
use pinyin::ToPinyin;
use std::rc::Rc;

#[derive(IntoElement, Clone)]
pub struct TemplateItem {
    template: Rc<ConversationTemplate>,
    is_selected: bool,
}

impl RenderOnce for TemplateItem {
    fn render(self, _window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let ConversationTemplate {
            id,
            name,
            icon,
            description,
            mode,
            ..
        } = self.template.as_ref();
        h_flex()
            .id(*id)
            .gap_2()
            .p_4()
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .child(Label::new(icon))
            .child(
                Label::new(name).when_some(description.as_ref(), |this, description| {
                    this.secondary(description)
                }),
            )
            .child(
                match mode {
                    crate::database::Mode::Contextual => Tag::primary(),
                    crate::database::Mode::Single => Tag::info(),
                    crate::database::Mode::AssistantOnly => Tag::success(),
                }
                .child(mode.to_string())
                .outline(),
            )
    }
}

impl TemplateItem {
    fn new(template: Rc<ConversationTemplate>) -> Self {
        Self {
            template,
            is_selected: false,
        }
    }
}

impl Selectable for TemplateItem {
    fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_selected
    }
}

type OnConfirm = Rc<dyn Fn(&ConversationTemplate, &mut Window, &mut App) + 'static>;

pub(crate) struct TemporaryList {
    ix: Option<gpui_component::IndexPath>,
    items: Vec<Rc<ConversationTemplate>>,
    filtered_items: Vec<Rc<ConversationTemplate>>,
    on_confirm: OnConfirm,
}

impl TemporaryList {
    pub fn new(
        templates: Vec<ConversationTemplate>,
        on_confirm: impl Fn(&ConversationTemplate, &mut Window, &mut App) + 'static,
    ) -> Self {
        let items: Vec<Rc<ConversationTemplate>> = templates.into_iter().map(Rc::new).collect();
        let filtered_items = items.clone();
        Self {
            ix: None,
            filtered_items,
            items,
            on_confirm: Rc::new(on_confirm),
        }
    }
}

impl ListDelegate for TemporaryList {
    type Item = TemplateItem;

    fn items_count(&self, _section: usize, _cx: &gpui::App) -> usize {
        self.filtered_items.len()
    }

    fn render_item(
        &mut self,
        ix: gpui_component::IndexPath,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<gpui_component::list::ListState<Self>>,
    ) -> Option<Self::Item> {
        self.filtered_items
            .get(ix.row)
            .cloned()
            .map(TemplateItem::new)
    }

    fn set_selected_index(
        &mut self,
        ix: Option<gpui_component::IndexPath>,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<gpui_component::list::ListState<Self>>,
    ) {
        self.ix = ix;
    }
    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<gpui_component::list::ListState<Self>>,
    ) {
        if let Some(ix) = self.ix
            && let Some(item) = self.filtered_items.get(ix.row)
        {
            (self.on_confirm)(item.as_ref(), window, cx);
        }
    }
    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<gpui_component::list::ListState<Self>>,
    ) -> Task<()> {
        if query.is_empty() {
            self.filtered_items = self.items.clone();
        } else {
            self.filtered_items = self
                .items
                .iter()
                .filter(|item| {
                    item.name.contains(query)
                        || get_chinese_str(&item.name).contains(query)
                        || item
                            .description
                            .as_ref()
                            .is_some_and(|info| info.contains(query))
                        || get_chinese_str(item.description.as_ref().unwrap_or(&"".to_string()))
                            .contains(query)
                })
                .cloned()
                .collect();
        }
        Task::ready(())
    }
}

fn get_chinese_str(data: &str) -> String {
    data.chars()
        .map(|x| {
            x.to_pinyin()
                .map(|x| x.plain().to_string())
                .unwrap_or(x.to_string())
        })
        .fold("".to_string(), |acc, x| acc + &x)
}
