use super::*;
use crate::features::composer::Composer;
use crate::features::home::pickers::{Picker, PickerEvent, Projection};
use crate::{
    foundation::pi_resources,
    state::{
        pi,
        shortcuts::{InputSource, ModelChoice},
    },
};
use gpui_kit::component::{
    combobox::Combobox,
    form::{field, v_form},
    input::{Textarea, TextareaState},
    searchable_list::SearchableListItem,
};
use pi_rpc::{Client, LaunchOptions, protocol::Model};
#[derive(Clone)]
struct Choice {
    id: String,
    label: SharedString,
}
impl SearchableListItem for Choice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone()
    }
    fn value(&self) -> &String {
        &self.id
    }
}
type Choices = Entity<ComboboxState<SearchableVec<Choice>>>;
fn choices(
    items: Vec<(String, String)>,
    selected: String,
    window: &mut Window,
    cx: &mut App,
) -> Choices {
    cx.new(|cx| {
        let mut state = ComboboxState::new(
            SearchableVec::new(
                items
                    .into_iter()
                    .map(|(id, label)| Choice {
                        id,
                        label: label.into(),
                    })
                    .collect::<Vec<_>>(),
            ),
            vec![],
            window,
            cx,
        )
        .searchable(true);
        state.set_selected_values(&[selected], window, cx);
        state
    })
}
pub(super) fn open(
    definition: ShortcutTask,
    controller: Entity<ConfigController>,
    window: &mut Window,
    cx: &mut App,
) {
    let editor = cx.new(|cx| Editor::new(definition, controller, window, cx));
    window.open_dialog(cx, move |dialog, _, cx| {
        let save = editor.clone();
        let cancel = editor.clone();
        let busy = editor.read(cx).controller.read(cx).busy(cx);
        let loading = editor.read(cx).loading;
        dialog
            .title(t(cx, "shortcut-edit"))
            .width(px(560.))
            .overlay_closable(false)
            .child(editor.clone())
            .close_button(!busy)
            .footer(dialog_buttons("shortcut-save-task", busy, loading, cx))
            .on_cancel(move |_, _, cx| !cancel.read(cx).controller.read(cx).busy(cx))
            .on_ok(move |_, _, cx| save.update(cx, |s, cx| s.save(cx)))
    });
}
struct Editor {
    definition: ShortcutTask,
    controller: Entity<ConfigController>,
    name: Entity<InputState>,
    template: Choices,
    source: Choices,
    picker: Entity<Picker>,
    composer: Entity<TextareaState>,
    model: Option<ModelChoice>,
    thinking: Option<String>,
    levels: Vec<String>,
    default_thinking: String,
    models: Vec<Model>,
    default_model: Option<Model>,
    saving: Option<Shortcuts>,
    error: Option<String>,
    loading: bool,
    query: Option<pi::InstanceId>,
    client: Option<Client>,
    task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}
