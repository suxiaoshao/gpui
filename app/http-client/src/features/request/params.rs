use std::{mem, ops::Deref};

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, ParentElement as _, SharedString, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    v_flex,
};
use gpui_form::{ControlBinding, ControlProjection, Form};
use url::Url;

use super::draft::RequestDraft;
use crate::foundation::I18n;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ParamRowId(u64);

struct ParamRow {
    id: ParamRowId,
    key: Entity<InputState>,
    _key_subscription: Subscription,
    value: Entity<InputState>,
    _value_subscription: Subscription,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParamsMode {
    Editable,
    DisabledInvalidUrl,
}

struct ParamSnapshot {
    id: ParamRowId,
    key: String,
    value: String,
}

enum HttpParamsEvent {
    Change(String),
}

pub(super) struct HttpParamsState {
    mode: ParamsMode,
    retired: bool,
    last_valid_url: Option<Url>,
    rows: Vec<ParamRow>,
    next_row_id: u64,
}

impl HttpParamsState {
    fn new(raw_url: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state = Self {
            mode: ParamsMode::DisabledInvalidUrl,
            retired: false,
            last_valid_url: None,
            rows: Vec::new(),
            next_row_id: 0,
        };
        state.project_url_silently(raw_url, window, cx);
        state
    }

    fn can_edit(&self) -> bool {
        !self.retired && self.mode == ParamsMode::Editable
    }

    pub(super) fn has_decoded_key(&self, key: &str) -> bool {
        self.can_edit()
            && self.last_valid_url.as_ref().is_some_and(|url| {
                url.query_pairs()
                    .any(|(candidate, _)| candidate.as_ref() == key)
            })
    }

    fn allocate_row_id(&mut self) -> ParamRowId {
        let id = ParamRowId(self.next_row_id);
        self.next_row_id = self
            .next_row_id
            .checked_add(1)
            .expect("HTTP Params row identity exhausted");
        id
    }

    fn create_row(
        &mut self,
        id: ParamRowId,
        key: String,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ParamRow {
        let (key_placeholder, value_placeholder) = {
            let i18n = cx.global::<I18n>();
            (i18n.t("field-key"), i18n.t("field-value"))
        };
        let key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(key)
                .placeholder(key_placeholder)
        });
        let value_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder(value_placeholder)
        });

        let key_subscription = cx.subscribe_in(
            &key_input,
            window,
            move |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.publish_current_url(cx),
                InputEvent::PressEnter { .. } => {
                    if let Some(row) = this.rows.iter().find(|row| row.id == id) {
                        row.value.update(cx, |input, cx| input.focus(window, cx));
                    }
                }
                InputEvent::Focus | InputEvent::Blur => {}
            },
        );
        let value_subscription = cx.subscribe_in(
            &value_input,
            window,
            move |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.publish_current_url(cx),
                InputEvent::PressEnter { .. } => {
                    let next = this
                        .rows
                        .iter()
                        .position(|row| row.id == id)
                        .and_then(|index| this.rows.get(index + 1))
                        .or_else(|| this.rows.first());
                    if let Some(row) = next {
                        row.key.update(cx, |input, cx| input.focus(window, cx));
                    }
                }
                InputEvent::Focus | InputEvent::Blur => {}
            },
        );

        ParamRow {
            id,
            key: key_input,
            _key_subscription: key_subscription,
            value: value_input,
            _value_subscription: value_subscription,
        }
    }

    fn snapshots(&self, cx: &gpui::App) -> Vec<ParamSnapshot> {
        self.rows
            .iter()
            .map(|row| ParamSnapshot {
                id: row.id,
                key: row.key.read(cx).value().to_string(),
                value: row.value.read(cx).value().to_string(),
            })
            .collect()
    }

    fn project_url_silently(
        &mut self,
        raw_url: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(url) = parse_params_url(&raw_url) else {
            self.mode = ParamsMode::DisabledInvalidUrl;
            cx.notify();
            return;
        };
        let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        let old = self.snapshots(cx);
        let target_ids = reconcile_row_ids(&old, &pairs, &mut self.next_row_id);
        let mut old_rows = mem::take(&mut self.rows);
        let mut next_rows = Vec::with_capacity(pairs.len());

        for (id, (key, value)) in target_ids.into_iter().zip(pairs) {
            if let Some(index) = old_rows.iter().position(|row| row.id == id) {
                next_rows.push(old_rows.remove(index));
            } else {
                next_rows.push(self.create_row(id, key, value, window, cx));
            }
        }

        self.rows = next_rows;
        self.last_valid_url = Some(url);
        self.mode = ParamsMode::Editable;
        self.retired = false;
        cx.notify();
    }

    fn retire(&mut self, cx: &mut Context<Self>) {
        self.retired = true;
        self.mode = ParamsMode::DisabledInvalidUrl;
        cx.notify();
    }

    fn publish_current_url(&mut self, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let Some(mut url) = self.last_valid_url.clone() else {
            return;
        };
        let pairs: Vec<(String, String)> = self
            .snapshots(cx)
            .into_iter()
            .map(|row| (row.key, row.value))
            .collect();
        if pairs.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(pairs.iter());
        }
        self.last_valid_url = Some(url.clone());
        cx.emit(HttpParamsEvent::Change(url.to_string()));
    }

    fn add_param(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let id = self.allocate_row_id();
        let row = self.create_row(id, String::new(), String::new(), window, cx);
        self.rows.push(row);
        self.publish_current_url(cx);
        cx.notify();
    }

    fn remove_param(&mut self, id: ParamRowId, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let Some(index) = self.rows.iter().position(|row| row.id == id) else {
            return;
        };
        self.rows.remove(index);
        self.publish_current_url(cx);
        cx.notify();
    }

    fn move_param(&mut self, id: ParamRowId, offset: isize, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let Some(index) = self.rows.iter().position(|row| row.id == id) else {
            return;
        };
        let target = index.saturating_add_signed(offset).min(self.rows.len() - 1);
        if target == index {
            return;
        }
        let row = self.rows.remove(index);
        self.rows.insert(target, row);
        self.publish_current_url(cx);
        cx.notify();
    }
}

