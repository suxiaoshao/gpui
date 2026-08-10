use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use gpui::{
    AnyElement, App, Context, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState},
    label::Label,
    v_flex,
};
use gpui_form::{Form, FormEvent, ItemPath, PathKey, TotalItemsPath};
use gpui_form_gpui_component::FormInput;

use crate::{
    features::request::draft::{
        ApiKeyLocation, HeaderDraft, RequestAuthDraft, RequestBodyDraft, RequestDraft,
    },
    foundation::{I18n, i18n::validation_message},
};

struct HeaderRow {
    item: ItemPath<RequestDraft, HeaderDraft>,
    name: FormInput,
    value: FormInput,
}

impl HeaderRow {
    fn try_new(
        form: &Entity<Form<RequestDraft>>,
        item: ItemPath<RequestDraft, HeaderDraft>,
        window: &mut Window,
        cx: &mut Context<HttpHeadersView>,
    ) -> Result<Self, gpui_form::ResolveError> {
        let name_placeholder = cx.global::<I18n>().t("field-name");
        let value_placeholder = cx.global::<I18n>().t("field-value");
        let name = FormInput::try_new(
            form,
            item.clone().then(HeaderDraft::NAME),
            move |window, cx| InputState::new(window, cx).placeholder(name_placeholder),
            window,
            cx,
        )?;
        let value = FormInput::try_new(
            form,
            item.clone().then(HeaderDraft::VALUE),
            move |window, cx| InputState::new(window, cx).placeholder(value_placeholder),
            window,
            cx,
        )?;
        Ok(Self { item, name, value })
    }
}

/// Owns native controls for the live Header collection occurrences.
pub(crate) struct HttpHeadersView {
    form: Entity<Form<RequestDraft>>,
    rows: HashMap<PathKey, HeaderRow>,
    order: Vec<PathKey>,
    retry_scheduled: bool,
    retry_attempted: HashSet<PathKey>,
    #[cfg(test)]
    fail_next_bind: bool,
    _subscription: Subscription,
}