impl Editor {
    fn new(
        definition: ShortcutTask,
        controller: Entity<ConfigController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(definition.name.clone()));
        let composer = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 8));
        let template_path = definition.template.to_string_lossy().into_owned();
        let template = choices(
            if template_path.is_empty() {
                vec![]
            } else {
                vec![(template_path.clone(), template_path.clone())]
            },
            template_path,
            window,
            cx,
        );
        let source = choices(
            vec![
                ("selection".into(), t(cx, "shortcut-selection")),
                ("clipboard".into(), t(cx, "shortcut-clipboard")),
                ("fallback".into(), t(cx, "shortcut-fallback")),
            ],
            match definition.source {
                InputSource::Selection => "selection",
                InputSource::Clipboard => "clipboard",
                InputSource::SelectionOrClipboard => "fallback",
            }
            .into(),
            window,
            cx,
        );
        let owner = cx.entity().downgrade();
        let picker = cx.new(|cx| {
            Picker::new(
                move |cx| {
                    owner
                        .upgrade()
                        .map(|owner| owner.read(cx).projection(cx))
                        .unwrap_or_default()
                },
                window,
                cx,
            )
        });
        let sub = cx.subscribe_in(&picker, window, |this, _, event, window, cx| {
            match event {
                PickerEvent::Model(model) => {
                    this.model = Some(ModelChoice {
                        provider: model.provider.clone(),
                        id: model.id.clone(),
                    });
                    this.load_levels(window, cx);
                }
                PickerEvent::Thinking(level) => this.thinking = Some(level.clone()),
                PickerEvent::ResetModel => {
                    this.model = None;
                    this.load_levels(window, cx);
                }
                PickerEvent::ResetThinking => this.thinking = None,
                PickerEvent::Load | PickerEvent::Refresh => this.load(window, cx),
            }
            this.sync_picker(window, cx);
            cx.notify();
        });
        let store = controller.read(cx).store.clone();
        let config_sub = store.observe_in(cx, window, |this, op, window, cx| {
            if this.saving.is_some() && !op.is_running() {
                if op.problem().is_none()
                    && op
                        .data()
                        .and_then(|d| d.configured())
                        .is_some_and(|c| Some(&c.shortcuts) == this.saving.as_ref())
                {
                    window.close_dialog(cx);
                }
                this.error = op.problem().map(ToString::to_string);
                this.saving = None;
                cx.notify();
            }
        });
        cx.on_release(|this, cx| {
            if let Some(id) = this.query {
                pi::global(cx)
                    .update(cx, |pi, cx| pi.close(id, cx))
                    .detach();
            }
        })
        .detach();
        let mut this = Self {
            model: definition.model.clone(),
            thinking: definition.thinking.clone(),
            definition,
            controller,
            name,
            template,
            source,
            picker,
            composer,
            levels: vec![],
            default_thinking: String::new(),
            models: vec![],
            default_model: None,
            saving: None,
            error: None,
            loading: false,
            query: None,
            client: None,
            task: None,
            _subscriptions: vec![sub, config_sub],
        };
        this.load(window, cx);
        this
    }
    fn projection(&self, cx: &App) -> Projection {
        let model = self
            .model
            .as_ref()
            .and_then(|choice| {
                self.models
                    .iter()
                    .find(|m| m.provider == choice.provider && m.id == choice.id)
            })
            .or(self.default_model.as_ref());
        let name = self
            .model
            .as_ref()
            .map(|choice| {
                self.models
                    .iter()
                    .find(|m| m.provider == choice.provider && m.id == choice.id)
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| choice.id.clone())
            })
            .unwrap_or_else(|| t(cx, "shortcut-default-model"));
        Projection::for_settings(
            &self.models,
            model,
            name,
            self.levels.clone(),
            self.thinking
                .clone()
                .unwrap_or_else(|| self.default_thinking.clone()),
            self.loading || self.controller.read(cx).busy(cx),
            (self.model.is_some(), self.thinking.is_some()),
        )
    }
    fn sync_picker(&self, window: &mut Window, cx: &mut Context<Self>) {
        // The picker queries this editor; defer until its mutable borrow is released.
        let picker = self.picker.clone();
        window.defer(cx, move |window, cx| {
            picker.update(cx, |s, cx| s.sync_controls(window, cx))
        });
    }
    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        if let Some(id) = self.query.take() {
            pi::global(cx)
                .update(cx, |pi, cx| pi.close(id, cx))
                .detach();
        }
        let selected_template = self.template.read(cx).selected_value().unwrap_or_default();
        self.client = None;
        self.loading = true;
        self.error = None;
        let command = cx
            .try_global::<crate::app::temporary::Temporary>()
            .and_then(|t| t.command.clone());
        let launch = command.and_then(|command| {
            let mut options = LaunchOptions::new(command, std::env::temp_dir());
            options.args.push("--no-session".into());
            match pi::global(cx).update(cx, |pi, cx| pi.start(options, cx)) {
                Ok(id) => {
                    self.query = Some(id);
                    Some(id)
                }
                Err(e) => {
                    self.error = Some(e.to_string());
                    None
                }
            }
        });
        let pi = pi::global(cx);
        self.task = Some(cx.spawn_in(window, async move |owner, cx| {
            let catalog =
                smol::unblock(|| pi_resources::scan(pi_resources::agent_dir()?, None)).await;
            let _ = owner.update_in(cx, |this, window, cx| {
                match catalog {
                    Ok(catalog) => {
                        let options: Vec<_> = catalog
                            .resources
                            .into_iter()
                            .filter(|r| r.kind == pi_resources::Kind::Prompt && r.enabled)
                            .map(|r| Choice {
                                id: r.path.to_string_lossy().into_owned(),
                                label: format!(
                                    "{} — {}",
                                    r.name,
                                    r.package.unwrap_or_else(|| t(cx, "shortcut-personal"))
                                )
                                .into(),
                            })
                            .collect();
                        this.template.update(cx, |s, cx| {
                            s.set_items(SearchableVec::new(options), window, cx);
                            s.set_selected_values(&[selected_template], window, cx);
                        });
                    }
                    Err(e) => this.error = Some(e.to_string()),
                }
                cx.notify();
            });
            let result = async {
                let id = launch.ok_or("Pi is not ready")?;
                let start = std::time::Instant::now();
                let client = loop {
                    let client = pi
                        .read_with(cx, |pi, _| pi.client(id))
                        .map_err(|e| e.to_string())?;
                    if let Some(client) = client {
                        break client;
                    }
                    if start.elapsed().as_secs() > 30 {
                        return Err("Pi connection timed out".to_owned());
                    }
                    smol::Timer::after(std::time::Duration::from_millis(30)).await;
                };
                let ready = client.ready().await.map_err(|e| e.to_string())?;
                let models = client
                    .get_available_models()
                    .await
                    .map_err(|e| e.to_string())?
                    .models;
                Ok((client, models, ready.model, ready.thinking_level))
            }
            .await;
            let _ = owner.update_in(cx, |this, window, cx| {
                this.loading = false;
                match result {
                    Ok((client, models, default_model, default_thinking)) => {
                        this.default_thinking = default_thinking;
                        this.default_model = default_model;
                        this.client = Some(client);
                        this.models = models;
                        this.load_levels(window, cx);
                    }
                    Err(e) => this.error = Some(e),
                }
                this.sync_picker(window, cx);
                cx.notify();
            });
        }));
    }
    fn load_levels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let model = self
            .model
            .as_ref()
            .and_then(|choice| {
                self.models
                    .iter()
                    .find(|m| m.provider == choice.provider && m.id == choice.id)
            })
            .cloned()
            .or_else(|| self.default_model.clone());
        self.loading = true;
        self.task = Some(cx.spawn_in(window, async move |owner, cx| {
            let result = async {
                if let Some(model) = model {
                    client
                        .set_model(model.provider, model.id)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                client
                    .get_available_thinking_levels()
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;
            let _ = owner.update_in(cx, |this, window, cx| {
                this.loading = false;
                match result {
                    Ok(levels) => {
                        this.levels = levels.levels;
                    }
                    Err(e) => this.error = Some(e),
                }
                this.sync_picker(window, cx);
                cx.notify();
            });
        }));
    }
    fn save(&mut self, cx: &mut Context<Self>) -> bool {
        if self.controller.read(cx).busy(cx) || self.loading {
            return false;
        }
        let mut definition = self.definition.clone();
        definition.name = self.name.read(cx).value().trim().into();
        let Some(template) = self
            .template
            .read(cx)
            .selected_value()
            .filter(|v| !v.is_empty())
        else {
            self.error = Some(t(cx, "shortcut-template-required"));
            cx.notify();
            return false;
        };
        definition.template = template.into();
        definition.source = match self.source.read(cx).selected_value().as_deref() {
            Some("selection") => InputSource::Selection,
            Some("clipboard") => InputSource::Clipboard,
            _ => InputSource::SelectionOrClipboard,
        };
        definition.model = self.model.clone();
        definition.thinking = self.thinking.clone();
        let mut config = self.controller.read(cx).preferences(cx).shortcuts;
        config.tasks.retain(|t| t.id != definition.id);
        config.tasks.push(definition);
        if let Err(e) = config.validate() {
            self.error = Some(t(cx, &e));
            cx.notify();
            return false;
        }
        self.saving = Some(config.clone());
        self.controller.update(cx, |c, cx| {
            c.set_preference(PreferenceChange::Shortcuts(config), cx)
        });
        if !self.controller.read(cx).busy(cx) {
            self.saving = None;
            return true;
        }
        false
    }
}
impl Render for Editor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.loading || self.controller.read(cx).busy(cx);
        v_flex()
            .gap_3()
            .child(
                v_form()
                    .child(
                        field()
                            .label(t(cx, "shortcut-name"))
                            .child(Input::new(&self.name).disabled(disabled)),
                    )
                    .child(
                        field().label(t(cx, "shortcut-template")).child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Combobox::new(&self.template)
                                        .placeholder(t(cx, "shortcut-select-template"))
                                        .flex_1()
                                        .min_w_0()
                                        .cleanable(true)
                                        .disabled(disabled),
                                )
                                .child(
                                    Button::new("reload-task-options")
                                        .ghost()
                                        .small()
                                        .icon(IconName::RefreshCw)
                                        .tooltip(t(cx, "shortcut-reload-options"))
                                        .accessibility_label(t(cx, "shortcut-reload-options"))
                                        .loading(self.loading)
                                        .disabled(disabled)
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.load(window, cx)
                                            }),
                                        ),
                                ),
                        ),
                    )
                    .child(
                        field().label(t(cx, "shortcut-source")).child(
                            Combobox::new(&self.source)
                                .w_full()
                                .cleanable(false)
                                .disabled(disabled),
                        ),
                    )
                    .child(
                        field().label(t(cx, "composer-model-thinking")).child(
                            Composer::new(
                                Textarea::new(&self.composer)
                                    .appearance(false)
                                    .disabled(true)
                                    .aria_label(t(cx, "conversation-input")),
                                self.picker.clone(),
                            )
                            .build(cx),
                        ),
                    ),
            )
            .children(
                self.error
                    .as_ref()
                    .map(|e| div().text_color(cx.theme().danger).child(e.clone())),
            )
    }
}
