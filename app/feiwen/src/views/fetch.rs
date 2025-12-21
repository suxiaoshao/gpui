use super::{
    Workspace,
    workspace::{RouterType, WorkspaceEvent},
};
use crate::{
    errors::{FeiwenError, FeiwenResult},
    fetch::{self, FetchRunner},
    store::{Db, service::Novel},
};
use async_compat::Compat;
use diesel::{
    SqliteConnection,
    r2d2::{ConnectionManager, PooledConnection},
};
use gpui::*;
use gpui_component::{
    button::Button,
    input::{Input, InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
};
use prelude::FluentBuilder;
use regex::Regex;
use tracing::{Instrument, Level, event};

enum FetchFormEvent {
    SetUrl(String),
    SetStartPage(u32),
    SetEndPage(u32),
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

struct Runner<'a> {
    url: String,
    start_page: u32,
    end_page: u32,
    cookie: String,
    form: Entity<FetchForm>,
    conn: PooledConnection<ConnectionManager<SqliteConnection>>,
    cx: &'a mut AsyncApp,
}

impl fetch::FetchRunner for Runner<'_> {
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

impl Runner<'_> {
    async fn run(&mut self) {
        self.on_start();
        match self.fetch().await {
            Ok(_) => {
                self.on_success();
            }
            Err(err) => match err {
                FeiwenError::Sqlite(_)
                | FeiwenError::Connection(_)
                | FeiwenError::Pool(_)
                | FeiwenError::GetConnection(_) => {
                    event!(Level::ERROR, "Failed to fetch database: {:?}", err);
                    self.form_emit(FetchFormEvent::FetchDbError);
                }
                FeiwenError::HeaderParse(_) | FeiwenError::Request(_) => {
                    event!(Level::ERROR, "Failed to fetch network: {:?}", err);
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
                    event!(Level::ERROR, "Failed to fetch parse: {:?}", err);
                    self.form_emit(FetchFormEvent::FetchParseError);
                }
                err => {
                    event!(Level::ERROR, "Failed to fetch other: {:?}", err);
                }
            },
        };
    }
    fn on_success(&mut self) {
        self.form_emit(FetchFormEvent::FetchSuccess);
    }
    fn on_start(&mut self) {
        event!(Level::INFO, "Start fetch");
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
        if let Err(err) = self.form.update(self.cx, |_, cx| cx.emit(event)) {
            event!(Level::ERROR, "Failed to emit event: {:?}", err);
        }
    }
}

impl EventEmitter<FetchFormEvent> for FetchForm {}

pub(crate) struct FetchView {
    workspace: Entity<Workspace>,
    url_input: Entity<InputState>,
    start_page: Entity<InputState>,
    end_page: Entity<InputState>,
    cookie_input: Entity<InputState>,
    form: Entity<FetchForm>,
    _subscriptions: Vec<Subscription>,
}

impl FetchView {
    pub(crate) fn new(
        window: &mut Window,
        workspace: Entity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let integer_regex = Regex::new(r"^\d+$").unwrap();
        let mut _subscriptions = vec![];
        let url_input = cx.new(|cx| InputState::new(window, cx).placeholder("Url"));
        let start_page = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Start Page")
                .pattern(integer_regex.clone())
        });
        let end_page = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("End Page")
                .pattern(integer_regex)
        });
        let cookie_input = cx.new(|cx| InputState::new(window, cx).placeholder("Cookie"));

        _subscriptions.push(cx.subscribe_in(
            &url_input,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetUrl(text.to_string()));
                    });
                }
                InputEvent::PressEnter { .. } => {
                    view.start_page.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &start_page,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetStartPage(text.parse().unwrap_or(1)));
                    });
                }
                InputEvent::PressEnter { .. } => {
                    view.end_page.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &start_page,
            window,
            |view, state, event, window, cx| match event {
                NumberInputEvent::Step(StepAction::Decrement) => {
                    let start_page = match view.form.read(cx).start_page {
                        0 => 0,
                        n => n - 1,
                    };
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetStartPage(start_page));
                    });
                    state.update(cx, |input, cx| {
                        input.set_value(start_page.to_string(), window, cx);
                    });
                }
                NumberInputEvent::Step(StepAction::Increment) => {
                    let start_page = view.form.read(cx).start_page + 1;
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetStartPage(start_page));
                    });
                    state.update(cx, |input, cx| {
                        input.set_value(start_page.to_string(), window, cx);
                    });
                }
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &end_page,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetEndPage(text.parse().unwrap_or(1)));
                    });
                }
                InputEvent::PressEnter { .. } => {
                    view.cookie_input.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &end_page,
            window,
            |view, state, event, window, cx| match event {
                NumberInputEvent::Step(StepAction::Decrement) => {
                    let end_page = match view.form.read(cx).end_page {
                        0 => 0,
                        n => n - 1,
                    };
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetEndPage(end_page));
                    });
                    state.update(cx, |input, cx| {
                        input.set_value(end_page.to_string(), window, cx);
                    });
                }
                NumberInputEvent::Step(StepAction::Increment) => {
                    let end_page = view.form.read(cx).end_page + 1;
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetEndPage(end_page));
                    });
                    state.update(cx, |input, cx| {
                        input.set_value(end_page.to_string(), window, cx);
                    });
                }
            },
        ));
        _subscriptions.push(cx.subscribe_in(
            &cookie_input,
            window,
            |view, state, event, window, cx| match event {
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    view.form.update(cx, |_form, cx| {
                        cx.emit(FetchFormEvent::SetCookie(text.to_string()));
                    });
                }
                InputEvent::PressEnter { .. } => {
                    view.url_input.update(cx, |input, cx| {
                        input.focus(window, cx);
                    });
                }
                _ => {}
            },
        ));
        let form = cx.new(|_cx| Default::default());
        _subscriptions.push(cx.subscribe(&form, Self::subscribe));
        Self {
            workspace,
            url_input,
            start_page,
            end_page,
            cookie_input,
            form,
            _subscriptions,
        }
    }
    fn subscribe(
        &mut self,
        subscriber: Entity<FetchForm>,
        emitter: &FetchFormEvent,
        cx: &mut Context<Self>,
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
            FetchFormEvent::SetEndPage(end_page) => {
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
    fn fetch(&mut self, subscriber: Entity<FetchForm>, cx: &mut Context<Self>) {
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
        let task = cx.spawn(async move |_, cx| {
            let span = tracing::info_span!("send", url, start_page, end_page, cookie);
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
                .instrument(span)
                .await
        });
        task.detach();
    }
}

impl Render for FetchView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                        Button::new("router-query")
                            .label("Go query")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.workspace.update(cx, |_data, cx| {
                                    cx.emit(WorkspaceEvent::UpdateRouter(RouterType::Query));
                                });
                            })),
                    )
                    .child(Button::new("fetch").label("Fetch").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.form.update(cx, |_data, cx| {
                                cx.emit(FetchFormEvent::StartFetch);
                            });
                        },
                    ))),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p_1()
                    .gap_1()
                    .child(Input::new(&self.url_input).flex_initial())
                    .child(NumberInput::new(&self.start_page).flex_initial())
                    .child(NumberInput::new(&self.end_page).flex_initial())
                    .child(Input::new(&self.cookie_input).flex_initial())
                    .when_some(state_element, |this, element| this.child(element)),
            )
    }
}
