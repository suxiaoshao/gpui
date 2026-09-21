//! System registration and template preparations belong to the application.
use crate::state::{config::AppConfig, shortcuts::Shortcuts};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::{
    foundation::{
        attachments::{self, Attachment, Content},
        i18n::t,
    },
    state::shortcuts::{InputSource, ShortcutTask, system_binding},
};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui_kit::*;
use std::collections::BTreeMap;

pub(crate) struct ShortcutsRuntime {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    manager: Result<GlobalHotKeyManager, String>,
    config: Shortcuts,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    actions: BTreeMap<u32, String>,
    jobs: BTreeMap<String, Task<()>>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    active: BTreeMap<String, String>,
    pub error: Option<String>,
    paused: bool,
    draining: bool,
    _events: Option<Task<()>>,
}
impl Global for ShortcutsRuntime {}
pub fn init(cx: &mut App) {
    let runtime = ShortcutsRuntime {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        manager: GlobalHotKeyManager::new().map_err(|e| e.to_string()),
        config: Shortcuts::default(),
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        actions: BTreeMap::new(),
        jobs: BTreeMap::new(),
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        active: BTreeMap::new(),
        error: None,
        paused: false,
        draining: false,
        _events: None,
    };
    cx.set_global(runtime);
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let (tx, rx) = smol::channel::unbounded();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state() == HotKeyState::Pressed {
                let _ = tx.try_send(event.id());
            }
        }));
        let task = cx.spawn(async move |cx| {
            while let Ok(id) = rx.recv().await {
                cx.update(|cx| {
                    let rt = cx.global::<ShortcutsRuntime>();
                    if rt.paused || rt.draining {
                        return;
                    }
                    let Some(action) = rt.actions.get(&id).cloned() else {
                        return;
                    };
                    if action.is_empty() {
                        super::temporary::toggle(cx);
                    } else {
                        trigger(&action, cx);
                    }
                });
            }
        });
        cx.global_mut::<ShortcutsRuntime>()._events = Some(task);
    }
}
impl ShortcutsRuntime {
    fn replace(&mut self, config: &Shortcuts) -> Result<(), String> {
        config.validate()?;
        if self.config == *config {
            return Ok(());
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let parse = |c: &Shortcuts| -> Result<BTreeMap<u32, (HotKey, String)>, String> {
                std::iter::once(("", c.launcher.as_str()))
                    .chain(
                        c.tasks
                            .iter()
                            .filter(|t| t.enabled)
                            .map(|t| (t.id.as_str(), t.binding.as_str())),
                    )
                    .filter(|(_, b)| !b.is_empty())
                    .map(|(id, b)| {
                        let key: HotKey = system_binding(b)?
                            .parse()
                            .map_err(|e: global_hotkey::hotkey::HotKeyParseError| e.to_string())?;
                        Ok((key.id(), (key, id.to_owned())))
                    })
                    .collect()
            };
            let old = parse(&self.config)?;
            let new = parse(config)?;
            let manager = self.manager.as_ref().map_err(Clone::clone)?;
            let mut added = Vec::new();
            for (id, (key, _)) in &new {
                if old.contains_key(id) {
                    continue;
                }
                if let Err(error) = manager.register(*key) {
                    for key in added {
                        let _ = manager.unregister(key);
                    }
                    return Err(error.to_string());
                }
                added.push(*key);
            }
            let mut removed = Vec::new();
            for (id, (key, _)) in &old {
                if new.contains_key(id) {
                    continue;
                }
                if let Err(error) = manager.unregister(*key) {
                    for key in removed {
                        let _ = manager.register(key);
                    }
                    for key in added {
                        let _ = manager.unregister(key);
                    }
                    return Err(error.to_string());
                }
                removed.push(*key);
            }
            self.actions = new
                .into_iter()
                .map(|(id, (_, action))| (id, action))
                .collect();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        if config.has_bindings() {
            return Err("System shortcuts are available on macOS and Windows".into());
        }
        self.config = config.clone();
        Ok(())
    }
}
fn validate_registration(value: &AppConfig, cx: &App) -> Result<(), String> {
    value.clone().normalized()?;
    for binding in std::iter::once(value.shortcuts.launcher.as_str()).chain(
        value
            .shortcuts
            .tasks
            .iter()
            .filter(|task| task.enabled)
            .map(|task| task.binding.as_str()),
    ) {
        crate::state::keybindings::validate_global(binding, cx)?;
    }
    Ok(())
}
/// Pause triggers while the configuration transaction is in flight.
pub fn prepare(value: &AppConfig, cx: &mut App) -> Result<(), String> {
    validate_registration(value, cx)?;
    if !cx.has_global::<ShortcutsRuntime>() {
        return Ok(());
    }
    cx.update_global::<ShortcutsRuntime, _>(|rt, _| {
        rt.replace(&value.shortcuts)?;
        rt.paused = true;
        rt.error = None;
        Ok(())
    })
}
pub fn apply(value: &AppConfig, cx: &mut App) {
    if !cx.has_global::<ShortcutsRuntime>() {
        return;
    }
    let validation = validate_registration(value, cx);
    cx.update_global::<ShortcutsRuntime, _>(|rt, _| {
        if rt.draining {
            return;
        }
        rt.error = validation.and_then(|()| rt.replace(&value.shortcuts)).err();
        rt.paused = false;
    });
}
pub fn shutdown(cx: &mut App) {
    if !cx.has_global::<ShortcutsRuntime>() {
        return;
    }
    cx.update_global::<ShortcutsRuntime, _>(|rt, _| {
        rt.draining = true;
        rt.jobs.clear();
        let _ = rt.replace(&Shortcuts::default());
        rt._events = None;
    });
}

pub fn cancel_preparation(_key: &str, cx: &mut App) {
    if !cx.has_global::<ShortcutsRuntime>() {
        return;
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    cx.update_global::<ShortcutsRuntime, _>(|rt, _| {
        let id = rt
            .active
            .iter()
            .find(|(_, key)| key.as_str() == _key)
            .map(|(id, _)| id.clone());
        if let Some(id) = id {
            rt.jobs.remove(&id);
            rt.active.remove(&id);
        }
    });
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn trigger(id: &str, cx: &mut App) {
    let Some(state) = super::temporary::state(cx) else {
        super::temporary::remember_frontmost(cx);
        super::temporary::show(cx);
        return;
    };
    let rt = cx.global::<ShortcutsRuntime>();
    if rt.jobs.contains_key(id) {
        if let Some(key) = rt.active.get(id).cloned() {
            state.update(cx, |s, cx| s.open(&key, cx));
            super::temporary::show(cx);
        }
        return;
    }
    if let Some(key) =
        rt.active.get(id).filter(|key| {
            state.read(cx).sessions.get(*key).is_some_and(|s| {
                s.busy() || !s.pending_ui.is_empty() || s.pending_template.is_some()
            })
        })
    {
        let key = key.clone();
        state.update(cx, |s, cx| s.open(&key, cx));
        super::temporary::show(cx);
        return;
    }
    let Some(definition) = rt
        .config
        .tasks
        .iter()
        .find(|task| task.id == id && task.enabled)
        .cloned()
    else {
        return;
    };
    cx.global_mut::<ShortcutsRuntime>().active.remove(id);
    let clipboard = cx.read_from_clipboard();
    // Selection capture must finish before activating our window.
    super::temporary::remember_frontmost(cx);
    let id = id.to_owned();
    let job_id = id.clone();
    let task = cx.spawn(async move |cx| {
        let result = run(definition, clipboard, state.clone(), cx).await;
        cx.update(|cx| {
            if let Err(error) = result {
                super::temporary::show(cx);
                state.update(cx, |_, cx| {
                    cx.emit(crate::state::conversation::ConversationEvent::Notify {
                        message: error.clone(),
                        error: true,
                    });
                    cx.notify();
                });
            }
            cx.global_mut::<ShortcutsRuntime>().jobs.remove(&id);
        });
    });
    cx.global_mut::<ShortcutsRuntime>()
        .jobs
        .insert(job_id, task);
}
#[cfg(any(target_os = "macos", target_os = "windows"))]
async fn run(
    definition: ShortcutTask,
    clipboard: Option<ClipboardItem>,
    state: Entity<crate::state::conversation::ConversationState>,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let mut text = String::new();
    let mut attachments = Vec::new();
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if definition.source != InputSource::Clipboard {
        text = smol::unblock(|| get_selected_text::get_selected_text().unwrap_or_default()).await;
    }
    if text.trim().is_empty()
        && definition.source != InputSource::Selection
        && let Some(item) = clipboard
    {
        if let Some(paths) = item.entries().iter().find_map(|e| {
            if let ClipboardEntry::ExternalPaths(p) = e {
                Some(p.paths().to_vec())
            } else {
                None
            }
        }) {
            attachments = smol::unblock(move || attachments::from_paths(paths)).await?;
        } else if let Some(bytes) = item.entries().iter().find_map(|e| {
            if let ClipboardEntry::Image(i) = e {
                Some(i.bytes().to_vec())
            } else {
                None
            }
        }) {
            let name = cx.update(|cx| t(cx, "attachment-clipboard"));
            attachments.push(smol::unblock(move || Attachment::from_image(name, &bytes)).await?);
        } else {
            text = item.text().unwrap_or_default();
        }
    }
    for a in &attachments {
        if let Content::File { path, .. } = &a.content {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push('@');
            text.push_str(&path.to_string_lossy());
        }
    }
    attachments.retain(|a| matches!(a.content, Content::Image { .. }));
    let has_input = !text.trim().is_empty() || !attachments.is_empty();
    let template = definition.template.clone();
    let body =
        smol::unblock(move || std::fs::read_to_string(template).map_err(|e| e.to_string())).await?;
    let key = state.update(cx, |s, cx| {
        let previous = s.selected.clone();
        s.new_draft(None, cx);
        if s.selected == previous {
            return Err(s
                .storage_error
                .clone()
                .unwrap_or_else(|| "Could not create temporary conversation".into()));
        }
        let key = s
            .selected
            .clone()
            .ok_or("Could not create temporary conversation")?;
        s.sessions.get_mut(&key).unwrap().preparing = true;
        Ok::<_, String>(key)
    })?;
    cx.update(|cx| {
        cx.global_mut::<ShortcutsRuntime>()
            .active
            .insert(definition.id.clone(), key.clone());
        super::temporary::show(cx);
    });
    let result = async {
        let started = std::time::Instant::now();
        let client = loop {
            let ready = state.read_with(cx, |s, cx| {
                let session = s
                    .sessions
                    .get(&key)
                    .ok_or("Temporary conversation was removed".to_owned())?;
                if let Some(error) = &session.error {
                    return Err(error.clone());
                }
                Ok(session.state.as_ref().and_then(|_| s.client(&key, cx)))
            })?;
            if let Some(client) = ready {
                break client;
            }
            if started.elapsed().as_secs() > 30 {
                return Err("Pi connection timed out".to_owned());
            }
            smol::Timer::after(std::time::Duration::from_millis(30)).await;
        };
        let commands = client.get_commands().await.map_err(|e| e.to_string())?;
        let command = commands
            .commands
            .iter()
            .find(|c| {
                c.source == "prompt"
                    && c.source_info
                        .get("path")
                        .and_then(|v| v.as_str())
                        .is_some_and(|p| std::path::Path::new(p) == definition.template)
            })
            .ok_or("Prompt template is unavailable in this Pi session")?;
        if commands
            .commands
            .iter()
            .filter(|c| c.name == command.name)
            .count()
            != 1
        {
            return Err("Prompt command name is ambiguous".into());
        }
        if let Some(model) = &definition.model {
            client
                .set_model(model.provider.clone(), model.id.clone())
                .await
                .map_err(|e| e.to_string())?;
        }
        if let Some(level) = &definition.thinking {
            if !client
                .get_available_thinking_levels()
                .await
                .map_err(|e| e.to_string())?
                .levels
                .contains(level)
            {
                return Err("Thinking level is not available for this model".into());
            }
            client
                .set_thinking_level(level.clone())
                .await
                .map_err(|e| e.to_string())?;
        }
        let snapshot = client.get_state().await.map_err(|e| e.to_string())?;
        state.update(cx, |s, cx| {
            let session = s.sessions.get_mut(&key).unwrap();
            session.state = Some(snapshot);
            session.preparing = false;
            session.attachments = attachments;
            session.pending_template = Some(crate::state::shortcuts::PendingTemplate {
                name: command.name.clone(),
                body,
            });
            session.info.name = Some(definition.name.clone());
            s.set_draft(&key, text, cx);
        });
        if !has_input {
            return Ok(());
        }
        let accepted = state
            .update(cx, |s, cx| s.send_draft(&key, cx))
            .ok_or("Temporary message could not be submitted")?;
        if !accepted.await.unwrap_or(false) {
            return Err("Pi did not accept the temporary message".into());
        }
        Ok(())
    }
    .await;
    state.update(cx, |s, cx| {
        if let Some(session) = s.sessions.get_mut(&key) {
            session.preparing = false;
            if let Err(error) = &result {
                session.error = Some(error.clone());
            }
        }
        cx.notify();
    });
    result
}

#[cfg(test)]
mod validation_tests {
    use super::{ShortcutsRuntime, apply, prepare};
    use crate::state::{config::AppConfig, shortcuts::ShortcutTask};
    use gpui_kit::{KeyBinding, TestAppContext, component::input::Copy};

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[gpui_kit::test]
    fn template_trigger_starts_without_a_version_probe_and_deduplicates(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::state::pi::init(cx);
            crate::app::temporary::init(cx);
            // The configured command is available before any version probe.
            crate::app::temporary::set_command("pi".into(), cx);
            let mut config = crate::state::shortcuts::Shortcuts::default();
            config.tasks.push(ShortcutTask {
                id: "translate".into(),
                name: "Translate".into(),
                enabled: true,
                template: "/test.md".into(),
                source: crate::state::shortcuts::InputSource::Clipboard,
                ..Default::default()
            });
            cx.set_global(ShortcutsRuntime {
                manager: Err("no OS registration in test".into()),
                config,
                actions: Default::default(),
                jobs: Default::default(),
                active: Default::default(),
                error: None,
                paused: false,
                draining: false,
                _events: None,
            });
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("original input".into()));
            super::trigger("translate", cx);
            assert!(
                cx.global::<crate::app::temporary::Temporary>()
                    .state
                    .is_some()
            );
            assert!(
                cx.global::<ShortcutsRuntime>()
                    .jobs
                    .contains_key("translate")
            );
            super::trigger("translate", cx);
            assert_eq!(cx.global::<ShortcutsRuntime>().jobs.len(), 1);
            // Cancel before the fixture would read files or launch Pi.
            cx.global_mut::<ShortcutsRuntime>().jobs.clear();
        });
    }

