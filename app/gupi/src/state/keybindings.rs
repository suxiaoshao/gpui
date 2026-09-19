use crate::{
    app::menus,
    features::home::actions::{Kind, Run},
};
use gpui_kit::*;
use std::collections::BTreeMap;

pub(crate) type Overrides = BTreeMap<String, String>;
pub(crate) struct Command {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub default: &'static str,
}
pub(crate) const COMMANDS: &[Command] = &[
    Command {
        id: "temporary_trash",
        label: "temporary-trash",
        kind: Kind::TrashTemporary,
        default: "secondary-shift-backspace",
    },
    Command {
        id: "temporary_actions",
        label: "temporary-actions",
        kind: Kind::TemporaryActions,
        default: "secondary-k",
    },
    Command {
        id: "temporary_paste",
        label: "temporary-paste-answer",
        kind: Kind::PasteAnswer,
        default: "enter",
    },
    Command {
        id: "temporary_copy",
        label: "command-copy-last-answer",
        kind: Kind::CopyTemporaryAnswer,
        default: "secondary-enter",
    },
    Command {
        id: "temporary_reveal",
        label: "temporary-reveal-workspace",
        kind: Kind::RevealWorkspace,
        default: "",
    },
    Command {
        id: "temporary_hide",
        label: "temporary-hide",
        kind: Kind::HideTemporary,
        default: "",
    },
    Command {
        id: "temporary_session_1",
        label: "temporary-session-1",
        kind: Kind::TemporarySession(1),
        default: "secondary-1",
    },
    Command {
        id: "temporary_session_2",
        label: "temporary-session-2",
        kind: Kind::TemporarySession(2),
        default: "secondary-2",
    },
    Command {
        id: "temporary_session_3",
        label: "temporary-session-3",
        kind: Kind::TemporarySession(3),
        default: "secondary-3",
    },
    Command {
        id: "temporary_session_4",
        label: "temporary-session-4",
        kind: Kind::TemporarySession(4),
        default: "secondary-4",
    },
    Command {
        id: "temporary_session_5",
        label: "temporary-session-5",
        kind: Kind::TemporarySession(5),
        default: "secondary-5",
    },
    Command {
        id: "temporary_session_6",
        label: "temporary-session-6",
        kind: Kind::TemporarySession(6),
        default: "secondary-6",
    },
    Command {
        id: "temporary_session_7",
        label: "temporary-session-7",
        kind: Kind::TemporarySession(7),
        default: "secondary-7",
    },
    Command {
        id: "temporary_session_8",
        label: "temporary-session-8",
        kind: Kind::TemporarySession(8),
        default: "secondary-8",
    },
    Command {
        id: "temporary_session_9",
        label: "temporary-session-9",
        kind: Kind::TemporarySession(9),
        default: "secondary-9",
    },
    Command {
        id: "palette",
        label: "command-palette",
        kind: Kind::Palette,
        default: "secondary-shift-p",
    },
    Command {
        id: "quick_open",
        label: "conversation-search",
        kind: Kind::QuickOpen,
        default: "secondary-p",
    },
    Command {
        id: "new",
        label: "conversation-new",
        kind: Kind::New,
        default: "secondary-n",
    },
    Command {
        id: "settings",
        label: "menu-settings",
        kind: Kind::Settings,
        default: "secondary-,",
    },
    Command {
        id: "sidebar",
        label: "conversation-sidebar",
        kind: Kind::Sidebar,
        default: "secondary-b",
    },
    Command {
        id: "scan",
        label: "conversation-refresh",
        kind: Kind::Scan,
        default: "secondary-shift-r",
    },
    Command {
        id: "focus_input",
        label: "command-focus-input",
        kind: Kind::FocusInput,
        default: "secondary-l",
    },
    Command {
        id: "main",
        label: "menu-show-main",
        kind: Kind::ShowMain,
        default: "",
    },
    Command {
        id: "quit",
        label: "menu-quit",
        kind: Kind::Quit,
        default: "cmd-q",
    },
    Command {
        id: "model",
        label: "command-model",
        kind: Kind::Model,
        default: "secondary-alt-/",
    },
    Command {
        id: "history",
        label: "conversation-history",
        kind: Kind::History,
        default: "secondary-alt-b",
    },
    Command {
        id: "export",
        label: "conversation-export",
        kind: Kind::Export,
        default: "",
    },
    Command {
        id: "clone",
        label: "conversation-clone",
        kind: Kind::Clone,
        default: "",
    },
    Command {
        id: "copy_answer",
        label: "command-copy-last-answer",
        kind: Kind::CopyLastAnswer,
        default: "",
    },
    Command {
        id: "compact",
        label: "command-compact",
        kind: Kind::Compact,
        default: "",
    },
    Command {
        id: "reload",
        label: "conversation-reconnect",
        kind: Kind::Reconnect,
        default: "secondary-r",
    },
    Command {
        id: "rename",
        label: "conversation-rename",
        kind: Kind::Rename,
        default: "",
    },
    Command {
        id: "stop",
        label: "settings-key-stop-or-hide",
        kind: Kind::Stop,
        default: "escape",
    },
    Command {
        id: "close",
        label: "conversation-close-run",
        kind: Kind::Close,
        default: "",
    },
    Command {
        id: "reveal",
        label: "settings-key-reveal-session",
        kind: Kind::Reveal,
        default: "",
    },
    Command {
        id: "copy_path",
        label: "conversation-copy-path",
        kind: Kind::CopyPath,
        default: "",
    },
    Command {
        id: "delete",
        label: "conversation-delete",
        kind: Kind::Delete,
        default: "",
    },
];
impl Command {
    pub fn value<'a>(&'a self, overrides: &'a Overrides) -> &'a str {
        overrides
            .get(self.id)
            .map(String::as_str)
            .unwrap_or(self.default)
    }
    fn bindings(&self, text: &str) -> Vec<KeyBinding> {
        if text.is_empty() {
            return vec![];
        }
        if self.kind.temporary_only() {
            let plain_enter = self.kind == Kind::PasteAnswer
                && Keystroke::parse(text)
                    .is_ok_and(|key| key == Keystroke::parse("enter").unwrap());
            let context = if plain_enter {
                "GupiTemporary && !Input && !Command && !GupiExtension"
            } else {
                "GupiTemporary"
            };
            let mut bindings = vec![KeyBinding::new(text, Run(self.kind), Some(context))];
            if self.kind == Kind::PasteAnswer {
                // Inputs handle plain Enter through their submit event; lists
                // otherwise consume it as selection confirmation.
                bindings.push(KeyBinding::new(
                    text,
                    Run(self.kind),
                    Some(if plain_enter {
                        "GupiTemporary > List && !Input && !Command"
                    } else {
                        "GupiTemporary > List"
                    }),
                ));
            }
            return bindings;
        }
        match self.kind {
            Kind::Settings => vec![KeyBinding::new(text, menus::ShowSettings, None)],
            Kind::ShowMain => vec![KeyBinding::new(text, menus::ShowMainWindow, None)],
            Kind::Quit => vec![KeyBinding::new(text, menus::Quit, None)],
            Kind::Palette => vec![
                KeyBinding::new(text, menus::ShowCommandPalette, Some("GupiApplication")),
                KeyBinding::new(text, Run(self.kind), Some("Gupi")),
                KeyBinding::new(text, Run(self.kind), Some("GupiPalette")),
            ],
            Kind::Stop => vec![KeyBinding::new(text, Run(self.kind), Some("Gupi"))],
            _ => ["Gupi", "GupiPalette"]
                .map(|context| KeyBinding::new(text, Run(self.kind), Some(context)))
                .to_vec(),
        }
    }
}
/// Input widgets own ordinary Enter (sending, IME and popup precedence).
/// They only invoke the temporary primary action if Enter is still configured.
pub(crate) fn uses_enter(kind: Kind, cx: &App) -> bool {
    use gpui_kit::AsKeystroke;
    let enter = Keystroke::parse("enter").unwrap();
    cx.key_bindings().borrow().bindings().any(|binding| {
        binding
            .action()
            .as_any()
            .downcast_ref::<Run>()
            .is_some_and(|action| action.0 == kind)
            && binding.keystrokes().len() == 1
            && binding.keystrokes()[0].as_keystroke() == &enter
    })
}
fn command_for(action: &dyn Action) -> Option<&'static Command> {
    let kind = if action.as_any().is::<menus::ShowSettings>() {
        Kind::Settings
    } else if action.as_any().is::<menus::Quit>() {
        Kind::Quit
    } else if action.as_any().is::<menus::ShowMainWindow>() {
        Kind::ShowMain
    } else if action.as_any().is::<menus::ShowCommandPalette>() {
        Kind::Palette
    } else {
        action.as_any().downcast_ref::<Run>()?.0
    };
    COMMANDS.iter().find(|c| c.kind == kind)
}
pub(crate) fn syntax(text: &str) -> Result<(), &'static str> {
    if text.is_empty() {
        return Ok(());
    }
    if text.chars().any(char::is_whitespace) {
        return Err("settings-key-invalid");
    }
    Keystroke::parse(text)
        .map(|_| ())
        .map_err(|_| "settings-key-invalid")
}
pub(crate) fn validate(
    id: &str,
    text: &str,
    overrides: &Overrides,
    cx: &App,
) -> Result<(), String> {
    syntax(text).map_err(str::to_owned)?;
    let Some(command) = COMMANDS.iter().find(|c| c.id == id) else {
        return Err("settings-key-invalid".into());
    };
    if text.is_empty() {
        return Ok(());
    }
    let candidate = Keystroke::parse(text).map_err(|_| "settings-key-invalid".to_owned())?;
    for other in COMMANDS.iter().filter(|c| c.id != id) {
        for binding in other.bindings(other.value(overrides)) {
            if starts_with(&binding, &candidate) {
                return Err(other.label.into());
            }
        }
    }
    // Evaluate component bindings only in focus stacks that coexist with application actions.
    let global = matches!(command.kind, Kind::Settings | Kind::ShowMain | Kind::Quit);
    let keymap = cx.key_bindings();
    for binding in keymap
        .borrow()
        .bindings()
        .filter(|b| command_for(b.action()).is_none())
    {
        if !starts_with(binding, &candidate) {
            continue;
        }
        let applicable = ["Input", "Textarea", "GupiExtension", "Command", "List"]
            .iter()
            .any(|leaf| {
                let stack: Vec<_> = ["GupiApplication", "Gupi GupiTemporary", leaf]
                    .into_iter()
                    .map(|s| KeyContext::parse(s).unwrap())
                    .collect();
                binding
                    .predicate()
                    .is_none_or(|p| (0..stack.len()).any(|i| p.eval(&stack[..=i])))
            });
        if global || applicable {
            // Existing default Esc intentionally coexists with editors and dialogs.
            if text != command.default {
                return Err(binding.action().name().into());
            }
        }
    }
    Ok(())
}
/// System shortcuts apply in every focus context, including the first stroke
/// of a component chord. Application command conflicts are checked by AppConfig.
pub(crate) fn validate_global(text: &str, cx: &App) -> Result<(), String> {
    if text.is_empty() {
        return Ok(());
    }
    let candidate = Keystroke::parse(text).map_err(|_| "settings-key-invalid".to_owned())?;
    if cx
        .key_bindings()
        .borrow()
        .bindings()
        .any(|binding| command_for(binding.action()).is_none() && starts_with(binding, &candidate))
    {
        return Err("settings-key-conflict".into());
    }
    Ok(())
}
// Component-owned bindings may contain chords; their first stroke must also
// stay available when assigning a single application or system shortcut.
fn starts_with(binding: &KeyBinding, key: &Keystroke) -> bool {
    binding
        .keystrokes()
        .first()
        .is_some_and(|first| first.as_keystroke() == key)
}
#[derive(Default)]
struct Applied(Overrides);
impl Global for Applied {}
pub(crate) fn apply(overrides: &Overrides, cx: &mut App) {
    if cx
        .try_global::<Applied>()
        .is_some_and(|applied| applied.0 == *overrides)
    {
        return;
    }
    // Start from the live keymap, preserving unrelated bindings and their registration order.
    let old: Vec<_> = cx.key_bindings().borrow().bindings().cloned().collect();
    let mut bindings = Vec::new();
    let mut replaced = std::collections::BTreeSet::new();
    for binding in old {
        if let Some(command) = command_for(binding.action()) {
            if replaced.insert(command.id) {
                bindings.extend(command.bindings(command.value(overrides)));
            }
        } else {
            bindings.push(binding);
        }
    }
    for command in COMMANDS {
        if replaced.insert(command.id) {
            bindings.extend(command.bindings(command.value(overrides)));
        }
    }
    cx.clear_key_bindings();
    cx.bind_keys(bindings);
    cx.set_global(Applied(overrides.clone()));
    menus::refresh(cx);
}

