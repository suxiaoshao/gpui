//! Command search paths shared by every Pi-related subprocess.
use gpui_kit::{App, Global};
use gpui_tokio::Tokio;
use std::{ffi::OsString, fmt::Display};
#[cfg(any(target_os = "macos", test))]
use std::{future::Future, sync::Arc};
#[cfg(any(target_os = "macos", test))]
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct Snapshot {
    path: Option<OsString>,
    warning: Option<String>,
}
impl Snapshot {
    pub fn variables(&self) -> Vec<(OsString, OsString)> {
        self.path
            .iter()
            .map(|path| ("PATH".into(), path.clone()))
            .collect()
    }
    pub fn explain(&self, error: impl Display) -> String {
        match &self.warning {
            Some(warning) => format!("{error}\nShell PATH: {warning}"),
            None => error.to_string(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Environment {
    inherited: Option<OsString>,
    capture: bool,
    #[cfg(any(target_os = "macos", test))]
    loaded: Arc<Mutex<Option<Snapshot>>>,
}
impl Global for Environment {}

pub(crate) fn init(cx: &mut App) {
    let environment = Environment::new(should_capture());
    cx.set_global(environment.clone());
    Tokio::spawn(cx, async move { environment.load(false).await }).detach();
}

pub(crate) fn current(cx: &App) -> Environment {
    cx.try_global::<Environment>()
        .cloned()
        .unwrap_or_else(|| Environment::new(false))
}

fn should_capture() -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::io::IsTerminal;
        // Terminal launches already have the user's deliberately selected environment.
        !(std::io::stdin().is_terminal()
            || std::io::stdout().is_terminal()
            || std::io::stderr().is_terminal())
    }
    #[cfg(not(target_os = "macos"))]
    false
}

impl Environment {
    fn new(capture: bool) -> Self {
        Self {
            inherited: std::env::var_os("PATH"),
            capture,
            #[cfg(any(target_os = "macos", test))]
            loaded: Arc::new(Mutex::new(None)),
        }
    }
    pub async fn load(&self, refresh: bool) -> Snapshot {
        #[cfg(target_os = "macos")]
        if self.capture {
            return self
                .load_with(refresh, crate::foundation::shell_path::capture())
                .await;
        }
        let _ = (refresh, self.capture);
        Snapshot {
            path: self.inherited.clone(),
            warning: None,
        }
    }
    #[cfg(any(target_os = "macos", test))]
    async fn load_with(
        &self,
        refresh: bool,
        capture: impl Future<Output = Result<OsString, String>>,
    ) -> Snapshot {
        let mut loaded = self.loaded.lock().await;
        if !refresh && let Some(snapshot) = &*loaded {
            return snapshot.clone();
        }
        let snapshot = match capture.await {
            Ok(path) => Snapshot {
                path: Some(path),
                warning: None,
            },
            Err(warning) => {
                // Keep absolute/system-installed commands usable when shell setup fails.
                tracing::warn!("Could not load login shell PATH; using inherited command paths");
                Snapshot {
                    path: self.inherited.clone(),
                    warning: Some(warning),
                }
            }
        };
        *loaded = Some(snapshot.clone());
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn cached_failure_uses_inherited_paths_until_explicit_retry() {
        let environment = Environment {
            inherited: Some("/original/bin".into()),
            capture: true,
            loaded: Arc::new(Mutex::new(None)),
        };
        let failed = environment
            .load_with(false, async { Err("shell timed out".into()) })
            .await;
        assert_eq!(
            failed.variables(),
            [("PATH".into(), "/original/bin".into())]
        );
        assert!(failed.explain("pi unavailable").contains("shell timed out"));
        let cached = environment
            .clone()
            .load_with(false, async { panic!("must share cached result") })
            .await;
        assert_eq!(cached.path, failed.path);
        let retried = environment
            .load_with(true, async { Ok("/node/bin:/pi/bin".into()) })
            .await;
        assert_eq!(retried.path, Some("/node/bin:/pi/bin".into()));
        assert!(retried.warning.is_none());
        assert_eq!(
            environment
                .load_with(false, async { panic!("must reuse successful result") })
                .await
                .path,
            retried.path
        );
    }
}
