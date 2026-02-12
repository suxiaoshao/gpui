use crate::{
    components::template_edit_dialog::open_add_template_dialog,
    database::{ConversationTemplate, Db, Mode},
    errors::AiChatResult,
    i18n::I18n,
    store::{ChatData, ChatDataEvent},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    list::{List, ListDelegate, ListState},
    tag::Tag,
    v_flex,
};
use std::{ops::Deref, rc::Rc};

actions!(template_list_view, [Add]);

const CONTEXT: &str = "template_list_view";

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("secondary-n", Add, Some(CONTEXT))]);
}

#[derive(IntoElement, Clone)]
struct TemplateItem {
    template: Rc<ConversationTemplate>,
    is_selected: bool,
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

impl RenderOnce for TemplateItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let template = self.template;
        h_flex()
            .id(template.id)
            .w_full()
            .gap_3()
            .items_start()
            .px_4()
            .py_3()
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .child(
                Label::new(&template.icon)
                    .when_some(template.description.as_ref(), |this, desc| {
                        this.secondary(desc)
                    }),
            )
            .child(
                v_flex().flex_1().gap_1().child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Label::new(&template.name).text_sm())
                        .child(
                            match template.mode {
                                Mode::Contextual => Tag::primary(),
                                Mode::Single => Tag::info(),
                                Mode::AssistantOnly => Tag::success(),
                            }
                            .outline()
                            .child(template.mode.to_string()),
                        ),
                ),
            )
    }
}

type OnConfirm = Rc<dyn Fn(i32, &mut Window, &mut App) + 'static>;

struct TemplateListDelegate {
    ix: Option<gpui_component::IndexPath>,
    items: Vec<Rc<ConversationTemplate>>,
    filtered_items: Vec<Rc<ConversationTemplate>>,
    on_confirm: OnConfirm,
}

impl TemplateListDelegate {
    fn new(items: Vec<ConversationTemplate>, on_confirm: OnConfirm) -> Self {
        let items = items.into_iter().map(Rc::new).collect::<Vec<_>>();
        Self {
            ix: None,
            filtered_items: items.clone(),
            items,
            on_confirm,
        }
    }
}

impl ListDelegate for TemplateListDelegate {
    type Item = TemplateItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.filtered_items.len()
    }

    fn render_item(
        &mut self,
        ix: gpui_component::IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.filtered_items
            .get(ix.row)
            .cloned()
            .map(TemplateItem::new)
    }

    fn set_selected_index(
        &mut self,
        ix: Option<gpui_component::IndexPath>,
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
        if let Some(ix) = self.ix
            && let Some(template) = self.filtered_items.get(ix.row)
        {
            (self.on_confirm)(template.id, window, cx);
        }
    }

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        if query.is_empty() {
            self.filtered_items = self.items.clone();
        } else {
            self.filtered_items = self
                .items
                .iter()
                .filter(|item| {
                    item.name.contains(query)
                        || item
                            .description
                            .as_ref()
                            .is_some_and(|description| description.contains(query))
                })
                .cloned()
                .collect();
        }
        Task::ready(())
    }
}

pub(crate) struct TemplateListView {
    templates: AiChatResult<Entity<ListState<TemplateListDelegate>>>,
}

impl TemplateListView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            templates: Self::build_list(window, cx),
        }
    }

    fn get_templates(cx: &mut Context<Self>) -> AiChatResult<Vec<ConversationTemplate>> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::all(conn)
    }

    fn build_list(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AiChatResult<Entity<ListState<TemplateListDelegate>>> {
        let templates = Self::get_templates(cx)?;
        let on_confirm: OnConfirm = Rc::new(|template_id, _window, cx| {
            let chat_data = cx.global::<ChatData>().deref().clone();
            chat_data.update(cx, |_this, cx| {
                cx.emit(ChatDataEvent::OpenTemplateDetail(template_id));
            });
        });
        let list = cx.new(move |cx| {
            let mut state =
                ListState::new(TemplateListDelegate::new(templates, on_confirm), window, cx)
                    .searchable(true);
            state.focus(window, cx);
            state
        });
        Ok(list)
    }

    fn reload_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.templates = Self::build_list(window, cx);
        cx.notify();
    }

    fn add_template(&mut self, _: &Add, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        open_add_template_dialog(
            Rc::new(move |template, window, cx| {
                let _ = this.update(cx, |view, cx| {
                    view.reload_templates(window, cx);
                });
                let chat_data = cx.global::<ChatData>().deref().clone();
                chat_data.update(cx, |_this, cx| {
                    cx.emit(ChatDataEvent::OpenTemplateDetail(template.id));
                });
            }),
            window,
            cx,
        );
    }
}

impl Render for TemplateListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (add_label, load_failed_title) = {
            let i18n = cx.global::<I18n>();
            (i18n.t("button-add"), i18n.t("notify-load-templates-failed"))
        };
        v_flex()
            .key_context(CONTEXT)
            .size_full()
            .on_action(cx.listener(Self::add_template))
            .child(
                h_flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .px_4()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Button::new("template-add").primary().label(add_label).on_click(
                        cx.listener(|_view, _, window, cx| {
                            window.dispatch_action(Add.boxed_clone(), cx);
                        }),
                    )),
            )
            .map(|this| match &self.templates {
                Ok(templates) => this.child(List::new(templates).large()),
                Err(err) => this.child(
                    v_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .child(Label::new(format!("{load_failed_title}: {err}")).text_sm()),
                ),
            })
    }
}
