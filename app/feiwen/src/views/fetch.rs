use async_compat::Compat;
use components::{button, input_border, IntInput, TextInput};
use diesel::{
    r2d2::{ConnectionManager, PooledConnection},
    SqliteConnection,
};
use gpui::*;
use prelude::FluentBuilder;
use theme::Theme;

use crate::{
    errors::{FeiwenError, FeiwenResult},
    fetch::{self, FetchRunner},
    store::{service::Novel, Db},
};

use super::{
    workspace::{RouterType, WorkspaceEvent},
    Workspace,
};

enum FetchFormEvent {
    SetUrl(String),
    SetStartPage(u32),
    SetEedPage(u32),
    SetCookie(String),
    StartFetch,
    FetchDbError,
    FetchSuccess,
    FetchStart {
        total: i64,
        page: u32,
        start_page: u32,
        end_page: u32,
    },
    Fetching {
        total: i64,
        page: u32,
        start_page: u32,
        end_page: u32,
    },
    FetchNetworkError,
    FetchParseError,
}

#[derive(Default, Clone, Copy)]
enum FetchState {
    #[default]
    None,
    Fetching {
        total: i64,
        page: u32,
        start_page: u32,
        end_page: u32,
    },
    Success,
    DbError,
    NetworkError,
    ParseError,
}

#[derive(Default, Clone)]
struct FetchForm {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
    state: FetchState,
}

impl FetchForm {
    fn render_state(&self) -> Option<Div> {
        let element = match self.state {
            FetchState::None => return None,
            FetchState::Fetching {
                total,
                page,
                start_page,
                end_page,
            } => div().child(format!(
                "Fetching page {start_page}/{page}/{end_page} of a total of {total}"
            )),
            FetchState::Success => div().child("Success"),
            FetchState::DbError => div().child("Database Error"),
            FetchState::NetworkError => div().child("Network Error"),
            FetchState::ParseError => div().child("Parse Error"),
        };
        Some(element)
    }
}

struct Runner {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
    form: Model<FetchForm>,
    conn: PooledConnection<ConnectionManager<SqliteConnection>>,
    cx: AsyncWindowContext,
}

impl fetch::FetchRunner for Runner {
    fn get_url(&self) -> &str {
        &self.url
    }

    fn get_cookies(&self) -> &str {
        &self.cookie
    }

    fn get_start(&self) -> u32 {
        self.start_page
    }

    fn get_end(&self) -> u32 {
        self.end_page
    }
    fn resolve_novel(&mut self, novels: Vec<Novel>, page: u32) -> FeiwenResult<()> {
        for novel in novels {
            novel.save(&mut self.conn)?;
        }
        let total = Novel::count(&mut self.conn)?;
        self.form_emit(FetchFormEvent::Fetching {
            total,
            page,
            start_page: self.start_page,
            end_page: self.end_page,
        });
        Ok(())
    }
}

impl Runner {
    async fn run(&mut self) {
        self.on_start();
        match self.fetch().await {
            Ok(_) => {
                self.on_success();
            }
            Err(err) => {
                // todo log
                match err {
                    FeiwenError::Sqlite(_)
                    | FeiwenError::Connection(_)
                    | FeiwenError::Pool(_)
                    | FeiwenError::GetConnection(_) => {
                        self.form_emit(FetchFormEvent::FetchDbError);
                    }
                    FeiwenError::HeaderParse(_) | FeiwenError::Request(_) => {
                        self.form_emit(FetchFormEvent::FetchNetworkError);
                    }
                    FeiwenError::DescParse
                    | FeiwenError::HrefParse
                    | FeiwenError::CountParse
                    | FeiwenError::ReadCountParse
                    | FeiwenError::WordCountParse
                    | FeiwenError::AuthorNameParse
                    | FeiwenError::NovelIdParse(_)
                    | FeiwenError::AuthorIdParse(_)
                    | FeiwenError::ChapterIdParse(_)
                    | FeiwenError::ReplyCountParse
                    | FeiwenError::CountUintParse(_) => {
                        self.form_emit(FetchFormEvent::FetchParseError);
                    }
                    _ => {
                        // todo log
                    }
                }
            }
        };
    }
    fn on_success(&mut self) {
        self.form_emit(FetchFormEvent::FetchSuccess);
    }
    fn on_start(&mut self) {
        let total = match Novel::count(&mut self.conn) {
            Ok(data) => data,
            Err(_) => {
                self.form_emit(FetchFormEvent::FetchDbError);
                return;
            }
        };
        self.form_emit(FetchFormEvent::FetchStart {
            total,
            page: self.start_page,
            start_page: self.start_page,
            end_page: self.end_page,
        });
    }
    fn form_emit(&mut self, event: FetchFormEvent) {
        if let Err(_err) = self.form.update(&mut self.cx, |_, cx| cx.emit(event)) {
            // todo log
        }
    }
}

