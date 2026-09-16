use super::{AppConfig, AppLanguage, ConfigController, PiProbeController, SettingsView};
use gpui_form::Form;
use gpui_kit::component::Root;
use gpui_kit::{AppContext, TestAppContext};

#[gpui_kit::test]
fn shared_settings_layout_renders_without_reentrant_entity_access(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        app_theme::init(cx);
        crate::state::theme::init(cx);
        crate::foundation::i18n::apply(AppLanguage::Chinese, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let form = cx.new(|_| Form::new(AppConfig::default()));
        let controller = cx.new(|cx| ConfigController::new(&form, cx));
        let draft = cx.new(|_| PiProbeController::new());
        let applied = cx.new(|_| PiProbeController::new());
        let view = cx.new(|cx| {
            SettingsView::new(
                form,
                controller,
                draft,
                applied,
                cx.focus_handle(),
                window,
                cx,
            )
        });
        Root::new(view, window, cx)
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
}
