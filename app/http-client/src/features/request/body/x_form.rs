use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use gpui::{
    AnyElement, Context, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};
use gpui_component::{
    button::Button,
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState},
    label::Label,
    v_flex,
};
use gpui_form::{DynamicItemsPath, DynamicPath, Form, FormEvent, ItemPath, PathKey};
use gpui_form_gpui_component::FormInput;

use crate::{
    features::request::draft::{KeyValueDraft, RequestDraft, UrlEncodedBodyDraft},
    foundation::I18n,
};

struct UrlEncodedRow {
    item: ItemPath<RequestDraft, KeyValueDraft>,
    key: FormInput,
    value: FormInput,
}

impl UrlEncodedRow {
    fn try_new(
        form: &Entity<Form<RequestDraft>>,
        item: ItemPath<RequestDraft, KeyValueDraft>,
        window: &mut Window,
        cx: &mut Context<UrlEncodedView>,
    ) -> Result<Self, gpui_form::ResolveError> {
        let key_placeholder = cx.global::<I18n>().t("field-key");
        let value_placeholder = cx.global::<I18n>().t("field-value");
        let key = FormInput::try_new(
            form,
            item.clone().then(KeyValueDraft::KEY),
            move |window, cx| InputState::new(window, cx).placeholder(key_placeholder),
            window,
            cx,
        )?;
        let value = FormInput::try_new(
            form,
            item.clone().then(KeyValueDraft::VALUE),
            move |window, cx| InputState::new(window, cx).placeholder(value_placeholder),
            window,
            cx,
        )?;
        Ok(Self { item, key, value })
    }
}

pub(super) struct UrlEncodedView {
    form: Entity<Form<RequestDraft>>,
    fields: DynamicItemsPath<RequestDraft, KeyValueDraft>,
    rows: HashMap<PathKey, UrlEncodedRow>,
    order: Vec<PathKey>,
    retry_scheduled: bool,
    retry_attempted: HashSet<PathKey>,
    #[cfg(test)]
    fail_next_bind: bool,
    _subscription: Subscription,
}

impl UrlEncodedView {
    pub(super) fn new(
        form: Entity<Form<RequestDraft>>,
        body: DynamicPath<RequestDraft, UrlEncodedBodyDraft>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let fields = body.then(UrlEncodedBodyDraft::FIELDS);
        let subscription = cx.subscribe_in(
            &form,
            window,
            |this, _, event: &FormEvent<RequestDraft>, window, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    return;
                };
                let impact = change.impact(&this.fields);
                if impact.structure_changed() || impact.retired() {
                    this.retry_attempted.clear();
                    this.reconcile(window, cx);
                    cx.notify();
                }
            },
        );
        let mut view = Self {
            form,
            fields,
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
        let Ok(items) = self.fields.try_items(&self.form, cx) else {
            self.rows.clear();
            self.order.clear();
            self.retry_attempted.clear();
            return;
        };
        let live = items.iter().map(ItemPath::key).collect::<HashSet<_>>();
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
            match UrlEncodedRow::try_new(&self.form, item.clone(), window, cx) {
                Ok(row) => {
                    self.rows.insert(key, row);
                }
                Err(_) => {
                    tracing::warn!("failed to bind a live URL-encoded body row");
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

    fn render_row(&self, id: &PathKey, cx: &mut Context<Self>) -> AnyElement {
        let row = self
            .rows
            .get(id)
            .expect("render order only contains successfully bound URL-encoded rows");
        let enabled_path = row.item.clone().then(KeyValueDraft::ENABLED);
        let enabled = enabled_path.try_get(&self.form, cx).unwrap_or(false);
        let form = self.form.clone();
        let enabled_for_click = enabled_path.clone();
        let checkbox = Checkbox::new(child_id("urlencoded", id, "enabled"))
            .checked(enabled)
            .on_click(move |checked, _, cx| {
                let _ = enabled_for_click.try_set(&form, *checked, cx);
            });

        let delete_form = self.form.clone();
        let fields = self.fields.clone();
        let delete_item = row.item.clone();
        let delete = Button::new(child_id("urlencoded", id, "delete"))
            .label(cx.global::<I18n>().t("button-delete"))
            .on_click(move |_, _, cx| {
                let _ = fields.remove(&delete_form, delete_item.clone(), cx);
            });

        let position = self.order.iter().position(|candidate| candidate == id);
        let previous = position
            .and_then(|position| position.checked_sub(1))
            .and_then(|position| self.rows.get(&self.order[position]))
            .map(|row| row.item.clone());
        let next = position
            .and_then(|position| self.order.get(position + 1))
            .and_then(|key| self.rows.get(key))
            .map(|row| row.item.clone());
        let up = previous.map(|previous| {
            let form = self.form.clone();
            let fields = self.fields.clone();
            let item = row.item.clone();
            Button::new(child_id("urlencoded", id, "up"))
                .label(cx.global::<I18n>().t("button-move-up"))
                .on_click(move |_, _, cx| {
                    let _ = fields.move_before(&form, &item, &previous, cx);
                })
        });
        let down = next.map(|next| {
            let form = self.form.clone();
            let fields = self.fields.clone();
            let item = row.item.clone();
            Button::new(child_id("urlencoded", id, "down"))
                .label(cx.global::<I18n>().t("button-move-down"))
                .on_click(move |_, _, cx| {
                    let _ = fields.move_before(&form, &next, &item, cx);
                })
        });

        h_flex()
            .id(child_id("urlencoded", id, "row"))
            .w_full()
            .items_center()
            .gap_2()
            .child(checkbox)
            .child(div().flex_1().child(Input::new(&row.key)))
            .child(div().flex_1().child(Input::new(&row.value)))
            .children(up)
            .children(down)
            .child(delete)
            .into_any_element()
    }
}

impl Render for UrlEncodedView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (key_label, value_label, add_label) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-key"),
                i18n.t("field-value"),
                i18n.t("button-add"),
            )
        };
        let form = self.form.clone();
        let fields = self.fields.clone();
        let add = Button::new("add-urlencoded-field")
            .label(add_label)
            .on_click(move |_, _, cx| {
                let _ = fields.append(&form, KeyValueDraft::default(), cx);
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
                    .child(div().flex_1().child(Label::new(key_label)))
                    .child(div().flex_1().child(Label::new(value_label)))
                    .child(add),
            )
            .children(rows)
    }
}

