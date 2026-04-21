use crate::{
    i18n::I18n,
    llm::{
        ProviderSettingsFieldKind, ProviderSettingsFieldSpec, ProviderSettingsSpec,
        provider_by_name, provider_settings_specs,
    },
    state::AiChatConfig,
};
use gpui::*;
use gpui_component::{
    Sizable,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use tracing::{Level, event};

use super::layout::{SettingsRow, SettingsSection};

struct ProviderSettingsInputState {
    input: Entity<InputState>,
    last_value: String,
    _subscription: Subscription,
}

pub(super) fn render(window: &mut Window, cx: &mut App) -> AnyElement {
    let sections = provider_settings_specs()
        .into_iter()
        .map(|spec| render_provider_section(spec, window, cx))
        .collect::<Vec<_>>();

    v_flex()
        .w_full()
        .gap_4()
        .children(sections)
        .into_any_element()
}

fn render_provider_section(
    spec: ProviderSettingsSpec,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let title = cx.global::<I18n>().t(spec.title_key);
    let rows = spec
        .fields
        .iter()
        .map(|field| render_provider_field(spec.provider_name, *field, window, cx))
        .collect::<Vec<_>>();

    SettingsSection::new(title)
        .children(rows)
        .into_any_element()
}

fn render_provider_field(
    provider_name: &'static str,
    field: ProviderSettingsFieldSpec,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let label = cx.global::<I18n>().t(field.label_key);
    SettingsRow::new(
        label,
        provider_setting_input(provider_name, field, window, cx),
    )
    .into_any_element()
}

fn provider_setting_input(
    provider_name: &'static str,
    field: ProviderSettingsFieldSpec,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let initial_value = read_provider_setting(provider_name, field.key, cx).unwrap_or_default();
    let input_key = format!("provider-setting-{provider_name}-{}", field.key);
    let masked = field.kind == ProviderSettingsFieldKind::SecretText;
    let state = window
        .use_keyed_state(input_key, cx, |window, cx| {
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(initial_value.clone())
                    .masked(masked)
            });
            let _subscription = cx.subscribe_in(&input, window, {
                move |state: &mut ProviderSettingsInputState,
                      input,
                      event: &InputEvent,
                      window,
                      cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }

                    let next_value = input.read(cx).value().to_string();
                    if next_value == state.last_value {
                        return;
                    }

                    let write_result = provider_by_name(provider_name).and_then(|provider| {
                        provider.write_settings_field(field.key, next_value.clone(), cx)
                    });
                    if let Err(err) = write_result {
                        event!(
                            Level::ERROR,
                            "Failed to save provider setting {}.{}: {}",
                            provider_name,
                            field.key,
                            err
                        );
                        return;
                    }

                    let saved_value =
                        read_provider_setting(provider_name, field.key, cx).unwrap_or(next_value);
                    if saved_value != input.read(cx).value() {
                        input.update(cx, |input, cx| {
                            input.set_value(saved_value.clone(), window, cx);
                        });
                    }
                    state.last_value = saved_value;
                }
            });

            ProviderSettingsInputState {
                input,
                last_value: initial_value,
                _subscription,
            }
        })
        .read(cx);

    let input = Input::new(&state.input).small().w(px(320.));
    if masked {
        input.mask_toggle().into_any_element()
    } else {
        input.into_any_element()
    }
}

fn read_provider_setting(provider_name: &str, field_key: &str, cx: &App) -> Option<String> {
    let provider = provider_by_name(provider_name).ok()?;
    provider.read_settings_field(field_key, cx.global::<AiChatConfig>())
}
