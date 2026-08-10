use gpui::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Styled, Subscription, Window,
};
use gpui_component::{
    Disableable as _,
    checkbox::Checkbox,
    form::{field, v_form},
};
use gpui_form::Form;
use gpui_form_gpui_component::{IntegerInput, IntegerInputEvent, IntegerInputState};

use super::draft::{HttpClientTransportSettings, RequestDraft, RequestSettingsDraft};
use crate::foundation::I18n;

pub(super) struct SettingsView {
    form: Entity<Form<RequestDraft>>,
    timeout: Entity<IntegerInputState<u64>>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    pub(super) fn new(
        form: Entity<Form<RequestDraft>>,
        transport_settings: HttpClientTransportSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let timeout_value = transport_settings.timeout_ms();
        let timeout = cx.new(|cx| {
            let mut state = IntegerInputState::new(window, cx);
            state.set_value(timeout_value, window, cx);
            state
        });
        let timeout_settings = transport_settings.clone();
        let timeout_subscription = cx.subscribe_in(
            &timeout,
            window,
            move |_, _, event: &IntegerInputEvent<u64>, _, cx| {
                if let IntegerInputEvent::Change(Ok(value)) = event {
                    timeout_settings.set_timeout_ms(*value);
                    cx.notify();
                }
            },
        );
        let form_observer = cx.observe(&form, |_, _, cx| cx.notify());
        Self {
            form,
            timeout,
            _subscriptions: vec![timeout_subscription, form_observer],
        }
    }
}

impl gpui::Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = RequestDraft::ROOT.then(RequestDraft::SETTINGS);
        let follow_path = settings
            .clone()
            .then(RequestSettingsDraft::FOLLOW_REDIRECTS);
        let preserve_path = settings.then(RequestSettingsDraft::FOLLOW_ORIGINAL_METHOD);
        let follow = follow_path.get(&self.form, cx);
        let preserve = preserve_path.get(&self.form, cx);
        let (follow_label, preserve_label, timeout_label, timeout_help) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("settings-follow-redirects"),
                i18n.t("settings-follow-original-method"),
                i18n.t("field-timeout-ms"),
                i18n.t("settings-timeout-help"),
            )
        };

        let follow_form = self.form.clone();
        let preserve_form = self.form.clone();
        v_form()
            .p_3()
            .child(
                field().label_indent(false).child(
                    Checkbox::new("request-follow-redirects")
                        .label(follow_label)
                        .checked(follow)
                        .on_click(cx.listener(move |_, checked, _, cx| {
                            follow_path.set(&follow_form, *checked, cx);
                        })),
                ),
            )
            .child(
                field().label_indent(false).child(
                    Checkbox::new("request-preserve-redirect-method")
                        .label(preserve_label)
                        .checked(preserve)
                        .disabled(!follow)
                        .on_click(cx.listener(move |_, checked, _, cx| {
                            preserve_path.set(&preserve_form, *checked, cx);
                        })),
                ),
            )
            .child(
                field()
                    .label(timeout_label)
                    .description(timeout_help)
                    .child(IntegerInput::new(&self.timeout)),
            )
    }
}
