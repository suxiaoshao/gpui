use crate::{foundation::assets::IconName, foundation::search::field_matches_query};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, StyledExt,
    group_box::{GroupBox, GroupBoxVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    resizable::{h_resizable, resizable_panel},
    scroll::ScrollableElement,
    v_flex,
};
use std::rc::Rc;

pub(super) const SETTINGS_SIDEBAR_DEFAULT_WIDTH: Pixels = px(280.);
pub(super) const SETTINGS_SIDEBAR_MIN_WIDTH: Pixels = px(240.);
pub(super) const SETTINGS_SIDEBAR_MAX_WIDTH: Pixels = px(380.);

type ResizeHandler = Rc<dyn Fn(Pixels, &mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(SettingsPageKey, &mut Window, &mut App)>;
type SettingsRowRender = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsPageKey {
    General,
    Appearance,
    Provider,
    Projects,
}

#[derive(Clone, Debug)]
pub(super) struct SettingsPageSpec {
    pub(super) key: SettingsPageKey,
    pub(super) title: SharedString,
    pub(super) search_text: String,
}

impl SettingsPageSpec {
    pub(super) fn new(
        key: SettingsPageKey,
        title: impl Into<SharedString>,
        search_text: String,
    ) -> Self {
        Self {
            key,
            title: title.into(),
            search_text,
        }
    }
}

#[derive(IntoElement)]
pub(super) struct SettingsShell {
    sidebar_width: Pixels,
    search_input: Entity<InputState>,
    pages: Vec<SettingsPageSpec>,
    active_page: SettingsPageKey,
    body: AnyElement,
    on_resize: ResizeHandler,
    on_select: SelectHandler,
}

impl SettingsShell {
    pub(super) fn new(
        sidebar_width: Pixels,
        search_input: Entity<InputState>,
        pages: Vec<SettingsPageSpec>,
        active_page: SettingsPageKey,
        body: impl IntoElement,
    ) -> Self {
        Self {
            sidebar_width,
            search_input,
            pages,
            active_page,
            body: body.into_any_element(),
            on_resize: Rc::new(|_, _, _| {}),
            on_select: Rc::new(|_, _, _| {}),
        }
    }

    pub(super) fn on_resize(
        mut self,
        handler: impl Fn(Pixels, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Rc::new(handler);
        self
    }

    pub(super) fn on_select(
        mut self,
        handler: impl Fn(SettingsPageKey, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Rc::new(handler);
        self
    }
}

impl RenderOnce for SettingsShell {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_resize = self.on_resize.clone();

        h_resizable("settings-layout")
            .on_resize(move |state, window, cx| {
                let width = state
                    .read(cx)
                    .sizes()
                    .first()
                    .copied()
                    .unwrap_or(SETTINGS_SIDEBAR_DEFAULT_WIDTH);
                on_resize(width, window, cx);
            })
            .child(
                resizable_panel()
                    .size(self.sidebar_width)
                    .size_range(SETTINGS_SIDEBAR_MIN_WIDTH..SETTINGS_SIDEBAR_MAX_WIDTH)
                    .child(
                        SettingsNav::new(self.search_input, self.pages, self.active_page)
                            .on_select({
                                let on_select = self.on_select.clone();
                                move |key, window, cx| on_select(key, window, cx)
                            }),
                    ),
            )
            .child(resizable_panel().child(div().size_full().overflow_hidden().child(self.body)))
    }
}

#[derive(Clone, IntoElement)]
struct SettingsNav {
    search_input: Entity<InputState>,
    pages: Vec<SettingsPageSpec>,
    active_page: SettingsPageKey,
    on_select: SelectHandler,
}

impl SettingsNav {
    fn new(
        search_input: Entity<InputState>,
        pages: Vec<SettingsPageSpec>,
        active_page: SettingsPageKey,
    ) -> Self {
        Self {
            search_input,
            pages,
            active_page,
            on_select: Rc::new(|_, _, _| {}),
        }
    }

    fn on_select(
        mut self,
        handler: impl Fn(SettingsPageKey, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Rc::new(handler);
        self
    }
}

impl RenderOnce for SettingsNav {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_select = self.on_select.clone();

        v_flex()
            .id("settings-nav")
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().sidebar)
            .text_color(cx.theme().sidebar_foreground)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .p_3()
            .gap_3()
            .child(
                Input::new(&self.search_input)
                    .w_full()
                    .prefix(Icon::new(IconName::Search).text_color(cx.theme().muted_foreground)),
            )
            .child(v_flex().w_full().flex_1().min_h_0().gap_1().children(
                self.pages.into_iter().map(move |page| {
                    let active = page.key == self.active_page;
                    let on_select = on_select.clone();
                    h_flex()
                        .id(("settings-nav-item", page.key as usize))
                        .w_full()
                        .min_w_0()
                        .h(px(32.))
                        .px_2()
                        .items_center()
                        .rounded(cx.theme().radius)
                        .cursor_pointer()
                        .text_color(cx.theme().sidebar_foreground)
                        .when(active, |this| {
                            this.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().sidebar_accent_foreground)
                        })
                        .hover(|this| {
                            this.bg(cx.theme().sidebar_accent)
                                .text_color(cx.theme().sidebar_accent_foreground)
                        })
                        .on_click(move |_, window, cx| on_select(page.key, window, cx))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Label::new(page.title).text_sm()),
                        )
                }),
            ))
    }
}

