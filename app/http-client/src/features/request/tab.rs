use fluent_bundle::FluentArgs;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, IntoElement, ParentElement, Styled, Window,
    div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _,
    label::Label,
    scroll::ScrollableElement as _,
    tab::{Tab, TabBar},
    v_flex,
};

use super::{
    auth::AuthView,
    body::HttpBodyView,
    draft::{ApiKeyLocation, HttpClientTransportSettings, RequestAuthDraft, RequestDraft},
    headers::HttpHeadersView,
    params::HttpParamsInput,
    settings::SettingsView,
};
use crate::foundation::I18n;
use gpui_form::Form;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RequestTab {
    #[default]
    Params,
    Authorization,
    Headers,
    Body,
    Settings,
}

impl RequestTab {
    const ALL: [Self; 5] = [
        Self::Params,
        Self::Authorization,
        Self::Headers,
        Self::Body,
        Self::Settings,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Params => 0,
            Self::Authorization => 1,
            Self::Headers => 2,
            Self::Body => 3,
            Self::Settings => 4,
        }
    }
}

pub(super) struct RequestTabsView {
    form: Entity<Form<RequestDraft>>,
    tab: RequestTab,
    params: HttpParamsInput,
    authorization: Entity<AuthView>,
    headers: Entity<HttpHeadersView>,
    body: Entity<HttpBodyView>,
    settings: Entity<SettingsView>,
}

impl RequestTabsView {
    pub(super) fn new(
        form: Entity<Form<RequestDraft>>,
        transport_settings: HttpClientTransportSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let params = HttpParamsInput::new(&form, window, cx);
        let authorization = cx.new(|cx| AuthView::new(form.clone(), window, cx));
        let headers = cx.new(|cx| HttpHeadersView::new(form.clone(), window, cx));
        let body = cx.new(|cx| HttpBodyView::new(form.clone(), window, cx));
        let settings = cx.new(|cx| SettingsView::new(form.clone(), transport_settings, window, cx));
        Self {
            form,
            tab: RequestTab::Params,
            params,
            authorization,
            headers,
            body,
            settings,
        }
    }

    fn content(&self, cx: &mut App) -> AnyElement {
        match self.tab {
            RequestTab::Params => {
                let override_key = api_key_query_override(&self.form, &self.params, cx);
                let override_message = override_key.map(|name| {
                    let mut args = FluentArgs::new();
                    args.set("name", name);
                    cx.global::<I18n>()
                        .t_with_args("auth-query-override", &args)
                });
                v_flex()
                    .when_some(override_message, |this, message| {
                        this.child(
                            Label::new(message)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
                    .child(self.params.state().clone())
                    .into_any_element()
            }
            RequestTab::Authorization => self.authorization.clone().into_any_element(),
            RequestTab::Headers => self.headers.clone().into_any_element(),
            RequestTab::Body => self.body.clone().into_any_element(),
            RequestTab::Settings => self.settings.clone().into_any_element(),
        }
    }
}

impl gpui::Render for RequestTabsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let labels = {
            let i18n = cx.global::<I18n>();
            [
                i18n.t("tab-params"),
                i18n.t("tab-authorization"),
                i18n.t("tab-headers"),
                i18n.t("tab-body"),
                i18n.t("tab-settings"),
            ]
        };
        let content = self.content(cx);
        let content = if self.tab == RequestTab::Body {
            v_flex()
                .flex_1()
                .min_h(px(0.))
                .overflow_hidden()
                .child(content)
                .into_any_element()
        } else {
            div()
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scrollbar()
                .child(content)
                .into_any_element()
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                TabBar::new("request-tabs")
                    .selected_index(self.tab.index())
                    .on_click(cx.listener(|this, index, _, cx| {
                        if let Some(tab) = RequestTab::ALL.get(*index).copied() {
                            this.tab = tab;
                            cx.notify();
                        }
                    }))
                    .children(labels.into_iter().map(|label| Tab::new().label(label))),
            )
            .child(content)
    }
}

fn api_key_query_override(
    form: &Entity<Form<RequestDraft>>,
    params: &HttpParamsInput,
    cx: &App,
) -> Option<String> {
    let RequestAuthDraft::ApiKey(api_key) = RequestDraft::AUTH.get(form, cx) else {
        return None;
    };
    if api_key.location != ApiKeyLocation::Query
        || !params.state().read(cx).has_decoded_key(&api_key.name)
    {
        return None;
    }
    Some(api_key.name)
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;
    use crate::features::request::draft::ApiKeyAuthDraft;
    use crate::foundation::i18n::init_i18n;

    #[test]
    fn tab_indices_are_total_only_for_known_tabs() {
        for (index, tab) in RequestTab::ALL.into_iter().enumerate() {
            assert_eq!(tab.index(), index);
            assert_eq!(RequestTab::ALL.get(index), Some(&tab));
        }
        assert!(RequestTab::ALL.get(usize::MAX).is_none());
    }

    #[gpui::test]
    fn query_override_notice_uses_decoded_keys_and_query_auth_only(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            let draft = RequestDraft {
                url: "https://example.test/?%74ag=old".into(),
                auth: RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
                    name: "tag".into(),
                    value: "new".into(),
                    location: ApiKeyLocation::Query,
                }),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft));
            RequestTabsView::new(form, HttpClientTransportSettings::default(), window, cx)
        });

        cx.update(|_, cx| {
            let view = view.read(cx);
            assert_eq!(
                api_key_query_override(&view.form, &view.params, cx).as_deref(),
                Some("tag")
            );
        });
        cx.update(|_, cx| {
            let form = view.read(cx).form.clone();
            RequestDraft::AUTH.set(
                &form,
                RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
                    name: "tag".into(),
                    value: "new".into(),
                    location: ApiKeyLocation::Header,
                }),
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            let view = view.read(cx);
            assert!(api_key_query_override(&view.form, &view.params, cx).is_none());
        });
    }
}
