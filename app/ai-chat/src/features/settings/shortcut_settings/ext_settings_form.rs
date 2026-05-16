use crate::{
    components::ext_setting_help,
    foundation::i18n::I18n,
    llm::{ExtSettingControl, ExtSettingItem, ExtSettingOption},
};
use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ElementExt, IndexPath, Sizable, StyledExt, h_flex,
    label::Label,
    select::{Select, SelectEvent, SelectItem, SelectState},
    switch::Switch,
    v_flex,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ShortcutExtSettingsEvent {
    Change(ExtSettingItem),
}

impl EventEmitter<ShortcutExtSettingsEvent> for ShortcutExtSettingsForm {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ShortcutExtSettingOption {
    value: String,
    label: SharedString,
}

impl ShortcutExtSettingOption {
    fn new(option: ExtSettingOption, cx: &App) -> Self {
        Self {
            value: option.value.to_string(),
            label: cx.global::<I18n>().t(option.label_key).into(),
        }
    }
}

impl SelectItem for ShortcutExtSettingOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

struct SelectSettingState {
    item: ExtSettingItem,
    raw_options: Vec<ExtSettingOption>,
    select: Entity<SelectState<Vec<ShortcutExtSettingOption>>>,
}

enum ExtSettingState {
    Select(SelectSettingState),
    Boolean(ExtSettingItem),
}

pub(super) struct ShortcutExtSettingsForm {
    settings: Vec<ExtSettingState>,
    help_open_index: Option<usize>,
    help_positions: Vec<Point<Pixels>>,
    _subscriptions: Vec<Subscription>,
}

impl ShortcutExtSettingsForm {
    pub(super) fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            settings: Vec::new(),
            help_open_index: None,
            help_positions: Vec::new(),
            _subscriptions: Vec::new(),
        }
    }

    pub(super) fn clear(&mut self, cx: &mut Context<Self>) {
        self.settings.clear();
        self.help_open_index = None;
        self.help_positions.clear();
        self._subscriptions.clear();
        cx.notify();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.settings.is_empty()
    }

    pub(super) fn set_items(
        &mut self,
        items: Vec<ExtSettingItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut previous = std::mem::take(&mut self.settings);
        self.settings = items
            .into_iter()
            .map(|item| {
                let previous_index = previous.iter().position(|state| state.can_reuse_for(&item));
                let previous_state = previous_index.map(|index| previous.remove(index));
                self.setting_state_from_item(item, previous_state, window, cx)
            })
            .collect();
        self.help_open_index = None;
        self.help_positions
            .resize(self.settings.len(), Point::default());
        self.bind_subscriptions(window, cx);
        cx.notify();
    }

    fn setting_state_from_item(
        &self,
        item: ExtSettingItem,
        previous_state: Option<ExtSettingState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ExtSettingState {
        match item.control.clone() {
            ExtSettingControl::Select { value, options } => {
                let raw_options = options.clone();
                let options = options
                    .into_iter()
                    .map(|option| ShortcutExtSettingOption::new(option, cx))
                    .collect::<Vec<_>>();
                let selected_index = selected_option_index(&options, &value);

                if let Some(ExtSettingState::Select(mut previous)) = previous_state {
                    previous.item = item;
                    previous.raw_options = raw_options;
                    previous.select.update(cx, |select, cx| {
                        select.set_items(options, window, cx);
                        select.set_selected_index(selected_index, window, cx);
                    });
                    return ExtSettingState::Select(previous);
                }

                let select = cx.new(|cx| SelectState::new(options, selected_index, window, cx));
                ExtSettingState::Select(SelectSettingState {
                    item,
                    raw_options,
                    select,
                })
            }
            ExtSettingControl::Boolean(_) => ExtSettingState::Boolean(item),
        }
    }

    fn bind_subscriptions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._subscriptions.clear();
        for setting in &self.settings {
            let ExtSettingState::Select(setting) = setting else {
                continue;
            };
            let key = setting.item.key;
            self._subscriptions.push(cx.subscribe_in(
                &setting.select,
                window,
                move |this,
                      _select,
                      event: &SelectEvent<Vec<ShortcutExtSettingOption>>,
                      _window,
                      cx| {
                    let SelectEvent::Confirm(Some(value)) = event else {
                        return;
                    };
                    this.select_value(key, value.clone(), cx);
                },
            ));
        }
    }

    fn select_value(&mut self, key: &'static str, value: String, cx: &mut Context<Self>) {
        let Some(ExtSettingState::Select(setting)) = self
            .settings
            .iter_mut()
            .find(|setting| matches!(setting, ExtSettingState::Select(setting) if setting.item.key == key))
        else {
            return;
        };
        setting.item.control = ExtSettingControl::Select {
            value,
            options: setting.raw_options.clone(),
        };
        cx.emit(ShortcutExtSettingsEvent::Change(setting.item.clone()));
        cx.notify();
    }

    fn set_boolean_value(&mut self, key: &'static str, value: bool) -> Option<ExtSettingItem> {
        let Some(ExtSettingState::Boolean(setting)) = self.settings.iter_mut().find(
            |setting| matches!(setting, ExtSettingState::Boolean(setting) if setting.key == key),
        ) else {
            return None;
        };
        let ExtSettingControl::Boolean(current) = &mut setting.control else {
            return None;
        };
        *current = value;
        Some(setting.clone())
    }

    #[cfg(test)]
    fn has_tooltip(item: &ExtSettingItem) -> bool {
        item.tooltip.is_some()
    }

    fn set_help_position(&mut self, index: usize, bounds: Bounds<Pixels>) {
        if self.help_positions.len() <= index {
            self.help_positions.resize(index + 1, Point::default());
        }
        self.help_positions[index] = ext_setting_help::help_position(bounds);
    }

    fn show_help(&mut self, index: usize, cx: &mut Context<Self>) {
        self.help_open_index = Some(index);
        cx.notify();
    }

    fn hide_help(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.help_open_index == Some(index) {
            self.help_open_index = None;
            cx.notify();
        }
    }

    fn render_label(
        &self,
        item: &ExtSettingItem,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = h_flex().items_center().gap_1().child(
            Label::new(cx.global::<I18n>().t(item.label_key))
                .text_sm()
                .font_medium(),
        );
        let Some(tooltip_key) = item.tooltip else {
            return label.into_any_element();
        };

        let state = cx.entity();
        let icon = ext_setting_help::help_icon(
            SharedString::from(format!("shortcut-ext-setting-help-trigger-{index}")),
            cx,
        )
        .on_hover(cx.listener(move |settings, hovered: &bool, _window, cx| {
            if *hovered {
                settings.show_help(index, cx);
            }
        }))
        .on_prepaint(move |bounds, _window, cx| {
            state.update(cx, |settings, _cx| {
                settings.set_help_position(index, bounds);
            });
        });
        let position = self.help_positions.get(index).copied().unwrap_or_default();

        label
            .child(
                div()
                    .child(icon)
                    .when(self.help_open_index == Some(index), |this| {
                        this.child(ext_setting_help::help_panel(
                            SharedString::from(format!("shortcut-ext-setting-help-panel-{index}")),
                            tooltip_key,
                            position,
                            cx.listener(move |settings, hovered: &bool, _window, cx| {
                                if *hovered {
                                    settings.show_help(index, cx);
                                } else {
                                    settings.hide_help(index, cx);
                                }
                            }),
                            window,
                            cx,
                        ))
                    }),
            )
            .into_any_element()
    }

    fn render_boolean(
        &self,
        setting: &ExtSettingItem,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ExtSettingControl::Boolean(value) = setting.control else {
            unreachable!("boolean setting state always carries boolean control");
        };
        let setting_key = setting.key;
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .child(self.render_label(setting, index, window, cx))
            .child(
                h_flex().w_full().justify_start().child(
                    Switch::new(setting.key)
                        .checked(value)
                        .small()
                        .on_click(cx.listener(move |settings, checked, _window, cx| {
                            if let Some(item) = settings.set_boolean_value(setting_key, *checked) {
                                cx.emit(ShortcutExtSettingsEvent::Change(item));
                            }
                            cx.notify();
                        })),
                ),
            )
            .into_any_element()
    }

    fn render_select(
        &self,
        setting: &SelectSettingState,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        v_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .child(self.render_label(&setting.item, index, window, cx))
            .child(Select::new(&setting.select).w_full())
            .into_any_element()
    }
}

