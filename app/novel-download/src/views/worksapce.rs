use async_compat::Compat;
use futures::AsyncWriteExt;
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, StyledExt, button::Button, input::TextInput, label::Label};
use smol::fs::{File, OpenOptions};

use crate::{
    crawler::{Fetch, NovelBaseData},
    errors::{NovelError, NovelResult},
};
enum WorkspaceEvent {
    Send(String),
    FetchFileError,
    FetchSuccess,
    FetchStart,
    Fetching(FetchingNovelData),
    FetchNetworkError,
    FetchParseError,
}

#[derive(Debug, Clone)]
struct FetchingNovelData {
    name: String,
    author: String,
}

#[derive(Default, Clone)]
enum FetchState {
    #[default]
    None,
    Fetching(Option<FetchingNovelData>),
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
    fn render_state(&self) -> Option<Label> {
        let element = match &self.fetch_state {
            FetchState::None => return None,
            FetchState::Fetching(novel_data) => Label::new(match novel_data {
                Some(FetchingNovelData { name, author }) => format!("Fetching {name} by {author}"),
                None => "Fetching...".to_string(),
            }),
            FetchState::Success => Label::new("Success"),
            FetchState::FileError => Label::new("File Error"),
            FetchState::NetworkError => Label::new("Network Error"),
            FetchState::ParseError => Label::new("Parse Error"),
        };
        Some(element)
    }
    fn loading(&self) -> bool {
        match &self.fetch_state {
            FetchState::None => false,
            FetchState::Fetching(_) => true,
            _ => false,
        }
    }
}

impl EventEmitter<WorkspaceEvent> for Workspace {}

struct Runner {
    novel_id: String,
    workspace: Entity<Workspace>,
    cx: AsyncApp,
}

impl Runner {
    fn emit(&mut self, event: WorkspaceEvent) {
        if let Err(_err) = self.workspace.update(&mut self.cx, |_, cx| cx.emit(event)) {
            // todo log
        }
    }
}

impl Fetch for Runner {
    type BaseData = File;
    fn on_start(&mut self) -> NovelResult<()> {
        self.emit(WorkspaceEvent::FetchStart);
        Ok(())
    }
    async fn on_fetch_base(&mut self, base_data: NovelBaseData<'_>) -> NovelResult<Self::BaseData> {
        self.emit(WorkspaceEvent::Fetching(FetchingNovelData {
            name: base_data.name.to_string(),
            author: base_data.author_name.to_string(),
        }));
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
        content: &str,
        base_data: &mut Self::BaseData,
    ) -> NovelResult<()> {
        base_data.write_all(content.as_bytes()).await?;
        Ok(())
    }

    fn get_novel_id(&self) -> &str {
        &self.novel_id
    }

    fn on_success(&mut self, _base_data: &mut Self::BaseData) -> NovelResult<()> {
        self.emit(WorkspaceEvent::FetchSuccess);
        Ok(())
    }

    fn on_error(&mut self, error: &NovelError) {
        // todo log error
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
        }
    }
}

pub struct WorkspaceView {
    input: Entity<TextInput>,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,
}

impl WorkspaceView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspce = cx.new(|_| Workspace::default());
        cx.subscribe(&workspce, Self::subscribe).detach();
        Self {
            input: cx.new(|cx| TextInput::new(window, cx)),
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
                    data.fetch_state = FetchState::Fetching(None);
                });
            }
            WorkspaceEvent::Fetching(novel_data) => {
                subscriber.update(cx, |data, _| {
                    data.fetch_state = FetchState::Fetching(Some(novel_data.clone()));
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
        }
    }
    fn fetch(&mut self, subscriber: Entity<Workspace>, cx: &mut Context<Self>, novel_id: String) {
        let task = cx.spawn(|_, cx| {
            let mut runner = Runner {
                novel_id,
                workspace: subscriber,
                cx,
            };
            Compat::new(async move { runner.fetch().await })
        });
        task.detach();
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state_element = self.workspace.read(cx).render_state();
        let loading = self.workspace.read(cx).loading();
        div()
            .track_focus(&self.focus_handle)
            .key_context("NovelDownload")
            .p_4()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div().h_flex().gap_1().child(self.input.clone()).child(
                    Button::new("send")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.workspace.update(cx, |_data, cx| {
                                let text = this.input.read(cx).text();
                                cx.emit(WorkspaceEvent::Send(text.to_string()));
                                this.input.update(cx, |this, cx| {
                                    this.set_text("", window, cx);
                                });
                                this.focus_handle.focus(window);
                            });
                        }))
                        .loading(loading)
                        .child("send")
                        .track_focus(&self.focus_handle),
                ),
            )
            .when_some(state_element, |this, element| this.child(element))
    }
}
