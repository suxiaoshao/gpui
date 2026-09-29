use super::*;

pub(super) enum Files {
    Loading { _task: Task<()> },
    Ready(Result<Vec<ProjectPath>, String>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ProjectPath {
    path: PathBuf,
    directory: bool,
}

impl CommandPalette {
    pub(super) fn sync_files(&mut self, cx: &mut Context<Self>) {
        if self.files.is_some() || !self.input.read(cx).value().starts_with('@') {
            return;
        }
        let Some(root) = self
            .home
            .as_ref()
            .and_then(|(_, state)| state.read(cx).current().map(|s| s.info.cwd.clone()))
        else {
            return;
        };
        self.files = Some(Files::Loading {
            _task: cx.spawn(async move |owner, cx| {
                let files = smol::unblock(move || scan(root)).await;
                let _ = owner.update(cx, |owner, cx| {
                    owner.files = Some(Files::Ready(files));
                    cx.notify();
                });
            }),
        });
    }

    pub(super) fn file_candidates(&self, query: &str, cx: &App) -> Vec<Row> {
        let Some(Files::Ready(Ok(files))) = &self.files else {
            return vec![];
        };
        let Some((_, state)) = &self.home else {
            return vec![];
        };
        let Some(session) = state.read(cx).current() else {
            return vec![];
        };
        let query = query.to_lowercase();
        files
            .iter()
            .filter_map(|entry| {
                let path = &entry.path;
                let relative = path
                    .strip_prefix(&session.info.cwd)
                    .unwrap_or(path)
                    .to_string_lossy();
                let haystack = relative.to_lowercase();
                if !query.split_whitespace().all(|part| haystack.contains(part)) {
                    return None;
                }
                Some(Row {
                    id: Id::File(path.clone()),
                    group: 5,
                    title: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    description: relative.into_owned(),
                    detail: String::new(),
                    tooltip: path.to_string_lossy().into_owned(),
                    icon: if entry.directory {
                        IconName::Folder
                    } else {
                        IconName::FileText
                    },
                    enabled: self.target_valid(cx)
                        && session.can_edit_draft()
                        && session.attachments_read.is_none(),
                })
            })
            .take(200)
            .collect()
    }
}

fn scan(root: PathBuf) -> Result<Vec<ProjectPath>, String> {
    let mut paths = vec![];
    for entry in ignore::WalkBuilder::new(&root).build() {
        match entry {
            Ok(entry)
                if entry.depth() > 0
                    && entry
                        .file_type()
                        .is_some_and(|kind| kind.is_file() || kind.is_dir()) =>
            {
                paths.push(ProjectPath {
                    directory: entry.file_type().is_some_and(|kind| kind.is_dir()),
                    path: entry.into_path(),
                });
            }
            Err(error) if paths.is_empty() => return Err(error.to_string()),
            _ => {}
        }
    }
    paths.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(paths)
}
