use crate::database::ConversationTemplate;
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, IndexPath, Selectable, Size, StyleSized, h_flex,
    label::Label,
    list::{ListDelegate, ListState},
};
use std::{rc::Rc, sync::Arc};

type OnConfirm = Arc<dyn Fn(ConversationTemplate, &mut Window, &mut App) + 'static>;
type OnCancel = Arc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(IntoElement, Clone)]
pub(crate) struct TemplatePickerItem {
    template: Rc<ConversationTemplate>,
    is_selected: bool,
}

impl TemplatePickerItem {
    fn new(template: Rc<ConversationTemplate>) -> Self {
        Self {
            template,
            is_selected: false,
        }
    }
}

impl Selectable for TemplatePickerItem {
    fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.is_selected
    }
}

impl RenderOnce for TemplatePickerItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let template = self.template;
        h_flex()
            .id(template.id)
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
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .child(Label::new(&template.icon))
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .truncate()
                            .child(
                                Label::new(&template.name).text_sm().when_some(
                                    template.description.as_ref(),
                                    |this, description| this.secondary(description),
                                ),
                            ),
                    ),
            )
    }
}

pub(crate) struct TemplatePickerDelegate {
    ix: Option<IndexPath>,
    items: Vec<Rc<ConversationTemplate>>,
    on_confirm: OnConfirm,
    on_cancel: OnCancel,
}

impl TemplatePickerDelegate {
    pub(crate) fn new(
        items: Vec<ConversationTemplate>,
        on_confirm: OnConfirm,
        on_cancel: OnCancel,
    ) -> Self {
        Self {
            ix: None,
            items: items.into_iter().map(Rc::new).collect(),
            on_confirm,
            on_cancel,
        }
    }

    pub(crate) fn selected_index_for(
        items: &[ConversationTemplate],
        selected_template: Option<&ConversationTemplate>,
    ) -> Option<IndexPath> {
        let selected_template = selected_template?;
        items
            .iter()
            .position(|template| template.id == selected_template.id)
            .map(|ix| IndexPath::default().row(ix))
    }
}

impl ListDelegate for TemplatePickerDelegate {
    type Item = TemplatePickerItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.items.get(ix.row).cloned().map(TemplatePickerItem::new)
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
        let Some(template) = self.items.get(ix.row) else {
            return;
        };
        (self.on_confirm)(template.as_ref().clone(), window, cx);
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        (self.on_cancel)(window, cx);
    }
}
