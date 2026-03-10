use crate::{
    components::template_edit_dialog::open_add_template_dialog,
    database::{ConversationTemplate, Db},
    errors::AiChatResult,
    i18n::I18n,
    store::{ChatData, ChatDataEvent},
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, IndexPath, Selectable, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Enter, Input, InputEvent, InputState, MoveDown, MoveUp},
    label::Label,
    list::{List, ListDelegate, ListState},
    v_flex,
};
use std::{ops::Deref, rc::Rc};

actions!(template_list_view, [Add]);

const CONTEXT: &str = "template_list_view";
const TEMPLATE_ITEM_HEIGHT: f32 = 44.;

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
        let prompts_label = {
            let i18n = cx.global::<I18n>();
            format!("{} {}", template.prompts.len(), i18n.t("field-prompts"))
        };

        h_flex()
            .id(template.id)
            .w_full()
            .gap_3()
            .h(px(TEMPLATE_ITEM_HEIGHT))
            .items_center()
            .px_3()
            .py_2()
            .rounded(cx.theme().radius)
            .when(!self.is_selected, |this| {
                this.hover(|this| this.bg(cx.theme().accent.alpha(0.7)))
            })
            .when(self.is_selected, |this| this.bg(cx.theme().accent))
            .child(
                div()
                    .flex()
                    .size_8()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().border.opacity(0.35))
                    .child(Label::new(&template.icon).text_base()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_none()
                            .max_w(relative(0.45))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .truncate()
                            .child(Label::new(&template.name).text_sm()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .text_color(cx.theme().muted_foreground)
                            .when_some(template.description.as_ref(), |this, description| {
                                this.whitespace_nowrap()
                                    .truncate()
                                    .child(Label::new(description).text_xs())
                            }),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(prompts_label),
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

    fn apply_query(&mut self, query: &str) {
        self.filtered_items = filter_templates(&self.items, query);
        if self
            .ix
            .is_some_and(|ix| ix.row >= self.filtered_items.len())
        {
            self.ix = None;
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
        self.apply_query(query);
        Task::ready(())
    }
}

pub(crate) struct TemplateListView {
    search_input: Entity<InputState>,
    templates: AiChatResult<Entity<ListState<TemplateListDelegate>>>,
    _search_input_subscription: Subscription,
}

impl TemplateListView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_placeholder = cx.global::<I18n>().t("field-search-template");
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(search_placeholder.clone()));
        let _search_input_subscription =
            cx.subscribe_in(&search_input, window, Self::on_search_input_event);
        search_input.focus_handle(cx).focus(window);

        Self {
            templates: Self::build_list("", window, cx),
            search_input,
            _search_input_subscription,
        }
    }

    fn get_templates(cx: &mut Context<Self>) -> AiChatResult<Vec<ConversationTemplate>> {
        let conn = &mut cx.global::<Db>().get()?;
        ConversationTemplate::all(conn)
    }

    fn build_list(
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AiChatResult<Entity<ListState<TemplateListDelegate>>> {
        let templates = Self::get_templates(cx)?;
        let query = query.trim().to_string();
        let on_confirm: OnConfirm = Rc::new(|template_id, _window, cx| {
            let chat_data = cx.global::<ChatData>().deref().clone();
            chat_data.update(cx, |_this, cx| {
                cx.emit(ChatDataEvent::OpenTemplateDetail(template_id));
            });
        });
        let list = cx.new(move |cx| {
            let mut state =
                ListState::new(TemplateListDelegate::new(templates, on_confirm), window, cx);
            let has_items = {
                let delegate = state.delegate_mut();
                delegate.apply_query(&query);
                !delegate.filtered_items.is_empty()
            };
            state.set_selected_index(has_items.then_some(IndexPath::default()), window, cx);
            state.scroll_to_item(IndexPath::default(), ScrollStrategy::Top, window, cx);
            state
        });
        Ok(list)
    }

    fn reload_templates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.current_query(cx);
        self.templates = Self::build_list(&query, window, cx);
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

    fn current_query(&self, cx: &App) -> String {
        self.search_input.read(cx).value().trim().to_string()
    }

    fn on_search_input_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        self.apply_search_query(window, cx);
    }

    fn apply_search_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.current_query(cx);
        let Ok(templates) = &self.templates else {
            return;
        };

        templates.update(cx, |state, cx| {
            let has_items = {
                let delegate = state.delegate_mut();
                delegate.apply_query(&query);
                !delegate.filtered_items.is_empty()
            };
            state.set_selected_index(has_items.then_some(IndexPath::default()), window, cx);
            state.scroll_to_item(IndexPath::default(), ScrollStrategy::Top, window, cx);
        });
        cx.notify();
    }

    fn on_search_move_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }
        self.move_selection(-1, window, cx);
        cx.stop_propagation();
    }

    fn on_search_move_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }
        self.move_selection(1, window, cx);
        cx.stop_propagation();
    }

    fn move_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(templates) = &self.templates else {
            return;
        };

        templates.update(cx, |state, cx| {
            let count = state.delegate().filtered_items.len();
            if count == 0 {
                state.set_selected_index(None, window, cx);
                return;
            }

            let current = state.selected_index().map(|ix| ix.row).unwrap_or(0);
            let next = if delta < 0 {
                if current == 0 { count - 1 } else { current - 1 }
            } else if current + 1 >= count {
                0
            } else {
                current + 1
            };
            let next_ix = IndexPath::default().row(next);
            state.set_selected_index(Some(next_ix), window, cx);
            state.scroll_to_item(next_ix, ScrollStrategy::Top, window, cx);
        });
        cx.notify();
    }

    fn on_search_enter(&mut self, enter: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search_input.focus_handle(cx).is_focused(window) {
            return;
        }

        let Ok(templates) = &self.templates else {
            return;
        };

        templates.update(cx, |state, cx| {
            let selected = state.selected_index();
            state
                .delegate_mut()
                .set_selected_index(selected, window, cx);
            state.delegate_mut().confirm(enter.secondary, window, cx);
        });
        cx.stop_propagation();
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
            .on_action(cx.listener(Self::on_search_move_up))
            .on_action(cx.listener(Self::on_search_move_down))
            .on_action(cx.listener(Self::on_search_enter))
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.search_input)
                            .flex_1()
                            .prefix(
                                Icon::new(IconName::Search).text_color(cx.theme().muted_foreground),
                            )
                            .cleanable(true),
                    )
                    .child(
                        Button::new("template-add")
                            .primary()
                            .label(add_label)
                            .on_click(cx.listener(|_view, _, window, cx| {
                                window.dispatch_action(Add.boxed_clone(), cx);
                            })),
                    ),
            )
            .map(|this| match &self.templates {
                Ok(templates) => this.child(List::new(templates).large().px_2().py_2()),
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

fn filter_templates(
    items: &[Rc<ConversationTemplate>],
    query: &str,
) -> Vec<Rc<ConversationTemplate>> {
    let query = query.trim();
    if query.is_empty() {
        return items.to_vec();
    }

    items
        .iter()
        .filter(|item| template_matches_query(item, query))
        .cloned()
        .collect()
}

fn template_matches_query(template: &ConversationTemplate, query: &str) -> bool {
    template.name.contains(query)
        || template
            .description
            .as_ref()
            .is_some_and(|description| description.contains(query))
}

#[cfg(test)]
mod tests {
    use super::filter_templates;
    use crate::database::ConversationTemplatePrompt;
    use crate::database::{ConversationTemplate, Role};
    use std::rc::Rc;
    use time::OffsetDateTime;

    fn template(
        id: i32,
        name: &str,
        description: Option<&str>,
        prompt_count: usize,
    ) -> Rc<ConversationTemplate> {
        Rc::new(ConversationTemplate {
            id,
            name: name.to_string(),
            icon: "🤖".to_string(),
            description: description.map(ToString::to_string),
            prompts: (0..prompt_count)
                .map(|_| ConversationTemplatePrompt {
                    prompt: "hello".to_string(),
                    role: Role::User,
                })
                .collect(),
            created_time: OffsetDateTime::UNIX_EPOCH,
            updated_time: OffsetDateTime::UNIX_EPOCH,
        })
    }

    #[test]
    fn filter_templates_returns_all_for_blank_query() {
        let items = vec![
            template(1, "小说", None, 1),
            template(2, "命名助手", Some("生成更好的名字"), 2),
        ];

        let filtered = filter_templates(&items, "   ");

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].id, 1);
        assert_eq!(filtered[1].id, 2);
    }

    #[test]
    fn filter_templates_matches_name() {
        let items = vec![
            template(1, "小说", None, 1),
            template(2, "命名助手", Some("生成更好的名字"), 2),
        ];

        let filtered = filter_templates(&items, "命名");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 2);
    }

    #[test]
    fn filter_templates_matches_description() {
        let items = vec![
            template(1, "小说", Some("写奇幻冒险故事"), 1),
            template(2, "命名助手", Some("生成更好的名字"), 2),
        ];

        let filtered = filter_templates(&items, "奇幻");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 1);
    }

    #[test]
    fn filter_templates_trims_query_before_matching() {
        let items = vec![
            template(1, "小说", None, 1),
            template(2, "命名助手", Some("生成更好的名字"), 2),
        ];

        let filtered = filter_templates(&items, "  命名  ");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, 2);
    }
}