impl HttpHeadersView {
    pub(crate) fn new(
        form: Entity<Form<RequestDraft>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe_in(
            &form,
            window,
            |this, _, event: &FormEvent<RequestDraft>, window, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    return;
                };
                let impact = change.impact(&RequestDraft::HEADERS);
                if impact.structure_changed() || impact.retired() {
                    this.retry_attempted.clear();
                    this.reconcile(window, cx);
                    cx.notify();
                }
            },
        );
        let mut view = Self {
            form,
            rows: HashMap::new(),
            order: Vec::new(),
            retry_scheduled: false,
            retry_attempted: HashSet::new(),
            #[cfg(test)]
            fail_next_bind: false,
            _subscription: subscription,
        };
        view.reconcile(window, cx);
        view
    }

    fn reconcile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = RequestDraft::HEADERS.items(&self.form, cx);
        let live = items.iter().map(ItemPath::key).collect::<HashSet<_>>();

        // Retired controls are dropped before any fallible construction. They are never used as a
        // fallback for a newly allocated occurrence with the same business value.
        self.rows.retain(|key, _| live.contains(key));
        self.retry_attempted.retain(|key| live.contains(key));

        let mut failed = Vec::new();
        for item in &items {
            let key = item.key();
            if self.rows.contains_key(&key) {
                continue;
            }
            #[cfg(test)]
            if std::mem::take(&mut self.fail_next_bind) {
                failed.push(key);
                continue;
            }
            match HeaderRow::try_new(&self.form, item.clone(), window, cx) {
                Ok(row) => {
                    self.rows.insert(key, row);
                }
                Err(_) => {
                    tracing::warn!("failed to bind a live HTTP header row");
                    failed.push(key);
                }
            }
        }
        self.order = items
            .into_iter()
            .map(|item| item.key())
            .filter(|key| self.rows.contains_key(key))
            .collect();
        self.schedule_retry(failed, window, cx);
    }

    fn schedule_retry(
        &mut self,
        failed: Vec<PathKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut newly_failed = false;
        for key in failed {
            newly_failed |= self.retry_attempted.insert(key);
        }
        if !newly_failed || self.retry_scheduled {
            return;
        }
        self.retry_scheduled = true;
        cx.defer_in(window, |this, window, cx| {
            this.retry_scheduled = false;
            this.reconcile(window, cx);
            cx.notify();
        });
    }

    fn render_row(&self, key: &PathKey, cx: &mut Context<Self>) -> AnyElement {
        let row = self
            .rows
            .get(key)
            .expect("render order only contains successfully bound header rows");
        let enabled_path = row.item.clone().then(HeaderDraft::ENABLED);
        let name_path = row.item.clone().then(HeaderDraft::NAME);
        let value_path = row.item.clone().then(HeaderDraft::VALUE);
        let enabled = enabled_path.try_get(&self.form, cx).unwrap_or(false);
        let name_errors = visible_first_error(
            enabled,
            &name_path.try_errors(&self.form, cx).unwrap_or_default(),
            cx,
        );
        let value_errors = visible_first_error(
            enabled,
            &value_path.try_errors(&self.form, cx).unwrap_or_default(),
            cx,
        );
        let header_name = name_path.try_get(&self.form, cx).unwrap_or_default();
        let auth = RequestDraft::AUTH.get(&self.form, cx);
        let body = RequestDraft::BODY.get(&self.form, cx);
        let auth_overrides = enabled && auth_overrides_header(&auth, &header_name);
        let content_type_overrides = enabled
            && header_name.eq_ignore_ascii_case("content-type")
            && body_generates_content_type(&body);

        let form = self.form.clone();
        let enabled_for_click = enabled_path.clone();
        let enabled_control = Checkbox::new(child_id("header", key, "enabled"))
            .checked(enabled)
            .on_click(move |checked, _, cx| {
                let _ = enabled_for_click.try_set(&form, *checked, cx);
            });

        let delete_label = cx.global::<I18n>().t("button-delete");
        let delete_form = self.form.clone();
        let delete_item = row.item.clone();
        let delete = Button::new(child_id("header", key, "delete"))
            .label(delete_label)
            .on_click(move |_, _, cx| {
                let _ = headers_path().remove(&delete_form, delete_item.clone(), cx);
            });

        let position = self.order.iter().position(|candidate| candidate == key);
        let move_up = position
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| {
                self.rows
                    .get(&self.order[position])
                    .map(|previous| previous.item.clone())
            });
        let move_down = position
            .and_then(|position| self.order.get(position + 1))
            .and_then(|next| self.rows.get(next))
            .map(|next| next.item.clone());

        let up = move_up.map(|previous| {
            let form = self.form.clone();
            let item = row.item.clone();
            Button::new(child_id("header", key, "up"))
                .label(cx.global::<I18n>().t("button-move-up"))
                .on_click(move |_, _, cx| {
                    let _ = headers_path().move_before(&form, &item, &previous, cx);
                })
        });
        let down = move_down.map(|next| {
            let form = self.form.clone();
            let item = row.item.clone();
            Button::new(child_id("header", key, "down"))
                .label(cx.global::<I18n>().t("button-move-down"))
                .on_click(move |_, _, cx| {
                    let _ = headers_path().move_before(&form, &next, &item, cx);
                })
        });

        let mut override_messages = Vec::new();
        if auth_overrides {
            override_messages.push(cx.global::<I18n>().t("auth-generated-override"));
        }
        if content_type_overrides {
            override_messages.push(cx.global::<I18n>().t("body-content-type-override"));
        }

        v_flex()
            .id(child_id("header", key, "row"))
            .w_full()
            .gap_1()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(enabled_control)
                    .child(input_with_error((*row.name).clone(), name_errors, cx))
                    .child(input_with_error((*row.value).clone(), value_errors, cx))
                    .children(up)
                    .children(down)
                    .child(delete),
            )
            .children(override_messages.into_iter().map(|message| {
                Label::new(message)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
            }))
            .into_any_element()
    }
}

fn auth_overrides_header(auth: &RequestAuthDraft, header_name: &str) -> bool {
    match auth {
        RequestAuthDraft::Basic(_) | RequestAuthDraft::Bearer(_) => {
            header_name.eq_ignore_ascii_case("authorization")
        }
        RequestAuthDraft::ApiKey(api_key) if api_key.location == ApiKeyLocation::Header => {
            header_name.eq_ignore_ascii_case(&api_key.name)
        }
        RequestAuthDraft::None | RequestAuthDraft::ApiKey(_) => false,
    }
}