#[derive(IntoElement)]
pub(super) struct SettingsPageFrame {
    title: SharedString,
    body: AnyElement,
    body_scroll: SettingsPageBodyScroll,
}

impl SettingsPageFrame {
    pub(super) fn new(title: impl Into<SharedString>, body: impl IntoElement) -> Self {
        Self {
            title: title.into(),
            body: body.into_any_element(),
            body_scroll: SettingsPageBodyScroll::Outer,
        }
    }

    pub(super) fn no_outer_body_scroll(mut self) -> Self {
        self.body_scroll = SettingsPageBodyScroll::None;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsPageBodyScroll {
    Outer,
    None,
}

impl RenderOnce for SettingsPageFrame {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let body = match self.body_scroll {
            SettingsPageBodyScroll::Outer => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .w_full()
                .overflow_hidden()
                .child(
                    div().size_full().overflow_y_scrollbar().child(
                        v_flex()
                            .w_full()
                            .min_w_0()
                            .max_w(px(960.))
                            .gap_4()
                            .p_4()
                            .child(self.body),
                    ),
                )
                .into_any_element(),
            SettingsPageBodyScroll::None => div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .w_full()
                .overflow_hidden()
                .p_4()
                .child(self.body)
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .overflow_hidden()
            .child(
                v_flex()
                    .flex_none()
                    .p_4()
                    .gap_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(Label::new(self.title).text_lg().font_medium()),
            )
            .child(body)
    }
}

pub(super) fn settings_group(
    title: impl Into<SharedString>,
    items: impl IntoIterator<Item = SettingsRowItem>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    GroupBox::new()
        .outline()
        .title(v_flex().gap_1().child(Label::new(title.into()).text_sm()))
        .gap_4()
        .children(items.into_iter().map(|item| item.render(window, cx)))
        .border_color(cx.theme().border)
        .into_any_element()
}

pub(super) struct SettingsRowItem {
    render: SettingsRowRender,
}

impl SettingsRowItem {
    fn render(self, window: &mut Window, cx: &mut App) -> AnyElement {
        (self.render)(window, cx)
    }
}

pub(super) fn settings_row_item(
    label: impl Into<SharedString>,
    control: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> SettingsRowItem {
    let label = label.into();
    SettingsRowItem {
        render: Rc::new(move |window, cx| settings_row(label.clone(), control(window, cx))),
    }
}

fn settings_row(label: SharedString, control: AnyElement) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .justify_between()
        .gap_4()
        .flex_wrap()
        .child(
            v_flex()
                .flex_1()
                .min_w(px(220.))
                .gap_1()
                .child(Label::new(label).text_sm().truncate()),
        )
        .child(div().flex_none().min_w(px(240.)).child(control))
        .into_any_element()
}

pub(super) fn settings_empty_message(message: impl Into<SharedString>) -> AnyElement {
    v_flex()
        .size_full()
        .min_h(px(280.))
        .items_center()
        .justify_center()
        .child(Label::new(message).text_sm())
        .into_any_element()
}

pub(super) fn settings_page_matches(spec: &SettingsPageSpec, query: &str) -> bool {
    field_matches_query(&spec.search_text, query)
}

pub(super) fn settings_search_text<I, S>(labels: I, extra: &str) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut text = labels
        .into_iter()
        .map(|label| label.as_ref().to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    text.push(' ');
    text.push_str(&extra.to_lowercase());
    text
}
