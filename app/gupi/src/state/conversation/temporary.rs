use super::*;

impl ConversationState {
    pub fn temporary(command: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut state = Self::new(command, cx);
        state.temporary = true;
        state
    }

    pub fn temporary_workspace(&self, key: &str) -> Option<PathBuf> {
        self.temporary
            .then(|| self.sessions.get(key).map(|s| s.info.cwd.clone()))
            .flatten()
    }

    pub(super) fn can_delete_temporary(&self, key: &str) -> bool {
        !self.draining
            && self.sessions.get(key).is_some_and(|s| {
                !s.settings_busy()
                    && !s.core_read.running()
                    && (s.instance.is_none()
                        || s.state.is_some()
                        || s.error.is_some()
                        || s.core_read.error().is_some())
            })
    }

    pub(super) fn delete_temporary(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.can_delete_temporary(key) {
            return;
        }
        let s = self.sessions.get_mut(key).unwrap();
        let directory = s.info.cwd.clone();
        s.binding += 1;
        s.reset_reads();
        s.state = None;
        s.clear_extension_ui();
        let closing = s
            .instance
            .take()
            .map(|id| pi::global(cx).update(cx, |pi, cx| pi.close(id, cx)));
        let key = key.to_owned();
        let target = key.clone();
        let task = cx.spawn(async move |owner, cx| {
            let result = async {
                if let Some(close) = closing
                    && close.await.is_some_and(|report| report.status.is_none())
                {
                    return Err("Pi process exit could not be confirmed".to_owned());
                }
                smol::unblock(move || trash_workspace(&directory)).await
            }
            .await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(s) = this.sessions.get_mut(&key) {
                    s.command.finish();
                }
                match result {
                    Ok(()) => {
                        this.sessions.remove(&key);
                        if this.selected.as_ref() == Some(&key) {
                            this.new_draft(None, cx);
                        }
                        notify_session(&key, cx);
                    }
                    Err(error) => cx.emit(ConversationEvent::Notify {
                        message: error,
                        error: true,
                    }),
                }
                notify_session(&key, cx);
            });
        });
        self.sessions.get_mut(&target).unwrap().command = SessionCommand::Deleting { task };
        notify_session(&target, cx);
    }
}

/// Only application-created directory entries are eligible. Trash the directory
/// itself; never walk links into user files referenced by a conversation.
fn validate_workspace(root: &std::path::Path, path: &std::path::Path) -> Result<bool, String> {
    if path.parent() != Some(root)
        || !path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("draft-"))
    {
        return Err("Not a Gupi temporary workspace".into());
    }
    if std::fs::symlink_metadata(root).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Temporary root must not be a symlink".into());
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Temporary workspace is no longer a directory".into());
    }
    Ok(true)
}

pub(crate) fn trash_workspace(path: &std::path::Path) -> Result<(), String> {
    let root = paths::temporary_dir().map_err(|e| e.to_string())?;
    if !validate_workspace(&root, path)? {
        return Ok(());
    }
    let context = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    let context = {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut context = context;
        context.set_delete_method(DeleteMethod::NsFileManager);
        context
    };
    context.delete(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::validate_workspace;
    #[test]
    fn cleanup_accepts_only_owned_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspaces");
        std::fs::create_dir(&root).unwrap();
        let owned = root.join("draft-test");
        std::fs::create_dir(&owned).unwrap();
        assert_eq!(validate_workspace(&root, &owned), Ok(true));
        assert!(validate_workspace(&root, dir.path()).is_err());
        assert!(validate_workspace(&root, &root.join("user-file")).is_err());
        let file = root.join("draft-file");
        std::fs::write(&file, "original").unwrap();
        assert!(validate_workspace(&root, &file).is_err());
        #[cfg(unix)]
        {
            let linked = root.join("draft-link");
            std::os::unix::fs::symlink(dir.path(), &linked).unwrap();
            assert!(validate_workspace(&root, &linked).is_err());
            let alias = dir.path().join("alias");
            std::os::unix::fs::symlink(&root, &alias).unwrap();
            assert!(validate_workspace(&alias, &alias.join("draft-test")).is_err());
        }
        assert_eq!(std::fs::read_to_string(file).unwrap(), "original");
    }
}