fn child_id(scope: &'static str, key: &PathKey, role: &'static str) -> ElementId {
    ElementId::NamedChild(
        Arc::new(ElementId::from(key)),
        format!("{scope}-{role}").into(),
    )
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, TestAppContext, VisualTestContext};

    use crate::{features::request::draft::RequestBodyDraft, foundation::i18n::init_i18n};

    use super::*;

    #[gpui::test]
    fn same_parent_reorder_preserves_urlencoded_native_rows(cx: &mut TestAppContext) {
        let (form, window) = cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
            let draft = RequestDraft {
                body: RequestBodyDraft::UrlEncoded(UrlEncodedBodyDraft {
                    fields: vec![
                        KeyValueDraft {
                            enabled: true,
                            key: "first".into(),
                            value: "one".into(),
                        },
                        KeyValueDraft {
                            enabled: true,
                            key: "second".into(),
                            value: "two".into(),
                        },
                    ],
                }),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft));
            let payload = RequestDraft::BODY
                .case(RequestBodyDraft::URL_ENCODED)
                .resolve(&form, cx)
                .expect("resolve UrlEncoded")
                .expect("UrlEncoded is active");
            let fields = payload.clone().then(UrlEncodedBodyDraft::FIELDS);
            let view_form = form.clone();
            let window = cx
                .open_window(Default::default(), move |window, cx| {
                    cx.new(|cx| UrlEncodedView::new(view_form, payload, window, cx))
                })
                .expect("open UrlEncoded test window");
            (form, (window, fields))
        });
        let (window, fields) = window;
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("UrlEncoded root");
        let items = cx.update(|_, cx| fields.try_items(&form, cx).expect("active fields"));
        let first = items[0].clone();
        let second = items[1].clone();
        let first_key = first.key();
        let second_key = second.key();
        let (first_entity, second_entity) = cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                (
                    view.rows[&first_key].key.entity_id(),
                    view.rows[&second_key].key.entity_id(),
                )
            })
        });

        cx.update(|_, cx| {
            fields
                .move_before(&form, &second, &first, cx)
                .expect("reorder URL-encoded fields");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert_eq!(view.order, [second_key.clone(), first_key.clone()]);
                assert_eq!(view.rows[&first_key].key.entity_id(), first_entity);
                assert_eq!(view.rows[&second_key].key.entity_id(), second_entity);
            });
        });
    }

    #[gpui::test]
    fn failed_urlencoded_binding_is_retried_once_from_the_live_path_key(cx: &mut TestAppContext) {
        let (form, window) = cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
            let draft = RequestDraft {
                body: RequestBodyDraft::UrlEncoded(UrlEncodedBodyDraft {
                    fields: vec![KeyValueDraft::default()],
                }),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft));
            let payload = RequestDraft::BODY
                .case(RequestBodyDraft::URL_ENCODED)
                .resolve(&form, cx)
                .expect("resolve UrlEncoded")
                .expect("UrlEncoded is active");
            let view_form = form.clone();
            let window = cx
                .open_window(Default::default(), move |window, cx| {
                    cx.new(|cx| UrlEncodedView::new(view_form, payload, window, cx))
                })
                .expect("open UrlEncoded test window");
            (form, window)
        });
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("UrlEncoded root");
        let key = cx.update(|_, cx| {
            let payload = RequestDraft::BODY
                .case(RequestBodyDraft::URL_ENCODED)
                .resolve(&form, cx)
                .expect("resolve UrlEncoded")
                .expect("UrlEncoded is active");
            payload
                .then(UrlEncodedBodyDraft::FIELDS)
                .try_items(&form, cx)
                .expect("live fields")[0]
                .key()
        });

        cx.update(|window, cx| {
            root.update(cx, |view, cx| {
                view.rows.remove(&key).expect("drop live row binding");
                view.fail_next_bind = true;
                view.reconcile(window, cx);
                assert!(view.retry_scheduled);
                assert!(!view.rows.contains_key(&key));
            });
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert!(view.rows.contains_key(&key));
                assert!(!view.retry_scheduled);
            });
        });
    }
}
