use super::ChatForm;
use crate::{
    components::{
        model_picker::provider_visual_for_model_choice,
        picker::{PickerPopoverConfig, picker_popover, picker_trigger_with_icon},
    },
    features::settings,
    foundation::{
        self, I18n,
        assets::{IconName, provider_visual_icon},
    },
};
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
};

impl ChatForm {
    pub(super) fn render_model_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_model_label = self.selected_model_label(cx.global::<foundation::I18n>());
        let selected_model_icon = self
            .selected_model_choice()
            .map(|choice| provider_visual_icon(provider_visual_for_model_choice(choice)))
            .unwrap_or_else(|| Icon::new(IconName::Sparkles))
            .size_4()
            .text_color(cx.theme().muted_foreground)
            .into_any_element();
        let search_placeholder = cx
            .global::<foundation::I18n>()
            .t("chat-form-model-search-placeholder")
            .into();
        let footer = (!self.has_model_choices()).then(|| self.render_model_picker_footer(cx));

        picker_popover(
            cx,
            PickerPopoverConfig {
                id: "chat-form-model-popover",
                open: self.model_picker_open,
                trigger: picker_trigger_with_icon(
                    "chat-form-model-trigger",
                    selected_model_icon,
                    selected_model_label,
                    self.model_picker_open,
                ),
                list: self.model_picker.clone(),
                width: px(340.),
                max_height: rems(18.).into(),
                search_placeholder: Some(search_placeholder),
                footer,
                on_open_change: cx.listener(|form, open: &bool, window, cx| {
                    form.set_model_picker_open(*open, window, cx);
                }),
            },
        )
    }

    fn render_model_picker_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .border_t_1()
            .border_color(cx.theme().border)
            .p_1()
            .child(
                Button::new("chat-form-configure-providers")
                    .ghost()
                    .icon(IconName::Settings)
                    .label(cx.global::<I18n>().t("chat-form-configure-providers"))
                    .small()
                    .w_full()
                    .on_click(|_, _window, cx| {
                        settings::open_settings_window_to_provider(cx);
                    }),
            )
            .into_any_element()
    }
}