fn body_generates_content_type(body: &RequestBodyDraft) -> bool {
    matches!(
        body,
        RequestBodyDraft::FormData(_) | RequestBodyDraft::UrlEncoded(_) | RequestBodyDraft::Text(_)
    )
}

impl Render for HttpHeadersView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name_label, value_label, add_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-name"),
                i18n.t("field-value"),
                i18n.t("button-add"),
            )
        };
        let form = self.form.clone();
        let add = Button::new("add-header")
            .label(add_label)
            .on_click(move |_, _, cx| {
                let _ = headers_path().append(&form, HeaderDraft::default(), cx);
            });
        let rows = self
            .order
            .iter()
            .map(|key| self.render_row(key, cx))
            .collect::<Vec<_>>();

        v_flex()
            .flex_1()
            .p_2()
            .gap_2()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(22.)).child(""))
                    .child(div().flex_1().child(Label::new(name_label)))
                    .child(div().flex_1().child(Label::new(value_label)))
                    .child(add),
            )
            .children(rows)
    }
}

fn input_with_error(
    input: Entity<InputState>,
    error: Option<SharedString>,
    cx: &mut App,
) -> AnyElement {
    v_flex()
        .flex_1()
        .gap_1()
        .child(Input::new(&input).w_full())
        .when_some(error, |this, error| {
            this.child(Label::new(error).text_xs().text_color(cx.theme().danger))
        })
        .into_any_element()
}

fn visible_first_error(
    enabled: bool,
    issues: &[gpui_form::ValidationIssue],
    cx: &App,
) -> Option<SharedString> {
    if !enabled {
        return None;
    }
    issues
        .first()
        .map(|issue| validation_message(issue.message(), cx))
}

fn child_id(scope: &'static str, key: &PathKey, role: &'static str) -> ElementId {
    ElementId::NamedChild(
        Arc::new(ElementId::from(key)),
        format!("{scope}-{role}").into(),
    )
}

