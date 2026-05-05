mod model_picker;

use super::picker::{
    PickerListDelegate, PickerPopoverOptions, PickerTrigger, render_picker_popover,
};
use crate::{
    features::settings::open_provider_settings_window,
    foundation::assets::IconName,
    foundation::i18n::I18n,
    llm::{ProviderModel, ProviderModelsFailure, provider_is_configured},
    state::{AiChatConfig, ModelStore, ModelStoreSnapshot, ModelStoreStatus},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    list::ListState,
    notification::{Notification, NotificationType},
};
use model_picker::{ModelOption, model_sections};
use std::ops::Deref;

#[derive(Clone)]
pub(crate) enum ModelSelectEvent {
    Change(Option<ProviderModel>),
    ModelsChanged,
}

impl EventEmitter<ModelSelectEvent> for ModelSelect {}

pub(crate) struct ModelSelect {
    models: Vec<ProviderModel>,
    selected_model: Option<ProviderModel>,
    model_picker: Entity<ListState<PickerListDelegate<ModelOption>>>,
    model_picker_bounds: Bounds<Pixels>,
    model_picker_loading: bool,
    model_picker_open: bool,
    provider_failures: Vec<ProviderModelsFailure>,
    _subscriptions: Vec<Subscription>,
}

impl ModelSelect {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.entity().downgrade();
        let on_confirm = std::rc::Rc::new(
            move |model: ModelOption, window: &mut Window, cx: &mut App| {
                let _ = state.update(cx, |select, cx| {
                    select.select_model(model.into_model(), window, cx);
                });
            },
        );
        let state = cx.entity().downgrade();
        let on_cancel = std::rc::Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = state.update(cx, |select, cx| {
                select.close_model_picker(false, window, cx);
            });
        });
        let empty_label = cx.global::<I18n>().t("empty-model-picker");
        let model_picker = cx.new(|cx| {
            ListState::new(
                PickerListDelegate::new(
                    Vec::new(),
                    false,
                    empty_label.into(),
                    on_confirm.clone(),
                    on_cancel.clone(),
                ),
                window,
                cx,
            )
            .searchable(true)
        });

        let mut this = Self {
            models: Vec::new(),
            selected_model: None,
            model_picker,
            model_picker_bounds: Bounds::default(),
            model_picker_loading: false,
            model_picker_open: false,
            provider_failures: Vec::new(),
            _subscriptions: Vec::new(),
        };
        this.bind_store_events(window, cx);
        this.sync_models_from_store(window, cx, false);
        this
    }

    pub(crate) fn selected_model(&self) -> Option<ProviderModel> {
        self.selected_model.clone()
    }

    pub(crate) fn restore_selected_model(
        &mut self,
        provider_name: &str,
        model_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ProviderModel> {
        let model = self
            .models
            .iter()
            .find(|model| model.provider_name == provider_name && model.id == model_id)
            .cloned()?;
        self.selected_model = Some(model.clone());
        let sections = model_sections(&self.models);
        let selected_ix = PickerListDelegate::selected_index_for(&sections, Some(&model));
        self.model_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.set_selected_index(selected_ix, window, cx);
        });
        cx.notify();
        Some(model)
    }

    fn bind_store_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let store_subscription = cx.observe_in(
            &cx.global::<ModelStore>().deref().clone(),
            window,
            |this, _, window, cx| {
                this.sync_models_from_store(window, cx, false);
            },
        );
        let config_subscription =
            cx.observe_global_in::<AiChatConfig>(window, |this, window, cx| {
                let selection_changed = this.sync_selection_with_config(cx);
                this.sync_models_from_store(window, cx, selection_changed);
            });
        self._subscriptions.push(store_subscription);
        self._subscriptions.push(config_subscription);
    }

    fn reload_models(&mut self, cx: &mut Context<Self>) {
        crate::state::chat::reload_models(cx);
    }

    fn model_store_snapshot(&self, cx: &App) -> ModelStoreSnapshot {
        cx.global::<ModelStore>().read(cx).snapshot()
    }

    fn sync_models_from_store(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        force_selection_sync: bool,
    ) {
        let ModelStoreSnapshot {
            models,
            status,
            failures,
        } = self.model_store_snapshot(cx);
        let models_changed = self.models != models;
        if models_changed {
            self.models = models;
        }

        let keep_selected = self.selected_model.as_ref().is_some_and(|selected| {
            self.models.iter().any(|model| {
                model.provider_name == selected.provider_name && model.id == selected.id
            })
        });
        let mut selection_changed = force_selection_sync;
        if !keep_selected && self.selected_model.take().is_some() {
            selection_changed = true;
            cx.emit(ModelSelectEvent::Change(None));
        }

        let next_loading = model_store_is_loading(status);
        let loading_changed = self.model_picker_loading != next_loading;
        let should_notify_failures = should_notify_model_load_failures(
            &self.provider_failures,
            &failures,
            self.model_picker_loading,
            next_loading,
        );
        self.provider_failures = failures;
        if loading_changed {
            self.model_picker_loading = next_loading;
        }

        let picker_selection = (models_changed || selection_changed).then(|| {
            let sections = model_sections(&self.models);
            let selected_ix =
                PickerListDelegate::selected_index_for(&sections, self.selected_model.as_ref());
            (sections, selected_ix)
        });

        if picker_selection.is_some() || loading_changed {
            self.model_picker.update(cx, |picker, cx| {
                if let Some((sections, selected_ix)) = picker_selection {
                    picker.delegate_mut().set_sections(sections);
                    picker.set_selected_index(selected_ix, window, cx);
                }
                if loading_changed {
                    picker.delegate_mut().set_loading(next_loading);
                }
            });
            if models_changed {
                cx.emit(ModelSelectEvent::ModelsChanged);
            }
            cx.notify();
        }
        if should_notify_failures {
            notify_model_load_failures(&self.provider_failures, window, cx);
        }
    }

    fn sync_selection_with_config(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(selected_model) = self.selected_model.as_ref() else {
            return false;
        };
        let configured =
            provider_is_configured(cx.global::<AiChatConfig>(), &selected_model.provider_name)
                .unwrap_or(false);
        if !configured && self.selected_model.take().is_some() {
            cx.emit(ModelSelectEvent::Change(None));
            return true;
        }
        false
    }

    fn open_model_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_picker_open = true;
        self.focus_model_picker(window, cx);
        cx.notify();
    }

    fn close_model_picker(
        &mut self,
        _focus_self: bool,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.model_picker_open = false;
        cx.notify();
    }

    fn focus_model_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_picker
            .update(cx, |picker, cx| picker.focus(window, cx));
    }

    fn select_model(&mut self, model: ProviderModel, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_model = Some(model.clone());
        self.close_model_picker(true, window, cx);
        cx.emit(ModelSelectEvent::Change(Some(model)));
        cx.notify();
    }

    fn render_model_picker(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bounds = self.model_picker_bounds;
        let search_label = cx.global::<I18n>().t("field-search-models");
        let reload_tooltip = cx.global::<I18n>().t("button-reload");
        let configure_label = cx.global::<I18n>().t("button-configure");
        let is_loading = self.model_picker_loading;
        let on_mouse_down_out = cx.listener(|select, ev: &MouseDownEvent, window, cx| {
            if select.model_picker_bounds.contains(&ev.position) {
                return;
            }
            select.close_model_picker(false, window, cx);
        });

        render_picker_popover(
            bounds,
            self.model_picker.clone(),
            PickerPopoverOptions {
                min_width: Some(px(220.)),
                search_placeholder: Some(search_label.into()),
                footer: Some(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .pb_2()
                        .pt_1()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .child(
                            Button::new("model-picker-configure")
                                .icon(IconName::Plug)
                                .label(configure_label)
                                .small()
                                .flex_1()
                                .on_click(cx.listener(|select, _event, window, cx| {
                                    cx.stop_propagation();
                                    select.close_model_picker(false, window, cx);
                                    open_provider_settings_window(cx);
                                })),
                        )
                        .child(
                            Button::new("model-picker-reload")
                                .icon(IconName::RefreshCcw)
                                .ghost()
                                .small()
                                .tooltip(reload_tooltip)
                                .disabled(is_loading)
                                .on_click(cx.listener(|select, _event, _window, cx| {
                                    cx.stop_propagation();
                                    select.reload_models(cx);
                                })),
                        )
                        .into_any_element(),
                ),
                ..Default::default()
            },
            on_mouse_down_out,
            cx,
        )
    }
}

