use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Shortcuts {
    pub launcher: String,
    pub tasks: Vec<ShortcutTask>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ShortcutTask {
    pub id: String,
    pub name: String,
    pub binding: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
    pub template: PathBuf,
    pub source: InputSource,
    pub model: Option<ModelChoice>,
    pub thinking: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelChoice {
    pub provider: String,
    pub id: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputSource {
    Selection,
    Clipboard,
    #[default]
    SelectionOrClipboard,
}
fn enabled() -> bool {
    true
}
impl Shortcuts {
    pub fn clear_bindings(&mut self) {
        self.launcher.clear();
        for task in &mut self.tasks {
            task.binding.clear();
        }
    }
    pub fn has_bindings(&self) -> bool {
        !self.launcher.is_empty() || self.tasks.iter().any(|t| !t.binding.is_empty())
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        for task in &self.tasks {
            if task.id.is_empty()
                || task.name.trim().is_empty()
                || task.template.as_os_str().is_empty()
                || !ids.insert(&task.id)
            {
                return Err("error-config-validation".into());
            }
        }
        for binding in std::iter::once(&self.launcher)
            .chain(self.tasks.iter().map(|t| &t.binding))
            .filter(|b| !b.is_empty())
        {
            let normalized = system_binding(binding)?;
            if !bindings.insert(normalized) {
                return Err("settings-key-conflict".into());
            }
        }
        Ok(())
    }
}
/// Keep the same GPUI syntax as the existing inline shortcut editor.
pub(crate) fn system_binding(binding: &str) -> Result<String, String> {
    let key = gpui_kit::Keystroke::parse(binding).map_err(|_| "settings-key-invalid".to_owned())?;
    if key.modifiers.function
        || !(key.modifiers.control || key.modifiers.platform || key.modifiers.alt)
    {
        return Err("settings-key-invalid".into());
    }
    let mut parts = Vec::new();
    if key.modifiers.control {
        parts.push("Control".to_owned());
    }
    if key.modifiers.platform {
        parts.push("Super".to_owned());
    }
    if key.modifiers.alt {
        parts.push("Alt".to_owned());
    }
    if key.modifiers.shift {
        parts.push("Shift".to_owned());
    }
    parts.sort();
    parts.dedup();
    parts.push(
        match key.key.as_str() {
            "-" => "Minus",
            "=" => "Equal",
            "/" => "Slash",
            "," => "Comma",
            "." => "Period",
            ";" => "Semicolon",
            "'" => "Quote",
            "[" => "BracketLeft",
            "]" => "BracketRight",
            "\\" => "Backslash",
            "`" => "Backquote",
            "enter" => "Enter",
            "escape" => "Escape",
            "space" => "Space",
            "backspace" => "Backspace",
            "up" => "ArrowUp",
            "down" => "ArrowDown",
            "left" => "ArrowLeft",
            "right" => "ArrowRight",
            other => other,
        }
        .to_owned(),
    );
    let value = parts.join("+");
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        value
            .parse::<global_hotkey::hotkey::HotKey>()
            .map(|key| key.to_string())
            .map_err(|_| "settings-key-invalid".to_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Ok(value)
}

#[derive(Clone, Debug)]
pub(crate) struct PendingTemplate {
    pub name: String,
    pub body: String,
}

/// Pi command argument quoting has no backslash escape syntax.
pub(crate) fn argument(value: &str) -> String {
    value
        .split('"')
        .map(|part| format!("\"{part}\""))
        .collect::<Vec<_>>()
        .join("'\"'")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_pi_argument_keeps_quotes_newlines_and_backslashes() {
        assert_eq!(argument("a b\n中\\文'\"$1"), "\"a b\n中\\文'\"'\"'\"$1\"");
        assert_eq!(argument(""), "\"\"");
    }
    #[test]
    fn reset_only_removes_bindings_and_duplicate_ids_are_invalid() {
        let task = ShortcutTask {
            id: "a".into(),
            name: "Translate".into(),
            binding: "ctrl-alt-t".into(),
            enabled: true,
            template: "/tmp/translate.md".into(),
            ..Default::default()
        };
        let mut config = Shortcuts {
            launcher: "ctrl-alt-space".into(),
            tasks: vec![task.clone()],
        };
        assert!(config.validate().is_ok());
        config.clear_bindings();
        assert!(!config.has_bindings());
        assert_eq!(config.tasks[0].name, task.name);
        assert!(config.tasks[0].enabled);
        config.tasks.push(task);
        assert!(config.validate().is_err());
    }
    #[test]
    fn system_shortcut_requires_a_modifier_and_has_one_canonical_form() {
        assert!(system_binding("x").is_err());
        assert!(system_binding("ctrl-x ctrl-y").is_err());
        assert_eq!(
            system_binding("ctrl-alt-x").unwrap(),
            system_binding("alt-ctrl-x").unwrap()
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            system_binding("secondary-x").unwrap(),
            system_binding("cmd-x").unwrap()
        );
    }
}

pub(crate) fn template_message(name: &str, body: &str, text: &str) -> String {
    let normalized = body.replace("\r\n", "\n");
    let body = normalized.as_str();
    let body = body
        .strip_prefix("---\n")
        .and_then(|s| {
            s.split_once("\n---")
                .map(|(_, body)| body.trim_start_matches(['\r', '\n']))
        })
        .unwrap_or(body);
    let parameterized = body.split('$').skip(1).any(|part| {
        if part.starts_with('@')
            || part.starts_with("ARGUMENTS")
            || part.starts_with(|c: char| c.is_ascii_digit())
        {
            return true;
        }
        let Some(inner) = part
            .strip_prefix('{')
            .and_then(|s| s.split_once('}').map(|(s, _)| s))
        else {
            return false;
        };
        if let Some((target, _)) = inner.split_once(":-") {
            return target == "@"
                || target == "ARGUMENTS"
                || !target.is_empty() && target.chars().all(|c| c.is_ascii_digit());
        }
        inner.strip_prefix("@:").is_some_and(|s| {
            let parts = s.split(':').collect::<Vec<_>>();
            (1..=2).contains(&parts.len())
                && parts
                    .iter()
                    .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        })
    });
    if parameterized {
        format!("/{name} {}", argument(text))
    } else {
        format!("\n{body}\n{text}")
    }
}

#[cfg(test)]
mod template_tests {
    use super::template_message;
    #[test]
    fn templates_preserve_input_and_use_pi_for_parameters() {
        assert_eq!(
            template_message(
                "test",
                "---\r\ndescription: test\r\n---\r\nTranslate",
                "a\nb"
            ),
            "\nTranslate\na\nb"
        );
        for body in ["do $1", "do $@", "do ${ARGUMENTS:-hello}", "do ${@:1:2}"] {
            assert_eq!(
                template_message("test", body, "a \"b\""),
                "/test \"a \"'\"'\"b\"'\"'\"\""
            );
        }
        assert_eq!(
            template_message("test", "/do $money ${name}", "input"),
            "\n/do $money ${name}\ninput"
        );
    }
}
