mod model_picker;

use super::picker::{
    PickerListDelegate, PickerPopoverOptions, PickerTrigger, render_picker_popover,
};
use crate::{
    i18n::I18n,
    llm::{ProviderModel, provider_is_configured},
    state::{AiChatConfig, ModelStore, ModelStoreSnapshot, ModelStoreStatus},
    views::settings::open_provider_settings_window,
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    list::ListState,
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
    model_picker_open: bool,
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
            model_picker_open: false,
            _subscriptions: Vec::new(),
        };
        this.bind_store_events(window, cx);
        this.ensure_models_loaded(window, cx);
        this.sync_models_from_store(window, cx);
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
                this.sync_models_from_store(window, cx);
            },
        );
        let config_subscription =
            cx.observe_global_in::<AiChatConfig>(window, |this, window, cx| {
                this.sync_selection_with_config(cx);
                this.sync_models_from_store(window, cx);
            });
        self._subscriptions.push(store_subscription);
        self._subscriptions.push(config_subscription);
    }

    fn ensure_models_loaded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model_store = cx.global::<ModelStore>().deref().clone();
        model_store.update(cx, |store, cx| store.ensure_loaded(window, cx));
    }

    fn reload_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model_store = cx.global::<ModelStore>().deref().clone();
        model_store.update(cx, |store, cx| store.reload(window, cx));
    }

    fn model_store_snapshot(&self, cx: &App) -> ModelStoreSnapshot {
        cx.global::<ModelStore>().read(cx).snapshot()
    }

    fn sync_models_from_store(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.model_store_snapshot(cx);
        self.models = snapshot.models.clone();

        let keep_selected = self.selected_model.as_ref().is_some_and(|selected| {
            self.models.iter().any(|model| {
                model.provider_name == selected.provider_name && model.id == selected.id
            })
        });
        if !keep_selected && self.selected_model.take().is_some() {
            cx.emit(ModelSelectEvent::Change(None));
        }

        let sections = model_sections(&self.models);
        let selected_ix =
            PickerListDelegate::selected_index_for(&sections, self.selected_model.as_ref());
        let is_loading = matches!(
            snapshot.status,
            Some(ModelStoreStatus::InitialLoading | ModelStoreStatus::Refreshing)
        );
        self.model_picker.update(cx, |picker, cx| {
            picker.delegate_mut().set_sections(sections);
            picker.delegate_mut().set_loading(is_loading);
            picker.set_selected_index(selected_ix, window, cx);
        });
        cx.emit(ModelSelectEvent::ModelsChanged);
        cx.notify();
    }

    fn sync_selection_with_config(&mut self, cx: &mut Context<Self>) {
        let Some(selected_model) = self.selected_model.as_ref() else {
            return;
        };
        let configured =
            provider_is_configured(cx.global::<AiChatConfig>(), &selected_model.provider_name)
                .unwrap_or(false);
        if !configured && self.selected_model.take().is_some() {
            cx.emit(ModelSelectEvent::Change(None));
            cx.notify();
        }
    }

    fn open_model_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_picker_open = true;
        self.ensure_models_loaded(window, cx);
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
        let snapshot = self.model_store_snapshot(cx);
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
                                .icon(IconName::Redo2)
                                .ghost()
                                .small()
                                .tooltip(reload_tooltip)
                                .disabled(matches!(
                                    snapshot.status,
                                    Some(
                                        ModelStoreStatus::InitialLoading
                                            | ModelStoreStatus::Refreshing
                                    )
                                ))
                                .on_click(cx.listener(|select, _event, window, cx| {
                                    cx.stop_propagation();
                                    select.reload_models(window, cx);
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
                            state.update(cx, |select, _| {
                                select.model_picker_bounds = bounds;
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