impl EventEmitter<FetchFormEvent> for FetchForm {}

#[derive(Clone)]
pub(crate) struct FetchView {
    workspace: Model<Workspace>,
    url_input: View<TextInput>,
    start_page: View<IntInput>,
    end_page: View<IntInput>,
    cookie_input: View<TextInput>,
    form: Model<FetchForm>,
}

impl FetchView {
    pub(crate) fn new(workspace: Model<Workspace>, cx: &mut ViewContext<Self>) -> Self {
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
            FetchFormEvent::StartFetch => {
                self.fetch(subscriber, cx);
            }
            FetchFormEvent::FetchDbError => {
                subscriber.update(cx, |data, _| {
                    data.state = FetchState::DbError;
                });
            }
            FetchFormEvent::FetchSuccess => {
                subscriber.update(cx, |data, _| {
                    data.state = FetchState::Success;
                });
            }
            FetchFormEvent::FetchStart {
                total,
                page,
                start_page,
                end_page,
            }
            | FetchFormEvent::Fetching {
                total,
                page,
                start_page,
                end_page,
            } => {
                subscriber.update(cx, |data, _| {
                    data.state = FetchState::Fetching {
                        total: *total,
                        page: *page,
                        start_page: *start_page,
                        end_page: *end_page,
                    }
                });
            }
            FetchFormEvent::FetchNetworkError => {
                subscriber.update(cx, |data, _| {
                    data.state = FetchState::NetworkError;
                });
            }
            FetchFormEvent::FetchParseError => {
                subscriber.update(cx, |data, _| {
                    data.state = FetchState::ParseError;
                });
            }
        };
        cx.notify();
    }
    fn fetch(&mut self, subscriber: Model<FetchForm>, cx: &mut ViewContext<Self>) {
        let conn = cx.global::<Db>();
        let conn = match conn.get() {
            Ok(data) => data,
            Err(_) => {
                subscriber.update(cx, |_this, cx| {
                    cx.emit(FetchFormEvent::FetchDbError);
                });
                return;
            }
        };
        let form = subscriber.read(cx);
        let url = form.url.clone();
        let start_page = form.start_page;
        let end_page = form.end_page;
        let cookie = form.cookie.clone();
        let form = subscriber.clone();
        let task = cx.spawn(|_, cx| {
            let mut runner = Runner {
                conn,
                url,
                start_page,
                end_page,
                cookie,
                form,
                cx,
            };
            Compat::new(async move { runner.run().await })
        });
        task.detach();
    }
}

impl Render for FetchView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let theme = cx.global::<Theme>();
        let state_element = self.form.read(cx).render_state();
        div()
            .h_full()
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(
                        button("router-query")
                            .child("Go query")
                            .on_click(cx.listener(|this, _, cx| {
                                this.workspace.update(cx, |_data, cx| {
                                    cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Query));
                                });
                            })),
                    )
                    .child(
                        button("fetch")
                            .child("Fetch")
                            .on_click(cx.listener(|this, _, cx| {
                                this.form.update(cx, |_data, cx| {
                                    cx.emit(FetchFormEvent::StartFetch);
                                });
                            })),
                    ),
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
                    .child(input_border(theme).child(self.cookie_input.clone()))
                    .when_some(state_element, |this, element| this.child(element)),
            )
    }
}
