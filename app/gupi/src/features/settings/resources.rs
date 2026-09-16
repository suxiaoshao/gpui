use super::*;
use crate::{
    foundation::pi_resources::{self as io, Kind, Resource},
    state::resources::{Change, ResourceController, ResourceEvent},
};
use gpui_form::FormSchema;
use gpui_kit::component::{
    input::{Textarea, TextareaState},
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use std::path::PathBuf;

#[derive(Clone, Default, PartialEq, FormSchema)]
struct TextDraft {
    text: String,
}
struct Editor {
    path: PathBuf,
    kind: Kind,
    editable: bool,
    create: bool,
    form: Entity<Form<TextDraft>>,
    input: Entity<TextareaState>,
    _binding: ControlBinding,
    _subscriptions: Vec<Subscription>,
}
#[derive(Clone)]
struct Open {
    path: PathBuf,
    kind: Kind,
    editable: bool,
    create: bool,
}
pub(super) struct ResourcesView {
    pub controller: Entity<ResourceController>,
    config: Entity<ConfigController>,
    applied_pi: Entity<PiProbeController>,
    source: Entity<InputState>,
    names: [Entity<InputState>; 2],
    search: Entity<InputState>,
    editor: Option<Editor>,
    open: refresh::Operation<(), io::Error, Task<()>>,
    pending_open: Option<Option<Open>>,
    delete: Option<Resource>,
    remove_package: Option<String>,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}
impl ResourcesView {
    pub fn new(
        config: Entity<ConfigController>,
        applied_pi: Entity<PiProbeController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let controller = cx.new(|_| ResourceController::new());
        let source = cx.new(|cx| InputState::new(window, cx));
        let names = std::array::from_fn(|_| cx.new(|cx| InputState::new(window, cx)));
        let search = cx.new(|cx| InputState::new(window, cx));
        let observe = cx.observe(&controller, |_, _, cx| cx.notify());
        let change = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        let saved = cx.subscribe(&controller, |this, _, event, cx| {
            let ResourceEvent::Saved(path, text) = event;
            if let Some(editor) = &mut this.editor
                && &editor.path == path
                && TextDraft::TEXT.get(&editor.form, cx) == *text
            {
                editor.create = false;
                editor.form.update(cx, |form, cx| {
                    form.rebase(TextDraft { text: text.clone() }, cx)
                });
            }
            cx.notify();
        });
        Self {
            controller,
            config,
            applied_pi,
            source,
            names,
            search,
            editor: None,
            open: refresh::Operation::new(),
            pending_open: None,
            delete: None,
            remove_package: None,
            error: None,
            _subscriptions: vec![observe, change, saved],
        }
    }
    fn change(&mut self, change: Change, cx: &mut Context<Self>) {
        if self.config.read(cx).busy(cx) {
            return;
        }
        self.error = None;
        self.controller.update(cx, |c, cx| c.change(change, cx));
    }
    fn package(&mut self, action: &'static str, source: String, cx: &mut Context<Self>) {
        let command = self.config.read(cx).preferences(cx).pi_command;
        let Some(command) = self
            .applied_pi
            .read(cx)
            .ready_for(command.as_deref())
            .map(|p| p.command.clone())
        else {
            self.error = Some(t(cx, "settings-resource-pi-required"));
            cx.notify();
            return;
        };
        self.change(
            Change::Package {
                command,
                action,
                source,
            },
            cx,
        );
    }
    fn request_open(&mut self, next: Option<Open>, window: &mut Window, cx: &mut Context<Self>) {
        if self.controller.read(cx).busy() || self.open.is_running() {
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|e| e.form.read(cx).is_dirty())
        {
            self.pending_open = Some(next);
            cx.notify();
            return;
        }
        self.open_editor(next, window, cx);
    }
    fn open_editor(&mut self, next: Option<Open>, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_open = None;
        self.error = None;
        let Some(next) = next else {
            self.editor = None;
            self.open = refresh::Operation::new();
            cx.notify();
            return;
        };
        if next.create {
            self.open = refresh::Operation::new();
            let text = if next.kind == Kind::Skill {
                format!(
                    "---\nname: {}\ndescription: \n---\n\n",
                    next.path
                        .parent()
                        .and_then(|p| p.file_name())
                        .unwrap_or_default()
                        .to_string_lossy()
                )
            } else {
                String::new()
            };
            self.editor = Some(Self::editor(next, text, window, cx));
            cx.notify();
            return;
        }
        let worker = cx.background_spawn({
            let path = next.path.clone();
            async move {
                match std::fs::read_to_string(&path) {
                    Ok(text) => Ok(text),
                    Err(e)
                        if e.kind() == std::io::ErrorKind::NotFound
                            && path
                                .file_name()
                                .is_some_and(|n| n == "SYSTEM.md" || n == "APPEND_SYSTEM.md") =>
                    {
                        Ok(String::new())
                    }
                    Err(e) => Err(io::Error::from(e)),
                }
            }
        });
        let task = cx.spawn_in(window, async move |owner, cx| {
            let result = worker.await;
            let _ = owner.update_in(cx, |this, window, cx| {
                match result {
                    Ok(text) => {
                        this.editor = Some(Self::editor(next, text, window, cx));
                        this.open.transition(Complete(Ok(())));
                    }
                    Err(error) => {
                        this.open.transition(Complete(Err(error)));
                    }
                }
                cx.notify();
            });
        });
        match &self.open {
            refresh::Operation::Idle(_) => self.open.transition(Load(task)),
            refresh::Operation::Unavailable(_) => self.open.transition(Retry(task)),
            _ => self.open.transition(Refresh(task)),
        }
        cx.notify();
    }
    fn editor(next: Open, text: String, window: &mut Window, cx: &mut Context<Self>) -> Editor {
        let form = cx.new(|_| Form::new(TextDraft { text }));
        let input = cx.new(|cx| TextareaState::new(window, cx));
        let (binding, writer) = TextDraft::TEXT.bind_control_in(
            &form,
            &input,
            |input, projection, window, cx| {
                if let ControlProjection::Value(text) = projection {
                    input.set_value(text, window, cx);
                }
            },
            window,
            cx,
        );
        let input_sub = cx.subscribe_in(&input, window, move |_, input, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                writer.defer_set(input.read(cx).value().to_string(), window, cx);
            }
        });
        let form_sub = cx.observe(&form, |_, _, cx| cx.notify());
        Editor {
            path: next.path,
            kind: next.kind,
            editable: next.editable,
            create: next.create,
            form,
            input,
            _binding: binding,
            _subscriptions: vec![input_sub, form_sub],
        }
    }
    fn name_input(&self, kind: Kind) -> &Entity<InputState> {
        &self.names[usize::from(kind == Kind::Prompt)]
    }
    fn new_resource(&mut self, kind: Kind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self
            .controller
            .read(cx)
            .catalog
            .data()
            .map(|c| c.root.clone())
        else {
            return;
        };
        match io::create_path(&root, kind, &self.name_input(kind).read(cx).value()) {
            Ok(path) => self.request_open(
                Some(Open {
                    path,
                    kind,
                    editable: true,
                    create: true,
                }),
                window,
                cx,
            ),
            Err(_) => {
                self.error = Some(t(cx, "settings-resource-invalid-name"));
                cx.notify();
            }
        }
    }
    fn pick_skill(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some(t(cx, "settings-resource-register").into()),
        });
        cx.spawn(async move |owner, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = owner.update(cx, |this, cx| this.change(Change::RegisterSkill(path), cx));
            }
        })
        .detach();
    }
    fn render_editor(&self, cx: &Context<Self>) -> AnyElement {
        let busy = self.controller.read(cx).busy() || self.open.is_running();
        let mut view = v_flex().gap_3();
        if let Some(editor) = &self.editor {
            let path = editor.path.clone();
            let form = editor.form.clone();
            let create = editor.create;
            view = view
                .child(div().child(editor.path.to_string_lossy().into_owned()))
                .child(
                    Textarea::new(&editor.input)
                        .h(px(320.))
                        .readonly(!editor.editable)
                        .disabled(busy),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .when(editor.editable, |row| {
                            row.child(
                                Button::new("resource-save")
                                    .primary()
                                    .label(t(cx, "settings-resource-save"))
                                    .disabled(busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Ok(prepared) =
                                            form.update(cx, |form, cx| form.prepare(cx))
                                        {
                                            this.change(
                                                Change::Save {
                                                    path: path.clone(),
                                                    text: prepared.value().text.clone(),
                                                    create,
                                                },
                                                cx,
                                            );
                                        }
                                    })),
                            )
                        })
                        .child(
                            Button::new("resource-close")
                                .label(t(cx, "settings-resource-close"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.request_open(None, window, cx)
                                })),
                        ),
                );
            if !editor.editable {
                view = view.child(t(cx, "settings-resource-readonly"));
                if let Some(catalog) = self.controller.read(cx).catalog.data()
                    && let Some(source) = catalog
                        .resources
                        .iter()
                        .find(|r| r.path == editor.path)
                        .and_then(|r| r.package.as_ref())
                    && let Some(package) = catalog.packages.iter().find(|p| &p.source == source)
                {
                    let path = package.path.clone();
                    view = view.child(
                        Button::new("editor-package")
                            .icon(IconName::Puzzle)
                            .label(t(cx, "settings-package-open"))
                            .on_click(move |_, _, cx| cx.reveal_path(&path)),
                    );
                }
            }
        }
        if self.pending_open.is_some() {
            view = view.child(t(cx, "settings-editor-discard")).child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("editor-discard")
                            .label(t(cx, "action-confirm"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                let next = this.pending_open.take().flatten();
                                this.open_editor(next, window, cx);
                            })),
                    )
                    .child(
                        Button::new("editor-keep")
                            .label(t(cx, "action-cancel"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pending_open = None;
                                cx.notify();
                            })),
                    ),
            );
        }
        view.into_any_element()
    }

    pub fn render_page(&self, kind: Kind, cx: &mut Context<Self>) -> AnyElement {
        let controller = self.controller.read(cx);
        let busy = controller.busy() || self.config.read(cx).busy(cx);
        let catalog = controller.catalog.data();
        let mut view = v_flex()
            .gap_4()
            .child(
                h_flex().gap_2().child(
                    Button::new("resources-refresh")
                        .label(t(cx, "settings-resource-refresh"))
                        .loading(controller.catalog.is_running())
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.controller.update(cx, |c, cx| c.refresh(cx))
                        })),
                ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(cx, "settings-resource-reload-help")),
            );
        if controller.mutation.is_running() {
            view = view.child(t(cx, "settings-resource-working"));
        } else if controller.mutation.data().is_some() && controller.mutation.problem().is_none() {
            view = view.child(t(cx, "settings-resource-saved"));
        }
        for error in controller
            .catalog
            .problem()
            .into_iter()
            .chain(controller.mutation.problem())
            .chain(self.open.problem())
        {
            view = view.child(div().text_color(cx.theme().danger).child(error.to_string()));
        }
        if let Some(error) = &self.error {
            view = view.child(div().text_color(cx.theme().danger).child(error.clone()));
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.kind == kind)
            || self.pending_open.is_some()
        {
            return view.child(self.render_editor(cx)).into_any_element();
        }
        if let Some(catalog) = catalog {
            for warning in &catalog.warnings {
                view = view.child(div().text_sm().child(warning.clone()));
            }
            if kind == Kind::Extension {
                let configured = self.config.read(cx).preferences(cx).pi_command;
                let pi_ready = self
                    .applied_pi
                    .read(cx)
                    .ready_for(configured.as_deref())
                    .is_some();
                view = view.child(div().text_lg().child(t(cx, "settings-package-heading")));
                if !pi_ready {
                    view = view.child(t(cx, "settings-resource-pi-required"));
                }
                view = view
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.source).disabled(busy || !pi_ready)),
                            )
                            .child(
                                Button::new("package-install")
                                    .label(t(cx, "settings-package-install"))
                                    .disabled(busy || !pi_ready)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.package(
                                            "install",
                                            this.source.read(cx).value().to_string(),
                                            cx,
                                        )
                                    })),
                            ),
                    )
                    .child(div().text_sm().child(t(cx, "settings-package-source-help")));
                for (index, package) in catalog.packages.iter().enumerate() {
                    let update = package.source.clone();
                    let remove = update.clone();
                    let path = package.path.clone();
                    let counts = io_counts(catalog, &package.source, cx);
                    view = view.child(
                        v_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(div().flex_1().child(package.source.clone()))
                                    .children(package.version.clone())
                                    .child(
                                        Button::new(("package-update", index))
                                            .label(t(cx, "settings-package-update"))
                                            .disabled(busy || !pi_ready)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.package("update", update.clone(), cx)
                                            })),
                                    )
                                    .child(
                                        Button::new(("package-remove", index))
                                            .label(t(cx, "settings-package-remove"))
                                            .disabled(busy || !pi_ready)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.remove_package = Some(remove.clone());
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                Button::new(("package-path", index))
                                    .ghost()
                                    .label(path.to_string_lossy().into_owned())
                                    .on_click(move |_, _, cx| cx.reveal_path(&path)),
                            )
                            .child(div().text_sm().child(counts)),
                    );
                }
            } else {
                view = view.child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .child(Input::new(self.name_input(kind)).disabled(busy)),
                        )
                        .child(
                            Button::new("resource-new")
                                .label(t(cx, "settings-resource-create"))
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.new_resource(kind, window, cx)
                                })),
                        )
                        .when(kind == Kind::Skill, |row| {
                            row.child(
                                Button::new("resource-register")
                                    .label(t(cx, "settings-resource-register"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.pick_skill(cx))),
                            )
                        }),
                );
                view = view.child(div().text_sm().child(t(cx, "settings-resource-name-help")));
                if kind == Kind::Skill {
                    view = view.child(
                        gpui_kit::component::form::field()
                            .label(t(cx, "settings-skill-search"))
                            .child(Input::new(&self.search)),
                    );
                }
            }
            if kind == Kind::Extension {
                view = view.child(div().text_lg().child(t(cx, "settings-extension-heading")));
            }
            if kind == Kind::Prompt {
                view = view.child(div().text_lg().child(t(cx, "settings-template-heading")));
            }
            let search = if kind == Kind::Skill {
                self.search.read(cx).value().to_lowercase()
            } else {
                String::new()
            };
            let resources: Vec<_> = catalog
                .resources
                .iter()
                .filter(|r| {
                    r.kind == kind
                        && format!("{} {}", r.name, r.description)
                            .to_lowercase()
                            .contains(&search)
                })
                .collect();
            if resources.is_empty() {
                view = view.child(t(cx, "settings-resource-empty"));
            }
            for (index, resource) in resources.into_iter().enumerate() {
                let toggle = resource.clone();
                let open = Open {
                    path: resource.path.clone(),
                    kind,
                    editable: resource.editable,
                    create: false,
                };
                let remove = resource.clone();
                let path = resource.path.clone();
                let mut row = h_flex()
                    .gap_2()
                    .child(div().flex_1().child(resource.name.clone()))
                    .child(
                        Switch::new(("resource-toggle", index))
                            .checked(resource.enabled)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, active, _, cx| {
                                this.change(Change::Toggle(toggle.clone(), *active), cx)
                            })),
                    );
                if kind != Kind::Extension {
                    row = row.child(
                        Button::new(("resource-open", index))
                            .label(t(cx, "settings-resource-open"))
                            .disabled(busy || self.open.is_running())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.request_open(Some(open.clone()), window, cx)
                            })),
                    );
                }
                if resource.editable {
                    row = row.child(
                        Button::new(("resource-delete", index))
                            .label(t(cx, "settings-resource-delete"))
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.delete = Some(remove.clone());
                                cx.notify();
                            })),
                    );
                }
                view = view.child(
                    v_flex()
                        .gap_1()
                        .child(row)
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(resource.description.clone()),
                        )
                        .children(
                            resource
                                .package
                                .as_ref()
                                .map(|p| div().text_sm().child(p.clone())),
                        )
                        .child(
                            Button::new(("resource-path", index))
                                .ghost()
                                .label(path.to_string_lossy().into_owned())
                                .on_click(move |_, _, cx| cx.reveal_path(&path)),
                        ),
                );
            }
            if kind == Kind::Prompt {
                view = view.child(div().text_lg().child(t(cx, "settings-system-heading")));
                for name in ["SYSTEM.md", "APPEND_SYSTEM.md"] {
                    let open = Open {
                        path: catalog.root.join(name),
                        kind,
                        editable: true,
                        create: false,
                    };
                    view = view.child(Button::new(name).label(name).disabled(busy).on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.request_open(Some(open.clone()), window, cx)
                        }),
                    ));
                }
                view = view.child(div().text_sm().child(t(cx, "settings-system-prompt-help")));
            }
        }
        if self.open.is_running() {
            view = view.child(t(cx, "settings-resource-loading"));
        }
        if self.delete.is_some() || self.remove_package.is_some() {
            view = view
                .child(t(
                    cx,
                    if self.delete.is_some() {
                        "settings-resource-confirm-remove"
                    } else {
                        "settings-package-confirm-remove"
                    },
                ))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("resource-confirm-delete")
                                .label(t(cx, "action-confirm"))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(resource) = this.delete.take() {
                                        if this
                                            .editor
                                            .as_ref()
                                            .is_some_and(|e| e.path == resource.path)
                                        {
                                            this.editor = None;
                                        }
                                        this.change(Change::Delete(resource), cx);
                                    } else if let Some(source) = this.remove_package.take() {
                                        this.package("remove", source, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("resource-cancel-delete")
                                .label(t(cx, "action-cancel"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.delete = None;
                                    this.remove_package = None;
                                    cx.notify();
                                })),
                        ),
                );
        }
        view.into_any_element()
    }
}
fn io_counts(catalog: &io::Catalog, source: &str, cx: &App) -> String {
    let mut args = fluent_bundle::FluentArgs::new();
    for kind in [Kind::Extension, Kind::Skill, Kind::Prompt, Kind::Theme] {
        args.set(
            kind.key(),
            catalog
                .resources
                .iter()
                .filter(|r| r.package.as_deref() == Some(source) && r.kind == kind)
                .count() as i64,
        );
    }
    crate::foundation::i18n::t_with_args(cx, "settings-package-counts", &args)
}
