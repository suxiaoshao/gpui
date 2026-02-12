use crate::{
    crawler::{ContentItem, Fetch, NovelBaseData},
    errors::{NovelError, NovelResult},
    i18n::I18n,
};
use async_compat::Compat;
use fluent_bundle::FluentArgs;
use futures::AsyncWriteExt;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, StyledExt,
    button::Button,
    input::{Input, InputState},
    label::Label,
    link::Link,
};
use smol::fs::{File, OpenOptions};
use tracing::{Instrument, Level, event};

enum WorkspaceEvent {
    Send(String),
    FetchFileError,
    FetchSuccess,
    FetchStart,
    FetchingNovel { name: String, author: String },
    FetchingChapter(String),
    FetchNetworkError,
    FetchParseError,
}

#[derive(Default, Clone)]
enum FetchState {
    #[default]
    None,
    Fetching,
    FetchingNovel {
        name: String,
        author: String,
        history: Vec<String>,
    },
    Success,
    FileError,
    NetworkError,
    ParseError,
}

#[derive(Default)]
struct Workspace {
    fetch_state: FetchState,
}

impl Workspace {
    fn render_state(&self, i18n: &I18n) -> Option<Div> {
        let element = match &self.fetch_state {
            FetchState::None => return None,
            FetchState::FetchingNovel {
                name,
                author,
                history,
            } => {
                let mut args = FluentArgs::new();
                args.set("name", name.clone());
                args.set("author", author.clone());
                div()
                    .flex()
                    .flex_col()
                    .child(Label::new(i18n.t_with_args("fetch-state-fetching-novel", &args)))
                    .children(history.iter().cloned().rev().take(5).rev().map(|url| {
                        Link::new(SharedString::from(&url))
                            .child(SharedString::from(&url))
                            .href(url)
                    }))
            }
            FetchState::Success => div().child(Label::new(i18n.t("fetch-state-success"))),
            FetchState::FileError => div().child(Label::new(i18n.t("fetch-state-file-error"))),
            FetchState::NetworkError => div().child(Label::new(i18n.t("fetch-state-network-error"))),
            FetchState::ParseError => div().child(Label::new(i18n.t("fetch-state-parse-error"))),
            FetchState::Fetching => div().child(Label::new(i18n.t("fetch-state-fetching"))),
        };
        Some(element)
    }
    fn loading(&self) -> bool {
        match &self.fetch_state {
            FetchState::None => false,
            FetchState::FetchingNovel { .. } => true,
            _ => false,
        }
    }
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

struct Runner<'a> {
    novel_id: String,
    workspace: Entity<Workspace>,
    cx: &'a mut AsyncApp,
}

impl Runner<'_> {
    fn emit(&mut self, event: WorkspaceEvent) {
        if let Err(err) = self.workspace.update(self.cx, |_, cx| cx.emit(event)) {
            event!(Level::ERROR, "Failed to emit event:{err}");
        }
    }
}

impl Fetch for Runner<'_> {
    type BaseData = File;
    fn on_start(&mut self) -> NovelResult<()> {
        event!(Level::INFO, "Starting fetch");
        self.emit(WorkspaceEvent::FetchStart);
        Ok(())
    }
    async fn on_fetch_base(&mut self, base_data: NovelBaseData<'_>) -> NovelResult<Self::BaseData> {
        event!(Level::INFO, "Fetching base data");
        self.emit(WorkspaceEvent::FetchingNovel {
            name: base_data.name.to_string(),
            author: base_data.author_name.to_string(),
        });
        let path = dirs_next::download_dir()
            .ok_or(NovelError::DownloadFolder)?
            .join(format!("{}by{}.txt", base_data.name, base_data.author_name));
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .await?;
        Ok(file)
    }

    async fn on_add_content(
        &mut self,
        content: &ContentItem,
        base_data: &mut Self::BaseData,
    ) -> NovelResult<()> {
        event!(Level::INFO, "Fetching chapter:{}", content.url);
        self.emit(WorkspaceEvent::FetchingChapter(content.url.clone()));
        base_data.write_all(content.content.as_bytes()).await?;
        Ok(())
    }

    fn get_novel_id(&self) -> &str {
        &self.novel_id
    }

    fn on_success(&mut self, _base_data: &mut Self::BaseData) -> NovelResult<()> {
        event!(Level::INFO, "Fetch success");
        self.emit(WorkspaceEvent::FetchSuccess);
        Ok(())
    }

    fn on_error(&mut self, error: &NovelError) {
        event!(Level::ERROR, "Fetch error: {}", error);
        match error {
            NovelError::NetworkError(_) => {
                self.emit(WorkspaceEvent::FetchNetworkError);
            }
            NovelError::ParseError => {
                self.emit(WorkspaceEvent::FetchParseError);
            }
            NovelError::Fs(_) | NovelError::DownloadFolder => {
                self.emit(WorkspaceEvent::FetchFileError);
            }
            NovelError::LogFileNotFound => {
                event!(Level::ERROR, "Log file not found");
                unimplemented!()
            }
        }
    }
}

