use super::*;

impl GlobalHotkeyState {
    fn parse_hotkey(hotkey: &str) -> AiChatResult<HotKey> {
        Ok(HotKey::from_str(hotkey)?)
    }

    fn register_action(
        &mut self,
        hotkey: &str,
        action: RegisteredHotkeyAction,
    ) -> AiChatResult<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()).copied() == Some(action) {
            event!(
                Level::INFO,
                hotkey = %hotkey,
                hotkey_id = hotkey.id(),
                action = ?action,
                "Hotkey action already registered"
            );
            return Ok(());
        }
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?action,
            "Registering hotkey action"
        );
        self.backend.register(hotkey)?;
        self.hotkey_actions.insert(hotkey.id(), action);
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?action,
            "Registered hotkey action"
        );
        Ok(())
    }

    fn unregister_action(
        &mut self,
        hotkey: &str,
        expected_action: RegisteredHotkeyAction,
    ) -> AiChatResult<()> {
        let hotkey = Self::parse_hotkey(hotkey)?;
        if self.hotkey_actions.get(&hotkey.id()).copied() != Some(expected_action) {
            event!(
                Level::INFO,
                hotkey = %hotkey,
                hotkey_id = hotkey.id(),
                expected_action = ?expected_action,
                "Skipping hotkey unregistration because registered action does not match"
            );
            return Ok(());
        }
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?expected_action,
            "Unregistering hotkey action"
        );
        self.backend.unregister(hotkey)?;
        self.hotkey_actions.remove(&hotkey.id());
        event!(
            Level::INFO,
            hotkey = %hotkey,
            hotkey_id = hotkey.id(),
            action = ?expected_action,
            "Unregistered hotkey action"
        );
        Ok(())
    }

    pub(super) fn upsert_binding_runtime(
        &mut self,
        binding: &GlobalShortcutBinding,
    ) -> AiChatResult<()> {
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            input_source = %binding.input_source,
            "Upserting global shortcut runtime binding"
        );
        let action = RegisteredHotkeyAction::ShortcutBinding {
            binding_id: binding.id,
        };
        let previous = self.shortcut_bindings.get(&binding.id).cloned();
        let should_unregister_previous = previous.as_ref().is_some_and(|old_binding| {
            old_binding.enabled && (old_binding.hotkey != binding.hotkey || !binding.enabled)
        });

        if let Some(old_binding) = previous.as_ref()
            && should_unregister_previous
        {
            self.unregister_action(&old_binding.hotkey, action)?;
        }

        if binding.enabled
            && let Err(err) = self.register_action(&binding.hotkey, action)
        {
            if let Some(old_binding) = previous.as_ref()
                && should_unregister_previous
                && old_binding.enabled
            {
                let _ = self.register_action(&old_binding.hotkey, action);
            }
            return Err(err);
        }

        self.shortcut_bindings.insert(binding.id, binding.clone());
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            "Upserted global shortcut runtime binding"
        );
        Ok(())
    }

    pub(super) fn remove_binding_runtime(
        &mut self,
        binding: &GlobalShortcutBinding,
    ) -> AiChatResult<()> {
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            "Removing global shortcut runtime binding"
        );
        self.shortcut_bindings.remove(&binding.id);
        if binding.enabled {
            self.unregister_action(
                &binding.hotkey,
                RegisteredHotkeyAction::ShortcutBinding {
                    binding_id: binding.id,
                },
            )?;
        }
        event!(
            Level::INFO,
            binding_id = binding.id,
            hotkey = %binding.hotkey,
            "Removed global shortcut runtime binding"
        );
        Ok(())
    }

    pub(super) fn load_initial_shortcuts(&mut self, cx: &mut App) -> AiChatResult<()> {
        self.temporary_hotkey = cx.global::<AiChatConfig>().temporary_hotkey.clone();
        if let Some(hotkey) = self.temporary_hotkey.clone() {
            event!(Level::INFO, hotkey = %hotkey, "Loading temporary hotkey");
            self.register_action(&hotkey, RegisteredHotkeyAction::TemporaryWindow)?;
        }

        let mut conn = cx.global::<Db>().get()?;
        let bindings = GlobalShortcutBinding::all(&mut conn)?;
        event!(
            Level::INFO,
            count = bindings.len(),
            "Loading initial global shortcut bindings"
        );
        for binding in bindings {
            self.upsert_binding_runtime(&binding)?;
        }
        event!(
            Level::INFO,
            temporary_hotkey = ?self.temporary_hotkey,
            registered_actions = self.hotkey_actions.len(),
            shortcut_bindings = self.shortcut_bindings.len(),
            "Loaded initial hotkeys"
        );
        Ok(())
    }

    fn action_for_id(&self, hotkey_id: u32) -> Option<RegisteredHotkeyAction> {
        self.hotkey_actions.get(&hotkey_id).copied()
    }

    pub fn update_temporary_hotkey(
        old_hotkey: Option<&str>,
        new_hotkey: Option<&str>,
        cx: &mut App,
    ) -> AiChatResult<()> {
        event!(
            Level::INFO,
            old_hotkey = ?old_hotkey,
            new_hotkey = ?new_hotkey,
            "Updating temporary hotkey"
        );
        let hotkeys = cx.global_mut::<GlobalHotkeyState>();
        if let Some(old_hotkey) = old_hotkey {
            hotkeys.unregister_action(old_hotkey, RegisteredHotkeyAction::TemporaryWindow)?;
        }
        if let Some(new_hotkey) = new_hotkey
            && let Err(err) =
                hotkeys.register_action(new_hotkey, RegisteredHotkeyAction::TemporaryWindow)
        {
            if let Some(old_hotkey) = old_hotkey {
                let _ =
                    hotkeys.register_action(old_hotkey, RegisteredHotkeyAction::TemporaryWindow);
            }
            return Err(err);
        }
        hotkeys.temporary_hotkey = new_hotkey.map(str::to_string);
        event!(
            Level::INFO,
            temporary_hotkey = ?hotkeys.temporary_hotkey,
            "Updated temporary hotkey"
        );
        Ok(())
    }

    pub fn save_global_shortcut_binding(
        id: Option<i32>,
        binding: NewGlobalShortcutBinding,
        cx: &mut App,
    ) -> AiChatResult<GlobalShortcutBinding> {
        event!(
            Level::INFO,
            binding_id = ?id,
            hotkey = %binding.hotkey,
            enabled = binding.enabled,
            provider_name = %binding.provider_name,
            model_id = %binding.model_id,
            input_source = %binding.input_source,
            "Saving global shortcut binding"
        );
        let mut conn = cx.global::<Db>().get()?;
        match id {
            Some(id) => {
                let previous = GlobalShortcutBinding::find(id, &mut conn)?;
                GlobalShortcutBinding::update(
                    id,
                    UpdateGlobalShortcutBinding {
                        hotkey: binding.hotkey.clone(),
                        enabled: binding.enabled,
                        template_id: binding.template_id,
                        provider_name: binding.provider_name.clone(),
                        model_id: binding.model_id.clone(),
                        mode: binding.mode,
                        request_template: binding.request_template.clone(),
                        input_source: binding.input_source,
                    },
                    &mut conn,
                )?;
                let updated = GlobalShortcutBinding::find(id, &mut conn)?;
                let mut runtime_result = Ok(());
                cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                    runtime_result = hotkeys.upsert_binding_runtime(&updated);
                });
                if let Err(err) = runtime_result {
                    let _ = GlobalShortcutBinding::update(
                        id,
                        UpdateGlobalShortcutBinding {
                            hotkey: previous.hotkey.clone(),
                            enabled: previous.enabled,
                            template_id: previous.template_id,
                            provider_name: previous.provider_name.clone(),
                            model_id: previous.model_id.clone(),
                            mode: previous.mode,
                            request_template: previous.request_template.clone(),
                            input_source: previous.input_source,
                        },
                        &mut conn,
                    );
                    let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                        hotkeys.upsert_binding_runtime(&previous)
                    });
                    return Err(err);
                }
                event!(
                    Level::INFO,
                    binding_id = updated.id,
                    hotkey = %updated.hotkey,
                    enabled = updated.enabled,
                    "Updated global shortcut binding"
                );
                Ok(updated)
            }
            None => {
                let created = GlobalShortcutBinding::insert(binding, &mut conn)?;
                let mut runtime_result = Ok(());
                cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                    runtime_result = hotkeys.upsert_binding_runtime(&created);
                });
                if let Err(err) = runtime_result {
                    let _ = GlobalShortcutBinding::delete(created.id, &mut conn);
                    return Err(err);
                }
                event!(
                    Level::INFO,
                    binding_id = created.id,
                    hotkey = %created.hotkey,
                    enabled = created.enabled,
                    "Created global shortcut binding"
                );
                Ok(created)
            }
        }
    }

    pub fn delete_global_shortcut_binding(id: i32, cx: &mut App) -> AiChatResult<()> {
        event!(
            Level::INFO,
            binding_id = id,
            "Deleting global shortcut binding"
        );
        let mut conn = cx.global::<Db>().get()?;
        let previous = GlobalShortcutBinding::find(id, &mut conn)?;
        let mut runtime_result = Ok(());
        cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
            runtime_result = hotkeys.remove_binding_runtime(&previous);
        });
        runtime_result?;
        if let Err(err) = GlobalShortcutBinding::delete(id, &mut conn) {
            let _ = cx.update_global::<GlobalHotkeyState, _>(|hotkeys, _cx| {
                hotkeys.upsert_binding_runtime(&previous)
            });
            return Err(err);
        }
        event!(
            Level::INFO,
            binding_id = id,
            hotkey = %previous.hotkey,
            "Deleted global shortcut binding"
        );
        Ok(())
    }

    pub(super) fn handle_pressed_hotkey(&mut self, hotkey_id: u32, cx: &mut App) {
        let Some(action) = self.action_for_id(hotkey_id) else {
            event!(
                Level::INFO,
                hotkey_id,
                "Ignoring hotkey press with no registered action"
            );
            return;
        };
        event!(Level::INFO, hotkey_id, action = ?action, "Handling pressed hotkey");
        match action {
            RegisteredHotkeyAction::TemporaryWindow => self.toggle_temporary_window(cx),
            RegisteredHotkeyAction::ShortcutBinding { binding_id } => {
                self.dispatch_shortcut_trigger(binding_id, cx)
            }
        }
    }
}
