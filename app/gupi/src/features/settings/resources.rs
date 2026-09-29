use super::*;
use crate::{
    foundation::pi_resources::{self as io, Kind, Resource},
    state::resources::{Change, ResourceController, ResourceEvent},
};
use gpui_form::FormSchema;
use gpui_kit::component::{
    Sizable, WindowExt,
    input::{Editor as CodeEditor, EditorState},
    notification::Notification,
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_operation::{Complete, Load, Refresh, Retry, Transition, refresh};
use std::path::PathBuf;

mod editor;
mod packages;
mod prompts;
mod skills;

#[derive(Clone, Default, PartialEq, FormSchema)]
struct TextDraft {
    text: String,
}
struct Editor {
    path: PathBuf,
    editable: bool,
    create: bool,
    form: Entity<Form<TextDraft>>,
    input: Entity<EditorState>,
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
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Overview,
    Packages,
    Catalog,
}
pub(super) struct ResourcesView {
    pub controller: Entity<ResourceController>,
    config: Entity<ConfigController>,
    applied_pi: Entity<PiProbeController>,
    source: Entity<InputState>,
    names: [Entity<InputState>; 2],
    search: Entity<InputState>,
    prompt_search: Entity<InputState>,
    previews: std::collections::BTreeMap<PathBuf, refresh::Operation<String, io::Error, Task<()>>>,
    editor: Option<Editor>,
    open: refresh::Operation<(), io::Error, Task<()>>,
    creating: Option<Kind>,
    installing: bool,
    expanded_packages: std::collections::BTreeSet<String>,
    error: Option<(Kind, String)>,
    _subscriptions: Vec<Subscription>,
}
impl ResourcesView {
    fn resource_source(&self, resource: &Resource, cx: &App) -> String {
        resource.package.clone().unwrap_or_else(|| {
            let personal = resource.editable
                || self
                    .controller
                    .read(cx)
                    .catalog
                    .data()
                    .is_some_and(|catalog| resource.path.starts_with(&catalog.root));
            t(
                cx,
                if personal {
                    "settings-source-personal"
                } else {
                    "settings-source-external"
                },
            )
        })
    }

    pub(super) fn load_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let worker = cx.background_spawn({
            let path = path.clone();
            async move { std::fs::read_to_string(path).map_err(io::Error::from) }
        });
        let target = path.clone();
        let task = cx.spawn(async move |owner, cx| {
            let result = worker.await;
            let _ = owner.update(cx, |this, cx| {
                if let Some(preview) = this.previews.get_mut(&target) {
                    preview.transition(Complete(result));
                }
                cx.notify();
            });
        });
        let mut preview = refresh::Operation::new();
        preview.transition(Load(task));
        self.previews.insert(path, preview);
        cx.notify();
    }

    pub fn new(
        config: Entity<ConfigController>,
        applied_pi: Entity<PiProbeController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let controller = cx.new(|_| ResourceController::new());
        let source = cx.new(|cx| InputState::new(window, cx));
        let names: [Entity<InputState>; 2] =
            std::array::from_fn(|_| cx.new(|cx| InputState::new(window, cx)));
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t(cx, "settings-skill-search")));
        let prompt_search =
            cx.new(|cx| InputState::new(window, cx).placeholder(t(cx, "settings-template-search")));
        let observe = cx.observe(&controller, |_, _, cx| cx.notify());
        let catalog_changed =
            cx.subscribe(&controller, |this: &mut Self, controller, event, cx| {
                if !matches!(event, ResourceEvent::CatalogChanged) {
                    return;
                }
                if let Some(catalog) = controller.read(cx).catalog.data() {
                    this.expanded_packages.retain(|source| {
                        catalog
                            .packages
                            .iter()
                            .any(|package| &package.source == source)
                    });
                    this.previews.retain(|path, _| {
                        catalog.resources.iter().any(|r| {
                            matches!(r.kind, Kind::Skill | Kind::Prompt) && r.path == *path
                        })
                    });
                }
                cx.notify();
            });
        let change = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        let saved = cx.subscribe_in(&controller, window, |this, _, event, window, cx| {
            let ResourceEvent::Saved(path, text) = event else {
                if let ResourceEvent::Finished { target, result } = event {
                    let mut args = fluent_bundle::FluentArgs::new();
                    args.set("target", target.clone());
                    let message = crate::foundation::i18n::t_with_args(
                        cx,
                        if result.is_ok() {
                            "settings-resource-success"
                        } else {
                            "settings-resource-failed"
                        },
                        &args,
                    );
                    window.push_notification(
                        match result {
                            Ok(()) => Notification::info(message),
                            Err(error) => Notification::error(format!("{message}\n{error}")),
                        },
                        cx,
                    );
                }
                return;
            };

            if this.previews.contains_key(path) {
                this.load_preview(path.clone(), cx);
            }
            if this.editor.as_ref().is_some_and(|editor| {
                &editor.path == path && TextDraft::TEXT.get(&editor.form, cx) == *text
            }) {
                this.editor = None;
                window.close_dialog(cx);
            }
            cx.notify();
        });
        let mut subscriptions = vec![observe, catalog_changed, change, saved];
        for input in [&source, &names[0], &names[1], &prompt_search] {
            subscriptions.push(cx.subscribe(input, |_, _, _: &InputEvent, cx| cx.notify()));
        }
        Self {
            controller,

            config,
            applied_pi,
            source,
            names,
            search,
            prompt_search,
            previews: Default::default(),
            editor: None,
            open: refresh::Operation::new(),
            creating: None,
            installing: false,
            expanded_packages: Default::default(),
            error: None,
            _subscriptions: subscriptions,
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
            self.error = Some((Kind::Extension, t(cx, "settings-resource-pi-required")));
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
                Open {
                    path,
                    kind,
                    editable: true,
                    create: true,
                },
                window,
                cx,
            ),
            Err(_) => {
                self.error = Some((kind, t(cx, "settings-resource-invalid-name")));
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
    fn confirm_remove(
        &mut self,
        resource: Option<Resource>,
        source: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let owner = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let owner = owner.clone();
            let resource = resource.clone();
            let source = source.clone();
            dialog
                .title(t(
                    cx,
                    if resource.is_some() {
                        "settings-resource-delete"
                    } else {
                        "settings-package-remove"
                    },
                ))
                .child(div().font_weight(FontWeight::MEDIUM).child(source.clone()))
                .child(t(
                    cx,
                    if resource.is_some() {
                        "settings-resource-confirm-remove"
                    } else {
                        "settings-package-confirm-remove"
                    },
                ))
                .on_ok(move |_, _, cx| {
                    owner
                        .update(cx, |this, cx| {
                            if this.controller.read(cx).busy() || this.config.read(cx).busy(cx) {
                                return false;
                            }
                            if let Some(resource) = &resource {
                                this.change(Change::Delete(resource.clone()), cx);
                            } else {
                                this.package("remove", source.clone(), cx);
                            }
                            true
                        })
                        .unwrap_or(false)
                })
        });
    }
    pub fn render_header(&self, cx: &Context<Self>) -> AnyElement {
        let controller = self.controller.read(cx);
        h_flex()
            .child(
                Button::new("resources-help")
                    .ghost()
                    .small()
                    .icon(IconName::Info)
                    .tooltip(t(cx, "settings-resource-reload-help")),
            )
            .child(
                Button::new("resources-refresh")
                    .ghost()
                    .small()
                    .icon(IconName::RotateCw)
                    .tooltip(t(cx, "settings-resource-refresh"))
                    .loading(controller.catalog.is_running())
                    .disabled(controller.busy() || self.config.read(cx).busy(cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.controller.update(cx, |c, cx| c.refresh(cx));
                        for path in this.previews.keys().cloned().collect::<Vec<_>>() {
                            this.load_preview(path, cx);
                        }
                    })),
            )
            .into_any_element()
    }
    pub fn has_status(&self, kind: Kind, cx: &App) -> bool {
        let controller = self.controller.read(cx);
        controller.mutation.is_running()
            || controller.catalog.problem().is_some()
            || self.open.problem().is_some()
            || self.open.is_running()
            || self
                .error
                .as_ref()
                .is_some_and(|(error_kind, _)| *error_kind == kind)
            || controller
                .catalog
                .data()
                .is_some_and(|catalog| !catalog.warnings.is_empty())
    }
    pub fn render_page(&self, kind: Kind, section: Section, cx: &mut Context<Self>) -> AnyElement {
        if matches!(
            self.controller.read(cx).catalog,
            refresh::Operation::Idle(_)
        ) {
            let controller = self.controller.clone();
            cx.defer(move |cx| {
                controller.update(cx, |owner, cx| {
                    if matches!(owner.catalog, refresh::Operation::Idle(_)) {
                        owner.refresh(cx);
                    }
                })
            });
        }
        if kind == Kind::Extension {
            let probe = self.applied_pi.clone();
            let command = self.config.read(cx).preferences(cx).pi_command;
            if !probe.read(cx).matches_command(command.as_deref()) {
                cx.defer(move |cx| probe.update(cx, |probe, cx| probe.request(command, false, cx)));
            }
        }
        let controller = self.controller.read(cx);
        let busy = controller.busy() || self.config.read(cx).busy(cx);
        let catalog = controller.catalog.data();
        let mut view = v_flex().gap_3();
        if section == Section::Overview {
            if controller.mutation.is_running() {
                view = view.child(t(cx, "settings-resource-working"));
            }
            for error in controller
                .catalog
                .problem()
                .into_iter()
                .chain(self.open.problem())
            {
                view = view.child(div().text_color(cx.theme().danger).child(error.to_string()));
            }
            if let Some((error_kind, error)) = &self.error
                && *error_kind == kind
            {
                view = view.child(div().text_color(cx.theme().danger).child(error.clone()));
            }
            if self.open.is_running() {
                view = view.child(t(cx, "settings-resource-loading"));
            }
            if let Some(catalog) = catalog {
                for warning in &catalog.warnings {
                    view = view.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(warning.clone()),
                    );
                }
            }
            return view.into_any_element();
        }
        if catalog.is_none() {
            return view.into_any_element();
        }
        if section == Section::Packages {
            return self.render_packages(cx);
        }
        if kind != Kind::Extension {
            let mut toolbar = h_flex().flex_wrap().gap_2();
            if kind == Kind::Prompt {
                toolbar = toolbar.justify_between().child(
                    div()
                        .debug_selector(|| "template-heading".into())
                        .child(t(cx, "settings-template-heading")),
                );
            }
            if kind == Kind::Skill {
                toolbar = toolbar.child(
                    div().flex_1().min_w(px(160.)).child(
                        Input::new(&self.search)
                            .prefix(gpui_kit::component::Icon::new(IconName::Search)),
                    ),
                );
            }
            toolbar = toolbar.child(
                Button::new("resource-add")
                    .icon(IconName::Plus)
                    .label(t(cx, "settings-resource-create"))
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.creating = Some(kind);
                        this.name_input(kind).focus_handle(cx).focus(window, cx);
                        cx.notify();
                    })),
            );
            if kind == Kind::Skill {
                toolbar = toolbar.child(
                    Button::new("resource-register")
                        .ghost()
                        .label(t(cx, "settings-resource-register"))
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| this.pick_skill(cx))),
                );
            }
            view = view.child(toolbar);
            if kind == Kind::Prompt {
                view = view.child(
                    Input::new(&self.prompt_search)
                        .prefix(gpui_kit::component::Icon::new(IconName::Search)),
                );
            }
            if self.creating == Some(kind) {
                view = view
                    .child(
                        gpui_kit::component::form::field()
                            .label(t(cx, "settings-resource-name"))
                            .description(t(cx, "settings-resource-name-help"))
                            .child(Input::new(self.name_input(kind)).disabled(busy)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("resource-create")
                                    .primary()
                                    .label(t(cx, "settings-resource-create"))
                                    .disabled(
                                        busy || self
                                            .name_input(kind)
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .is_empty(),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.new_resource(kind, window, cx)
                                    })),
                            )
                            .child(
                                Button::new("resource-create-cancel")
                                    .ghost()
                                    .label(t(cx, "action-cancel"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.creating = None;
                                        cx.notify();
                                    })),
                            ),
                    );
            }
        }
        if kind == Kind::Skill {
            if self.filtered_skills(cx).is_empty() {
                view = view.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t(
                            cx,
                            if self.search.read(cx).value().trim().is_empty() {
                                "settings-resource-none"
                            } else {
                                "settings-resource-empty"
                            },
                        )),
                );
            }
            return view.into_any_element();
        }
        if kind == Kind::Prompt && self.filtered_prompts(cx).is_empty() {
            view = view.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(
                        cx,
                        if self.prompt_search.read(cx).value().trim().is_empty() {
                            "settings-resource-none"
                        } else {
                            "settings-resource-empty"
                        },
                    )),
            );
        }
        view.into_any_element()
    }
}
fn io_counts(catalog: &io::Catalog, source: &str, cx: &App) -> Option<Vec<String>> {
    let mut counts = Vec::new();
    for (kind, label) in [
        (Kind::Extension, "settings-package-extensions"),
        (Kind::Skill, "settings-package-skills"),
        (Kind::Theme, "settings-package-themes"),
        (Kind::Prompt, "settings-package-prompts"),
    ] {
        let count = catalog
            .resources
            .iter()
            .filter(|r| r.package.as_deref() == Some(source) && r.kind == kind)
            .count();
        if count > 0 {
            let mut args = fluent_bundle::FluentArgs::new();
            args.set("count", count as i64);
            counts.push(crate::foundation::i18n::t_with_args(cx, label, &args));
        }
    }
    (!counts.is_empty()).then_some(counts)
}

// The name and description already appear in the card; preview only the Markdown body.
fn preview_body(text: &str) -> &str {
    let mut lines = text.split_inclusive('\n');
    if lines.next().is_some_and(|line| line.trim() == "---") {
        let mut offset = text.find('\n').map_or(text.len(), |index| index + 1);
        for line in lines {
            offset += line.len();
            if line.trim() == "---" {
                return text[offset..].trim_start();
            }
        }
    }
    text
}