    #[test]
    fn configured_shortcuts_accept_single_keys_and_reject_sequences() {
        let mut config = AppConfig::default();
        config.shortcuts.launcher = "ctrl-alt-x".into();
        for invalid in [
            "ctrl-alt-x y",
            "ctrl-alt-x\ty",
            "ctrl-alt-x\ny",
            " ctrl-alt-x",
        ] {
            config.keybindings.insert("new".into(), invalid.into());
            assert_eq!(
                config.clone().normalized().unwrap_err(),
                "settings-key-invalid"
            );
        }
        config.keybindings.insert("new".into(), "ctrl-alt-y".into());
        assert!(config.clone().normalized().is_ok());
        config.keybindings.insert("new".into(), "ctrl-alt-x".into());
        assert_eq!(
            config.clone().normalized().unwrap_err(),
            "settings-key-conflict"
        );
        config.keybindings.insert("new".into(), String::new());
        assert!(config.clone().normalized().is_ok());
        config.shortcuts.launcher = "ctrl-alt-x y".into();
        assert_eq!(config.normalized().unwrap_err(), "settings-key-invalid");
    }

    #[gpui_kit::test]
    fn saving_global_bindings_rejects_component_shortcuts(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            for key in ["secondary-c", "secondary-x", "secondary-v"] {
                let mut config = AppConfig::default();
                config.shortcuts.launcher = key.into();
                assert_eq!(prepare(&config, cx).unwrap_err(), "settings-key-conflict");
                config.shortcuts.launcher.clear();
                config.shortcuts.tasks.push(ShortcutTask {
                    id: "test".into(),
                    name: "Test".into(),
                    binding: key.into(),
                    enabled: true,
                    template: "/test.md".into(),
                    ..Default::default()
                });
                assert_eq!(prepare(&config, cx).unwrap_err(), "settings-key-conflict");
                config.shortcuts.tasks[0].enabled = false;
                assert!(prepare(&config, cx).is_ok());
            }
            let mut config = AppConfig::default();
            config.shortcuts.launcher = "ctrl-alt-space".into();
            assert!(prepare(&config, cx).is_ok());
            // A system registration would also swallow the start of this chord,
            // even though its component context is unrelated to the current view.
            cx.bind_keys([KeyBinding::new(
                "ctrl-alt-space x",
                Copy,
                Some("OtherEditor"),
            )]);
            assert_eq!(prepare(&config, cx).unwrap_err(), "settings-key-conflict");
        });
    }

    #[gpui_kit::test]
    fn loading_conflicting_shortcut_keeps_the_runtime_configuration(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_global(ShortcutsRuntime {
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                manager: Err("must not attempt OS registration".into()),
                config: Default::default(),
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                actions: Default::default(),
                jobs: Default::default(),
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                active: Default::default(),
                error: None,
                paused: true,
                draining: false,
                _events: None,
            });
            let mut config = AppConfig::default();
            config.shortcuts.launcher = "secondary-c".into();
            apply(&config, cx);
            let runtime = cx.global::<ShortcutsRuntime>();
            assert_eq!(runtime.error.as_deref(), Some("settings-key-conflict"));
            assert_eq!(runtime.config, Default::default());
            assert!(!runtime.paused);
        });
    }
}
