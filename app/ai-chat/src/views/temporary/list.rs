use crate::database::ConversationTemplate;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Selectable, h_flex, label::Label, list::ListDelegate, tag::Tag};
use std::rc::Rc;

#[derive(IntoElement, Clone)]
pub struct TemplateItem {
    template: ConversationTemplate,
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
        } = self.template;
        h_flex()
            .id(id)
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
    fn new(template: ConversationTemplate) -> Self {
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
    items: Vec<TemplateItem>,
    on_confirm: OnConfirm,
}

impl TemporaryList {
    pub fn new(
        templates: Vec<ConversationTemplate>,
        on_confirm: impl Fn(&ConversationTemplate, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            ix: None,
            items: templates.into_iter().map(TemplateItem::new).collect(),
            on_confirm: Rc::new(on_confirm),
        }
    }
}

impl ListDelegate for TemporaryList {
    type Item = TemplateItem;

    fn items_count(&self, _section: usize, _cx: &gpui::App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: gpui_component::IndexPath,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<gpui_component::list::ListState<Self>>,
    ) -> Option<Self::Item> {
        self.items.get(ix.row).cloned()
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
            && let Some(item) = self.items.get(ix.row)
        {
            (self.on_confirm)(&item.template, window, cx);
        }
    }
}