impl Render for ModelSelect {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_open = self.model_picker_open;
        let title: SharedString = self
            .selected_model
            .as_ref()
            .map(|model| model.id.clone().into())
            .unwrap_or_else(|| cx.global::<I18n>().t("field-models").into());

        div()
            .child(
                PickerTrigger::new(
                    "model-picker-button",
                    title,
                    cx.listener(|select, _event, window, cx| {
                        if select.model_picker_open {
                            select.close_model_picker(false, window, cx);
                        } else {
                            select.open_model_picker(window, cx);
                        }
                    }),
                    {
                        let state = cx.entity();
                        move |bounds, cx| {
                            state.update(cx, |select, cx| {
                                if select.model_picker_bounds != bounds {
                                    select.model_picker_bounds = bounds;
                                    cx.notify();
                                }
                            })
                        }
                    },
                )
                .selected(false)
                .open(is_open),
            )
            .when(self.model_picker_open, |this| {
                this.child(self.render_model_picker(window, cx))
            })
    }
}

fn model_store_is_loading(status: Option<ModelStoreStatus>) -> bool {
    matches!(
        status,
        Some(ModelStoreStatus::InitialLoading | ModelStoreStatus::Refreshing)
    )
}

fn should_notify_model_load_failures(
    previous: &[ProviderModelsFailure],
    current: &[ProviderModelsFailure],
    was_loading: bool,
    is_loading: bool,
) -> bool {
    !is_loading && !current.is_empty() && (previous != current || was_loading)
}