impl EventEmitter<HttpParamsEvent> for HttpParamsState {}

impl gpui::Render for HttpParamsState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let disabled = !self.can_edit();
        let (key_label, value_label, add_label, delete_label, up_label, down_label, invalid_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-key"),
                i18n.t("field-value"),
                i18n.t("button-add"),
                i18n.t("button-delete"),
                i18n.t("button-move-up"),
                i18n.t("button-move-down"),
                i18n.t("params-invalid-url-disabled"),
            )
        };

        v_flex()
            .p_2()
            .gap_2()
            .when(disabled, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(invalid_label),
                )
            })
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Label::new(key_label)))
                    .child(div().flex_1().child(Label::new(value_label)))
                    .child(
                        Button::new("request-params-add")
                            .label(add_label)
                            .disabled(disabled)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_param(window, cx)),
                            ),
                    ),
            )
            .children(self.rows.iter().map(|row| {
                let id = row.id;
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.key).disabled(disabled)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&row.value).disabled(disabled)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("request-param-up-{}", id.0)))
                            .label(up_label.clone())
                            .disabled(disabled)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.move_param(id, -1, cx)),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("request-param-down-{}", id.0)))
                            .label(down_label.clone())
                            .disabled(disabled)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.move_param(id, 1, cx)),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("request-param-delete-{}", id.0)))
                            .label(delete_label.clone())
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, _, cx| this.remove_param(id, cx))),
                    )
            }))
    }
}

pub(super) struct HttpParamsInput {
    state: Entity<HttpParamsState>,
    _binding: ControlBinding,
    _native_subscription: Subscription,
}

impl HttpParamsInput {
    pub(super) fn new<Owner>(
        form: &Entity<Form<RequestDraft>>,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Owner: 'static,
    {
        let initial = RequestDraft::URL.get(form, cx);
        let state = cx.new(|cx| HttpParamsState::new(initial, window, cx));
        let (binding, writer) = RequestDraft::URL.bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(url) => state.project_url_silently(url, window, cx),
                ControlProjection::Retired => state.retire(cx),
            },
            window,
            cx,
        );
        let native_subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &HttpParamsEvent, window, cx| {
                let HttpParamsEvent::Change(url) = event;
                writer.defer_set(url.clone(), window, cx);
            },
        );
        Self {
            state,
            _binding: binding,
            _native_subscription: native_subscription,
        }
    }

    pub(super) fn state(&self) -> &Entity<HttpParamsState> {
        &self.state
    }
}

