use crate::foundation::pi_resources::{self as io, Catalog, Error, Kind, Resource};
use gpui_kit::*;
use gpui_operation::{Cancel, Complete, Load, Refresh, Repair, Retry, Transition, refresh, repair};
use gpui_tokio::Tokio;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) enum Change {
    Package {
        command: PathBuf,
        action: &'static str,
        source: String,
    },
    Toggle(Resource, bool),
    RegisterSkill(PathBuf),
    Save {
        path: PathBuf,
        text: String,
        create: bool,
    },
    Delete(Resource),
}
pub(crate) struct ResourceController {
    pub catalog: refresh::Operation<Catalog, Error, Task<()>>,
    pub mutation: repair::Operation<(), Error, (), Task<()>>,
}
pub(crate) enum ResourceEvent {
    Saved(PathBuf, String),
    Finished {
        target: String,
        result: Result<(), Error>,
    },
}
impl EventEmitter<ResourceEvent> for ResourceController {}
impl ResourceController {
    pub fn new() -> Self {
        Self {
            catalog: refresh::Operation::new(),
            mutation: repair::Operation::new(),
        }
    }
    pub fn busy(&self) -> bool {
        self.catalog.is_running() || self.mutation.is_running()
    }
    pub fn stop(&mut self) {
        self.catalog.transition(Cancel);
        self.mutation.transition(Cancel);
    }
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let worker = cx.background_spawn(async {
            let root = io::agent_dir()?;
            io::scan(root, dirs_next::home_dir().map(|p| p.join(".agents")))
        });
        let task = cx.spawn(async move |owner, cx| {
            let result = worker.await;
            let _ = owner.update(cx, |owner, cx| {
                owner.catalog.transition(Complete(result));
                cx.notify();
            });
        });
        match &self.catalog {
            refresh::Operation::Idle(_) => self.catalog.transition(Load(task)),
            refresh::Operation::Unavailable(_) => self.catalog.transition(Retry(task)),
            _ => self.catalog.transition(Refresh(task)),
        }
        cx.notify();
    }
    pub fn change(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(root) = self.catalog.data().map(|c| c.root.clone()) else {
            return;
        };
        let target = match &change {
            Change::Package { source, .. } => source.clone(),
            Change::Toggle(resource, _) | Change::Delete(resource) => resource.name.clone(),
            Change::RegisterSkill(path) | Change::Save { path, .. } => path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        };
        let saved = match &change {
            Change::Save { path, text, .. } => Some((path.clone(), text.clone())),
            _ => None,
        };
        let worker = Tokio::spawn(cx, async move {
            match change {
                Change::Package {
                    command,
                    action,
                    source,
                } => io::package_action(command, root, action, source).await,
                change => tokio::task::spawn_blocking(move || match change {
                    Change::Toggle(resource, active) => io::set_enabled(&root, &resource, active),
                    Change::RegisterSkill(path) => io::register_skill(&root, &path),
                    Change::Save { path, text, create } => io::save_text(&path, &text, create),
                    Change::Delete(resource) => {
                        if !resource.editable
                            || resource.package.is_some()
                            || !matches!(resource.kind, Kind::Skill | Kind::Prompt)
                        {
                            return Err(Error("Resource is read-only".into()));
                        }
                        trash::delete(&resource.path).map_err(|e| Error(e.to_string()))
                    }
                    Change::Package { .. } => unreachable!(),
                })
                .await
                .map_err(|e| Error(e.to_string()))?,
            }
        });
        let task = cx.spawn(async move |owner, cx| {
            let result = worker.await.unwrap_or_else(|e| Err(Error(e.to_string())));
            let _ = owner.update(cx, |owner, cx| {
                let success = result.is_ok();
                owner.mutation.transition(Complete(result.clone()));
                cx.emit(ResourceEvent::Finished { target, result });
                if success && let Some((path, text)) = saved {
                    cx.emit(ResourceEvent::Saved(path, text));
                }
                // A CLI failure may still have changed files. Always reread the actual state.
                owner.refresh(cx);
                cx.notify();
            });
        });
        match &self.mutation {
            repair::Operation::Idle(_) => self.mutation.transition(Load(task)),
            repair::Operation::Ready(_) => self.mutation.transition(Refresh(task)),
            _ => self.mutation.transition(Repair { repair: (), task }),
        }
        cx.notify();
    }
}