#[cfg(test)]
mod tests {
    use super::{Kind, Overrides, Run, apply, command_for, validate};
    use gpui_kit::{AsKeystroke, Keystroke, TestAppContext};
    #[gpui_kit::test]
    fn temporary_trash_rebinding_removes_the_old_key_and_stays_in_its_window(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(Default::default(), cx);
            let overrides = Overrides::from([("temporary_trash".into(), "ctrl-alt-d".into())]);
            assert!(validate("temporary_trash", "ctrl-alt-d", &overrides, cx).is_ok());
            apply(&overrides, cx);
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let bindings = keymap
                .bindings()
                .filter(|binding| {
                    command_for(binding.action()).is_some_and(|c| c.id == "temporary_trash")
                })
                .collect::<Vec<_>>();
            assert_eq!(bindings.len(), 1);
            assert_eq!(
                bindings[0].keystrokes()[0].as_keystroke(),
                &Keystroke::parse("ctrl-alt-d").unwrap()
            );
            let temp = gpui_kit::KeyContext::parse("Gupi GupiTemporary").unwrap();
            let main = gpui_kit::KeyContext::parse("Gupi").unwrap();
            assert!(bindings[0].predicate().unwrap().eval(&[temp]));
            assert!(!bindings[0].predicate().unwrap().eval(&[main]));
            drop(keymap);
            apply(
                &Overrides::from([("temporary_trash".into(), String::new())]),
                cx,
            );
            assert!(!cx.key_bindings().borrow().bindings().any(|binding| {
                command_for(binding.action()).is_some_and(|c| c.id == "temporary_trash")
            }));
        });
    }
    #[gpui_kit::test]
    fn replacing_and_restoring_keeps_other_actions_and_component_bindings(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::foundation::i18n::apply(Default::default(), cx);
            apply(&Overrides::new(), cx);
            let unrelated: Vec<_> = cx
                .key_bindings()
                .borrow()
                .bindings()
                .filter(|b| command_for(b.action()).is_none())
                .map(|b| format!("{b:?}"))
                .collect();
            let mut overrides = Overrides::new();
            overrides.insert("new".into(), "secondary-shift-n".into());
            apply(&overrides, cx);
            let keys = cx.key_bindings();
            let keys = keys.borrow();
            let new: Vec<_> = keys
                .bindings()
                .filter(|b| {
                    b.action()
                        .as_any()
                        .downcast_ref::<Run>()
                        .is_some_and(|a| a.0 == Kind::New)
                })
                .collect();
            assert_eq!(new.len(), 2);
            assert!(new.iter().all(|b| b.keystrokes()[0].as_keystroke()
                == &Keystroke::parse("secondary-shift-n").unwrap()));
            assert_eq!(
                unrelated,
                keys.bindings()
                    .filter(|b| command_for(b.action()).is_none())
                    .map(|b| format!("{b:?}"))
                    .collect::<Vec<_>>()
            );
            drop(keys);
            assert!(validate("new", "secondary-p", &overrides, cx).is_err());
            assert!(validate("new", "secondary-j", &overrides, cx).is_ok());
            assert!(validate("new", "secondary-k", &overrides, cx).is_err());
            apply(&Overrides::new(), cx);
            assert!(
                cx.key_bindings()
                    .borrow()
                    .bindings()
                    .filter(|b| command_for(b.action()).is_some_and(|c| c.id == "new"))
                    .all(|b| b.keystrokes()[0].as_keystroke()
                        == &Keystroke::parse("secondary-n").unwrap())
            );
        });
    }
}