impl Render for ShortcutExtSettingsForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let controls = self
            .settings
            .iter()
            .enumerate()
            .map(|(index, setting)| match setting {
                ExtSettingState::Boolean(setting) => {
                    self.render_boolean(setting, index, window, cx)
                }
                ExtSettingState::Select(setting) => self.render_select(setting, index, window, cx),
            })
            .collect::<Vec<_>>();

        if controls.is_empty() {
            return div();
        }

        v_flex().w_full().gap_3().children(controls)
    }
}

impl ExtSettingState {
    fn can_reuse_for(&self, item: &ExtSettingItem) -> bool {
        match (self, &item.control) {
            (Self::Boolean(current), ExtSettingControl::Boolean(_)) => {
                current.key == item.key
                    && current.label_key == item.label_key
                    && current.tooltip == item.tooltip
            }
            (
                Self::Select(current),
                ExtSettingControl::Select {
                    options: new_options,
                    ..
                },
            ) => {
                current.item.key == item.key
                    && current.item.label_key == item.label_key
                    && current.item.tooltip == item.tooltip
                    && current.raw_options == *new_options
            }
            _ => false,
        }
    }
}

fn selected_option_index(options: &[ShortcutExtSettingOption], value: &str) -> Option<IndexPath> {
    options
        .iter()
        .position(|option| option.value == value)
        .map(|index| IndexPath::default().row(index))
}

#[cfg(test)]
mod tests {
    use super::{ShortcutExtSettingOption, ShortcutExtSettingsForm, selected_option_index};
    use crate::llm::{ExtSettingControl, ExtSettingItem};
    use gpui::SharedString;
    use gpui_component::IndexPath;
    use gpui_component::select::SelectItem;

    fn option(value: &'static str, label: &'static str) -> ShortcutExtSettingOption {
        ShortcutExtSettingOption {
            value: value.to_string(),
            label: SharedString::from(label),
        }
    }

    fn setting(tooltip: Option<&'static str>) -> ExtSettingItem {
        ExtSettingItem {
            key: "web_search",
            label_key: "field-web-search",
            tooltip,
            control: ExtSettingControl::Boolean(true),
        }
    }

    #[test]
    fn selected_index_uses_current_select_value() {
        let options = vec![
            option("none", "None"),
            option("low", "Low"),
            option("medium", "Medium"),
        ];

        assert_eq!(
            selected_option_index(&options, "medium"),
            Some(IndexPath::default().row(2))
        );
    }

    #[test]
    fn option_title_uses_display_label() {
        let option = option("high", "High");

        assert_eq!(option.title(), SharedString::from("High"));
    }

    #[test]
    fn help_trigger_is_rendered_only_for_described_settings() {
        assert!(ShortcutExtSettingsForm::has_tooltip(&setting(Some(
            "tooltip-ollama-web-search"
        ))));
        assert!(!ShortcutExtSettingsForm::has_tooltip(&setting(None)));
    }
}