pub struct WorkspaceView {
    input: Entity<InputState>,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
}

impl WorkspaceView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspce = cx.new(|_| Workspace::default());
        cx.subscribe(&workspce, Self::subscribe).detach();
        Self {
            input: cx.new(|cx| InputState::new(window, cx)),
            workspace: workspce,
            focus_handle: cx.focus_handle(),
        }
    }
    fn subscribe(
        &mut self,
        subscriber: Entity<Workspace>,
        emitter: &WorkspaceEvent,
        cx: &mut Context<Self>,
    ) {
        match emitter {
            WorkspaceEvent::Send(text) => {
                self.fetch(subscriber, cx, text.to_string());
            }
            WorkspaceEvent::FetchStart => {
                subscriber.update(cx, |data, _| {
                    data.fetch_state = FetchState::Fetching;
                });
            }
            WorkspaceEvent::FetchingNovel { author, name } => {
                subscriber.update(cx, |data, _| {
                    data.fetch_state = FetchState::FetchingNovel {
                        author: author.clone(),
                        name: name.clone(),
                        history: Vec::new(),
                    };
                });
            }
            WorkspaceEvent::FetchNetworkError => {
                subscriber.update(cx, |data, _| {
                    data.fetch_state = FetchState::NetworkError;
                });
            }
            WorkspaceEvent::FetchParseError => {
                subscriber.update(cx, |data, _| {
                    data.fetch_state = FetchState::ParseError;
                });
            }
            WorkspaceEvent::FetchFileError => {
                subscriber.update(cx, |data, _| {
                    data.fetch_state = FetchState::FileError;
                });
            }
            WorkspaceEvent::FetchSuccess => {
                subscriber.update(cx, |data, _| {
                    data.fetch_state = FetchState::Success;
                });
            }
            WorkspaceEvent::FetchingChapter(url) => {
                subscriber.update(cx, |data, _| {
                    if let FetchState::FetchingNovel { history, .. } = &mut data.fetch_state {
                        history.push(url.clone());
                    };
                });
            }
        }
        cx.notify();
    }
    fn fetch(&mut self, subscriber: Entity<Workspace>, cx: &mut Context<Self>, novel_id: String) {
        let task = cx.spawn(async move |_, cx| {
            let mut runner = Runner {
                novel_id: novel_id.clone(),
                workspace: subscriber,
                cx,
            };
            Compat::new(async move {
                let span = tracing::info_span!("send", novel_id);
                runner.fetch().instrument(span).await
            })
            .await
        });
        task.detach();
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (state_element, send_label) = {
            let i18n = cx.global::<I18n>();
            (
                self.workspace.read(cx).render_state(i18n),
                i18n.t("button-send"),
            )
        };
        let loading = self.workspace.read(cx).loading();
        div()
            .track_focus(&self.focus_handle)
            .key_context("NovelDownload")
            .p_4()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div().h_flex().gap_1().child(Input::new(&self.input)).child(
                    Button::new("send")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.workspace.update(cx, |_data, cx| {
                                let text = this.input.read(cx).text();
                                cx.emit(WorkspaceEvent::Send(text.to_string()));
                                this.input.update(cx, |this, cx| {
                                    this.set_value("", window, cx);
                                });
                                this.focus_handle.focus(window);
                            });
                        }))
                        .loading(loading)
                        .child(send_label)
                        .track_focus(&self.focus_handle),
                ),
            )
            .when_some(state_element, |this, element| this.child(element))
    }
}