fn notify_model_load_failures(
    failures: &[ProviderModelsFailure],
    window: &mut Window,
    cx: &mut App,
) {
    window.push_notification(
        Notification::new()
            .title(cx.global::<I18n>().t("notify-load-models-partial-failed"))
            .message(format_failure_message(failures))
            .with_type(NotificationType::Error),
        cx,
    );
}

fn format_failure_message(failures: &[ProviderModelsFailure]) -> String {
    failures
        .iter()
        .map(|failure| format!("{}: {}", failure.provider_name, failure.message))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        format_failure_message, model_store_is_loading, should_notify_model_load_failures,
    };
    use crate::{llm::ProviderModelsFailure, state::ModelStoreStatus};

    fn failure(provider: &str, message: &str) -> ProviderModelsFailure {
        ProviderModelsFailure {
            provider_name: provider.to_string(),
            message: message.to_string(),
        }
    }

    #[test]
    fn model_store_loading_status_matches_refresh_states() {
        assert!(model_store_is_loading(Some(
            ModelStoreStatus::InitialLoading
        )));
        assert!(model_store_is_loading(Some(ModelStoreStatus::Refreshing)));
        assert!(!model_store_is_loading(Some(ModelStoreStatus::Idle)));
        assert!(!model_store_is_loading(None));
    }

    #[test]
    fn model_load_failures_notify_when_new_or_reload_finishes() {
        let previous = [failure("OpenAI", "api key missing")];
        let current = [failure("OpenAI", "api key missing")];
        let changed = [failure("Ollama", "request failed")];

        assert!(!should_notify_model_load_failures(
            &[],
            &current,
            false,
            true
        ));
        assert!(should_notify_model_load_failures(
            &[],
            &current,
            false,
            false
        ));
        assert!(!should_notify_model_load_failures(
            &previous, &current, false, false
        ));
        assert!(should_notify_model_load_failures(
            &previous, &current, true, false
        ));
        assert!(should_notify_model_load_failures(
            &previous, &changed, false, false
        ));
        assert!(!should_notify_model_load_failures(
            &previous,
            &[],
            true,
            false
        ));
    }

    #[test]
    fn format_failure_message_lists_each_provider_on_its_own_line() {
        let message = format_failure_message(&[
            failure("Ollama", "request failed"),
            failure("OpenAI", "api key missing"),
        ]);

        assert_eq!(message, "Ollama: request failed\nOpenAI: api key missing");
    }
}
