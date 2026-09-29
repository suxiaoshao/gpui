//! Notification policy and transient session notices. No second session lifecycle.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CompletionMode {
    Off,
    #[default]
    Background,
    Always,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Preferences {
    pub waiting: bool,
    pub failures: bool,
    pub completion: CompletionMode,
    pub plugins: bool,
    pub attention: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            waiting: true,
            failures: true,
            completion: CompletionMode::Background,
            plugins: false,
            attention: true,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Severity {
    Info,
    Warning,
    Error,
}
impl Severity {
    pub fn from_pi(value: Option<&str>) -> Self {
        match value {
            Some("warning") => Self::Warning,
            Some("error") => Self::Error,
            _ => Self::Info,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct NoticeContent {
    pub id: Option<String>,
    pub message: String,
    pub severity: Severity,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Waiting(String),
    Completed,
    Failed,
    Plugin,
}
impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Waiting(_) => "notification-waiting",
            Self::Completed => "notification-completed",
            Self::Failed => "notification-failed",
            Self::Plugin => "notification-plugin",
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Notice {
    pub key: String,
    pub binding: u64,
    pub kind: Kind,
    pub message: Option<NoticeContent>,
}
impl Preferences {
    pub fn system(&self, kind: &Kind, foreground: bool, source_visible: bool) -> bool {
        if source_visible {
            return false;
        }
        match kind {
            Kind::Waiting(_) => self.waiting && !foreground,
            Kind::Failed => self.failures && !foreground,
            Kind::Completed => match self.completion {
                CompletionMode::Off => false,
                CompletionMode::Background => !foreground,
                CompletionMode::Always => true,
            },
            Kind::Plugin => self.plugins && !foreground,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_route_by_app_and_source_visibility() {
        let p = Preferences::default();
        for kind in [Kind::Waiting("a".into()), Kind::Failed, Kind::Completed] {
            assert!(p.system(&kind, false, false));
            assert!(!p.system(&kind, true, false));
            assert!(!p.system(&kind, true, true));
        }
        assert!(!p.system(&Kind::Plugin, false, false));
        let p = Preferences {
            completion: CompletionMode::Always,
            plugins: true,
            ..p
        };
        assert!(p.system(&Kind::Completed, true, false));
        assert!(!p.system(&Kind::Completed, true, true));
        assert!(p.system(&Kind::Plugin, false, false));
    }
    #[test]
    fn old_config_gets_defaults_and_unknown_severity_is_info() {
        let p: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(p, Preferences::default());
        assert_eq!(Severity::from_pi(Some("warning")), Severity::Warning);
        assert_eq!(Severity::from_pi(Some("custom")), Severity::Info);
    }
}