impl Deref for HttpParamsInput {
    type Target = Entity<HttpParamsState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

fn parse_params_url(raw: &str) -> Option<Url> {
    let url = Url::parse(raw.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    Some(url)
}

fn reconcile_row_ids(
    old: &[ParamSnapshot],
    next: &[(String, String)],
    next_row_id: &mut u64,
) -> Vec<ParamRowId> {
    let mut consumed = vec![false; old.len()];
    next.iter()
        .map(|(key, value)| {
            if let Some((index, row)) = old
                .iter()
                .enumerate()
                .find(|(index, row)| !consumed[*index] && row.key == *key && row.value == *value)
            {
                consumed[index] = true;
                return row.id;
            }

            let id = ParamRowId(*next_row_id);
            *next_row_id = next_row_id
                .checked_add(1)
                .expect("HTTP Params row identity exhausted");
            id
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use gpui::{IntoElement, Render, TestAppContext, VisualTestContext, WindowHandle, div};

    use super::super::url_input::UrlInput;
    use super::*;
    use crate::foundation::i18n::init_i18n;

    struct ParamsHarness {
        form: Entity<Form<RequestDraft>>,
        url_state: Entity<InputState>,
        _url: UrlInput,
        params: HttpParamsInput,
    }

    impl ParamsHarness {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let draft = RequestDraft {
                url: "https://example.com/path?tag=rust".into(),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft));
            let url = UrlInput::new(&form, window, cx);
            let url_state = (*url).clone();
            let params = HttpParamsInput::new(&form, window, cx);
            Self {
                form,
                url_state,
                _url: url,
                params,
            }
        }
    }

    impl Render for ParamsHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn open_harness(cx: &mut TestAppContext) -> WindowHandle<ParamsHarness> {
        cx.update(|cx| {
            init_i18n(cx);
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| ParamsHarness::new(window, cx))
            })
            .expect("open Params test window")
        })
    }

    fn harness_entities(
        root: &Entity<ParamsHarness>,
        cx: &mut VisualTestContext,
    ) -> (
        Entity<Form<RequestDraft>>,
        Entity<InputState>,
        Entity<HttpParamsState>,
    ) {
        cx.update(|_, cx| {
            root.read_with(cx, |root, _| {
                (
                    root.form.clone(),
                    root.url_state.clone(),
                    root.params.state().clone(),
                )
            })
        })
    }

    fn first_row(
        params: &Entity<HttpParamsState>,
        cx: &mut VisualTestContext,
    ) -> (ParamRowId, Entity<InputState>, Entity<InputState>) {
        cx.update(|_, cx| {
            let params = params.read(cx);
            let row = params.rows.first().expect("one Params row");
            (row.id, row.key.clone(), row.value.clone())
        })
    }

    fn set_input(input: &Entity<InputState>, value: &str, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            input.update(cx, |input, cx| {
                input.set_value(value, window, cx);
                cx.emit(InputEvent::Change);
            });
        });
        cx.run_until_parked();
    }

    fn snapshot(id: u64, key: &str, value: &str) -> ParamSnapshot {
        ParamSnapshot {
            id: ParamRowId(id),
            key: key.into(),
            value: value.into(),
        }
    }

    #[test]
    fn params_url_requires_absolute_http_or_https() {
        assert!(parse_params_url("https://example.com/path?a=1").is_some());
        assert!(parse_params_url(" http://localhost:3000/path ").is_some());
        assert!(parse_params_url("/relative").is_none());
        assert!(parse_params_url("ftp://example.com/file").is_none());
        assert!(parse_params_url("").is_none());
    }

    #[test]
    fn reconcile_preserves_duplicate_occurrences_in_old_order() {
        let old = vec![
            snapshot(10, "tag", "rust"),
            snapshot(11, "tag", "rust"),
            snapshot(12, "page", "1"),
        ];
        let next = vec![
            ("tag".into(), "rust".into()),
            ("page".into(), "1".into()),
            ("tag".into(), "rust".into()),
            ("new".into(), String::new()),
        ];
        let mut next_id = 13;

        let ids = reconcile_row_ids(&old, &next, &mut next_id);

        assert!(
            ids == vec![
                ParamRowId(10),
                ParamRowId(12),
                ParamRowId(11),
                ParamRowId(13)
            ]
        );
        assert_eq!(next_id, 14);
    }

    #[test]
    fn reconcile_does_not_reuse_changed_or_removed_rows() {
        let old = vec![snapshot(4, "a", "1"), snapshot(5, "b", "2")];
        let next = vec![("a".into(), "changed".into()), ("b".into(), "2".into())];
        let mut next_id = 6;

        let ids = reconcile_row_ids(&old, &next, &mut next_id);

        assert!(ids == vec![ParamRowId(6), ParamRowId(5)]);
    }

    #[gpui::test]
    fn url_input_and_params_are_source_aware_peers(cx: &mut TestAppContext) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("Params harness root");
        let (form, url_input, params) = harness_entities(&root, &mut cx);
        let (initial_id, key, _) = first_row(&params, &mut cx);

        set_input(&key, "topic", &mut cx);

        cx.update(|_, cx| {
            assert_eq!(
                RequestDraft::URL.get(&form, cx),
                "https://example.com/path?topic=rust"
            );
            assert_eq!(
                url_input.read(cx).value().as_ref(),
                "https://example.com/path?topic=rust"
            );
            let params = params.read(cx);
            assert_eq!(params.rows.len(), 1);
            assert!(params.rows[0].id == initial_id);
        });

        set_input(
            &url_input,
            "https://example.com/path?page=2&page=3",
            &mut cx,
        );

        cx.update(|_, cx| {
            assert_eq!(
                RequestDraft::URL.get(&form, cx),
                "https://example.com/path?page=2&page=3"
            );
            let params = params.read(cx);
            assert!(params.mode == ParamsMode::Editable);
            assert_eq!(params.rows.len(), 2);
            assert!(params.rows.iter().all(|row| row.id != initial_id));
            assert_eq!(params.rows[0].key.read(cx).value().as_ref(), "page");
            assert_eq!(params.rows[0].value.read(cx).value().as_ref(), "2");
            assert_eq!(params.rows[1].key.read(cx).value().as_ref(), "page");
            assert_eq!(params.rows[1].value.read(cx).value().as_ref(), "3");
        });
    }

    #[gpui::test]
    fn deleting_the_only_added_param_removes_the_query_delimiter(cx: &mut TestAppContext) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("Params harness root");
        let (form, _, params) = harness_entities(&root, &mut cx);

        cx.update(|_, cx| {
            RequestDraft::URL.set(&form, "https://example.com/path".into(), cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert!(params.read(cx).rows.is_empty());
            params.update(cx, |params, cx| params.add_param(window, cx));
        });
        cx.run_until_parked();

        let (id, key, value) = first_row(&params, &mut cx);
        set_input(&key, "only", &mut cx);
        set_input(&value, "one", &mut cx);
        cx.update(|_, cx| {
            assert_eq!(
                RequestDraft::URL.get(&form, cx),
                "https://example.com/path?only=one"
            );
            params.update(cx, |params, cx| params.remove_param(id, cx));
        });
        cx.run_until_parked();

        cx.update(|_, cx| {
            let raw = RequestDraft::URL.get(&form, cx);
            assert_eq!(raw, "https://example.com/path");
            assert!(!raw.ends_with('?'));
            assert!(Url::parse(&raw).expect("valid URL").query().is_none());
        });
    }

    #[gpui::test]
    fn decoded_key_lookup_is_read_only_and_disabled_for_invalid_url(cx: &mut TestAppContext) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("Params harness root");
        let (form, _, params) = harness_entities(&root, &mut cx);

        let encoded = "https://example.com/path?%74ag=rust";
        cx.update(|_, cx| RequestDraft::URL.set(&form, encoded.into(), cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(params.read(cx).has_decoded_key("tag"));
            assert!(!params.read(cx).has_decoded_key("%74ag"));
            assert_eq!(RequestDraft::URL.get(&form, cx), encoded);
        });

        cx.update(|_, cx| RequestDraft::URL.set(&form, "not a URL".into(), cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(!params.read(cx).has_decoded_key("tag"));
            assert_eq!(RequestDraft::URL.get(&form, cx), "not a URL");
        });
    }

    #[gpui::test]
    fn invalid_projection_preserves_rows_and_native_edit_is_the_only_normalizer(
        cx: &mut TestAppContext,
    ) {
        let window = open_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("Params harness root");
        let (form, _, params) = harness_entities(&root, &mut cx);
        let (initial_id, _, initial_value) = first_row(&params, &mut cx);

        cx.update(|_, cx| {
            RequestDraft::URL.set(&form, "not a URL".into(), cx);
        });
        cx.run_until_parked();

        cx.update(|window, cx| {
            let params_before = params.read(cx);
            assert!(params_before.mode == ParamsMode::DisabledInvalidUrl);
            assert_eq!(params_before.rows.len(), 1);
            assert!(params_before.rows[0].id == initial_id);
            params.update(cx, |params, cx| params.add_param(window, cx));
            assert_eq!(RequestDraft::URL.get(&form, cx), "not a URL");
        });

        let projected = " https://example.com/path?tag=rust ";
        cx.update(|_, cx| {
            RequestDraft::URL.set(&form, projected.into(), cx);
        });
        cx.run_until_parked();

        cx.update(|_, cx| {
            assert_eq!(RequestDraft::URL.get(&form, cx), projected);
            let params = params.read(cx);
            assert!(params.mode == ParamsMode::Editable);
            assert_eq!(params.rows.len(), 1);
            assert!(params.rows[0].id == initial_id);
        });

        set_input(&initial_value, "rust tools", &mut cx);

        let mut expected = Url::parse(projected.trim()).expect("valid projected URL");
        expected
            .query_pairs_mut()
            .clear()
            .append_pair("tag", "rust tools");
        cx.update(|_, cx| {
            assert_eq!(RequestDraft::URL.get(&form, cx), expected.to_string());
            let params = params.read(cx);
            assert!(params.mode == ParamsMode::Editable);
            assert!(params.rows[0].id == initial_id);
        });
    }
}