fn headers_path() -> TotalItemsPath<RequestDraft, HeaderDraft> {
    RequestDraft::ROOT.then(RequestDraft::HEADERS)
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::input::InputEvent;

    use crate::features::request::draft::ApiKeyAuthDraft;
    use crate::features::request::validation::RequestValidator;
    use crate::foundation::i18n::init_i18n;

    use super::*;

    fn open_headers(
        cx: &mut TestAppContext,
    ) -> (Entity<Form<RequestDraft>>, WindowHandle<HttpHeadersView>) {
        cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
            let draft = RequestDraft {
                headers: vec![
                    HeaderDraft {
                        enabled: true,
                        name: "x-first".into(),
                        value: "one".into(),
                    },
                    HeaderDraft {
                        enabled: true,
                        name: "x-second".into(),
                        value: "two".into(),
                    },
                ],
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft).with_validator(RequestValidator));
            let view_form = form.clone();
            let window = cx
                .open_window(Default::default(), move |window, cx| {
                    cx.new(|cx| HttpHeadersView::new(view_form, window, cx))
                })
                .expect("open Headers test window");
            (form, window)
        })
    }

    #[gpui::test]
    fn failed_live_binding_retries_once_and_never_reinstalls_a_retired_key(
        cx: &mut TestAppContext,
    ) {
        let (form, window) = open_headers(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("Headers root");

        let retried = cx.update(|_, cx| headers_path().items(&form, cx)[0].key());
        cx.update(|window, cx| {
            root.update(cx, |view, cx| {
                view.rows.remove(&retried).expect("drop live row binding");
                view.fail_next_bind = true;
                view.reconcile(window, cx);
                assert!(view.retry_scheduled);
                assert!(!view.rows.contains_key(&retried));
            })
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert!(view.rows.contains_key(&retried));
                assert!(!view.retry_scheduled);
            });
        });

        let transient = cx.update(|_, cx| {
            headers_path()
                .append(&form, HeaderDraft::default(), cx)
                .expect("append transient row")
        });
        cx.run_until_parked();
        let retired = transient.key();
        cx.update(|window, cx| {
            root.update(cx, |view, cx| {
                view.rows
                    .remove(&retired)
                    .expect("drop transient row binding");
                view.fail_next_bind = true;
                view.reconcile(window, cx);
                assert!(view.retry_scheduled);
                headers_path()
                    .remove(&view.form, transient.clone(), cx)
                    .expect("retire transient row");
            })
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert!(!view.rows.contains_key(&retired));
                assert!(!view.order.contains(&retired));
                assert!(!view.retry_scheduled);
            });
        });
    }

    #[gpui::test]
    fn disabled_header_hides_a_previous_submit_issue(cx: &mut TestAppContext) {
        let (form, window) = open_headers(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let item = cx.update(|_, cx| headers_path().items(&form, cx)[0].clone());
        let name = item.clone().then(HeaderDraft::NAME);
        let enabled = item.then(HeaderDraft::ENABLED);

        cx.update(|_, cx| {
            name.try_set(&form, "bad header".into(), cx)
                .expect("set invalid header name");
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
            assert!(!name.try_errors(&form, cx).expect("live name").is_empty());
            enabled
                .try_set(&form, false, cx)
                .expect("disable invalid header");
            let stale_issues = name.try_errors(&form, cx).expect("name remains live");
            assert!(
                visible_first_error(false, &stale_issues, cx).is_none(),
                "disabled rows never render old submit issues"
            );
        });
    }

    #[gpui::test]
    fn reorder_preserves_rows_and_removed_input_cannot_write_reinserted_value(
        cx: &mut TestAppContext,
    ) {
        let (form, window) = open_headers(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("Headers root");
        let items = cx.update(|_, cx| headers_path().items(&form, cx));
        let first = items[0].clone();
        let second = items[1].clone();
        let first_key = first.key();
        let second_key = second.key();
        let (first_input, first_entity, second_entity) = cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                let first = view.rows.get(&first_key).expect("first row");
                let second = view.rows.get(&second_key).expect("second row");
                (
                    (*first.name).clone(),
                    first.name.entity_id(),
                    second.name.entity_id(),
                )
            })
        });

        cx.update(|_, cx| {
            headers_path()
                .move_before(&form, &second, &first, cx)
                .expect("reorder headers");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert_eq!(view.order, [second_key.clone(), first_key.clone()]);
                assert_eq!(view.rows[&first_key].name.entity_id(), first_entity);
                assert_eq!(view.rows[&second_key].name.entity_id(), second_entity);
            });
        });

        let replacement = cx.update(|_, cx| {
            let removed = headers_path()
                .remove(&form, first.clone(), cx)
                .expect("remove first occurrence");
            headers_path()
                .append(&form, removed, cx)
                .expect("append replacement occurrence")
        });
        let replacement_key = replacement.key();
        cx.run_until_parked();
        cx.update(|window, cx| {
            root.read_with(cx, |view, _| {
                assert!(!view.rows.contains_key(&first_key));
                assert_ne!(
                    view.rows[&replacement_key].name.entity_id(),
                    first_entity,
                    "remove/reinsert allocates a new native row owner"
                );
            });
            first_input.update(cx, |input, cx| {
                input.set_value("stale", window, cx);
                cx.emit(InputEvent::Change);
            });
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(
                replacement
                    .clone()
                    .then(HeaderDraft::NAME)
                    .try_get(&form, cx)
                    .expect("replacement remains active"),
                "x-first"
            );
        });
    }

    #[test]
    fn generated_override_hints_match_the_compiler_precedence_rules() {
        assert!(!auth_overrides_header(
            &RequestAuthDraft::None,
            "authorization"
        ));
        assert!(auth_overrides_header(
            &RequestAuthDraft::basic(),
            "Authorization"
        ));
        assert!(auth_overrides_header(
            &RequestAuthDraft::bearer(),
            "authorization"
        ));

        let api_key = RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
            name: "X-Api-Key".into(),
            value: "secret".into(),
            location: ApiKeyLocation::Header,
        });
        assert!(auth_overrides_header(&api_key, "x-api-key"));
        assert!(!auth_overrides_header(&api_key, "authorization"));

        let query_key = RequestAuthDraft::ApiKey(ApiKeyAuthDraft {
            name: "key".into(),
            value: "secret".into(),
            location: ApiKeyLocation::Query,
        });
        assert!(!auth_overrides_header(&query_key, "key"));

        assert!(!body_generates_content_type(&RequestBodyDraft::None));
        assert!(!body_generates_content_type(&RequestBodyDraft::binary()));
        assert!(body_generates_content_type(&RequestBodyDraft::text()));
        assert!(body_generates_content_type(&RequestBodyDraft::url_encoded()));
        assert!(body_generates_content_type(&RequestBodyDraft::form_data()));
    }
}
