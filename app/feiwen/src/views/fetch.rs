use components::{button, input_border, IntInput, TextInput};
use gpui::*;
use theme::Theme;

use super::{
    workspace::{RouterType, WorkspaceEvent},
    Workspace,
};

enum FetchFormEvent {
    SetUrl(String),
    SetStartPage(u32),
    SetEedPage(u32),
    SetCookie(String),
    Fetch,
}

#[derive(Default)]
struct FetchForm {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
}

impl EventEmitter<FetchFormEvent> for FetchForm {}

#[derive(Clone)]
pub struct FetchView {
    workspace: Model<Workspace>,
    url_input: View<TextInput>,
    start_page: View<IntInput>,
    end_page: View<IntInput>,
    cookie_input: View<TextInput>,
    form: Model<FetchForm>,
}

impl FetchView {
    pub fn new(workspace: Model<Workspace>, cx: &mut ViewContext<Self>) -> Self {
        let url_on_change = cx.listener(|this, data: &SharedString, cx| {
            this.form.update(cx, |_form, cx| {
                cx.emit(FetchFormEvent::SetUrl(data.to_string()));
            });
        });
        let cookie_on_change = cx.listener(|this, data: &SharedString, cx| {
            this.form.update(cx, |_form, cx| {
                cx.emit(FetchFormEvent::SetCookie(data.to_string()));
            });
        });
        let start_page_on_change = cx.listener(|this, data: &u32, cx| {
            this.form.update(cx, |_form, cx| {
                cx.emit(FetchFormEvent::SetStartPage(*data));
            });
        });
        let end_page_on_change = cx.listener(|this, data: &u32, cx| {
            this.form.update(cx, |_form, cx| {
                cx.emit(FetchFormEvent::SetEedPage(*data));
            });
        });
        let form = cx.new_model(|_cx| Default::default());
        cx.subscribe(&form, Self::subscribe).detach();
        Self {
            workspace,
            url_input: cx.new_view(|cx| TextInput::new(cx, "", "Url").on_change(url_on_change)),
            start_page: cx
                .new_view(|cx| IntInput::new(cx, 0, "Start Page").on_change(start_page_on_change)),
            end_page: cx
                .new_view(|cx| IntInput::new(cx, 0, "End Page").on_change(end_page_on_change)),
            cookie_input: cx
                .new_view(|cx| TextInput::new(cx, "", "Cookie").on_change(cookie_on_change)),
            form,
        }
    }
    fn subscribe(
        &mut self,
        subscriber: Model<FetchForm>,
        emitter: &FetchFormEvent,
        cx: &mut ViewContext<Self>,
    ) {
        match emitter {
            FetchFormEvent::SetUrl(url) => {
                subscriber.update(cx, |data, _cx| {
                    data.url.clone_from(url);
                });
            }
            FetchFormEvent::SetStartPage(start_page) => {
                subscriber.update(cx, |data, _cx| {
                    data.start_page = *start_page;
                });
            }
            FetchFormEvent::SetEedPage(end_page) => {
                subscriber.update(cx, |data, _cx| {
                    data.end_page = *end_page;
                });
            }
            FetchFormEvent::SetCookie(cookie) => {
                subscriber.update(cx, |data, _cx| {
                    data.cookie.clone_from(cookie);
                });
            }
            FetchFormEvent::Fetch => unimplemented!(),
        };
    }
}

impl Render for FetchView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        div()
            .h_full()
            .w_full()
            .flex()
            .flex_col()
            .child(
                button("router-query")
                    .child("query")
                    .on_click(cx.listener(|this, _, cx| {
                        this.workspace.update(cx, |_data, cx| {
                            cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Query));
                        });
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p_1()
                    .gap_1()
                    .child(input_border(theme).child(self.url_input.clone()))
                    .child(input_border(theme).child(self.start_page.clone()))
                    .child(input_border(theme).child(self.end_page.clone()))
                    .child(input_border(theme).child(self.cookie_input.clone())),
            )
            .child(
                button("fetch")
                    .child("Fetch")
                    .on_click(cx.listener(|this, _, cx| {
                        this.form.update(cx, |_data, cx| {
                            cx.emit(FetchFormEvent::Fetch);
                        });
                    })),
            )
    }
}
