use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
    sync::Arc,
};

use gpui::{
    AnyElement, App, AppContext as _, Context, ElementId, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    select::{SelectItem, SelectState},
    v_flex,
};
use gpui_form::{
    ControlBinding, ControlProjection, DynamicItemsPath, DynamicPath, Form, FormEvent, ItemPath,
    PathKey, ResolveError,
};
use gpui_form_gpui_component::FormInput;

use crate::{
    features::request::{
        controls::{FormCaseSelect, FormFilePathInput},
        draft::{
            FormDataDraft, MultipartFileDraft, MultipartPartDraft, MultipartPartValueDraft,
            MultipartTextDraft, RequestDraft,
        },
    },
    foundation::{I18n, i18n::validation_message},
};

use super::binary::file_labels;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MultipartKind {
    Text,
    File,
}

#[derive(Clone)]
struct MultipartOption {
    kind: MultipartKind,
    title: SharedString,
}

impl SelectItem for MultipartOption {
    type Value = MultipartKind;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.kind
    }
}

type MultipartOptions = Vec<MultipartOption>;

struct OptionalStringInput {
    state: Entity<InputState>,
    _binding: ControlBinding,
    _subscription: Subscription,
}

impl OptionalStringInput {
    fn try_new(
        form: &Entity<Form<RequestDraft>>,
        path: DynamicPath<RequestDraft, Option<String>>,
        placeholder: SharedString,
        window: &mut Window,
        cx: &mut Context<FormDataView>,
    ) -> Result<Self, ResolveError> {
        let initial = path.try_get(form, cx)?.unwrap_or_default();
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        state.update(cx, |state, cx| state.set_value(initial, window, cx));
        let (binding, writer) = path.try_bind_control_in(
            form,
            &state,
            |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    state.set_value(value.unwrap_or_default(), window, cx);
                }
                ControlProjection::Retired => {}
            },
            window,
            cx,
        )?;
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    let value = state.read(cx).value().to_string();
                    writer.defer_set((!value.is_empty()).then_some(value), window, cx);
                }
                InputEvent::Blur => writer.defer_blur(window, cx),
                InputEvent::Focus | InputEvent::PressEnter { .. } => {}
            },
        );
        Ok(Self {
            state,
            _binding: binding,
            _subscription: subscription,
        })
    }
}

impl Deref for OptionalStringInput {
    type Target = Entity<InputState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

enum MultipartValueControls {
    Text(Box<MultipartTextControls>),
    File(Box<MultipartFileControls>),
    Unavailable,
}

struct MultipartTextControls {
    value: FormInput,
    content_type_path: DynamicPath<RequestDraft, Option<String>>,
    content_type: OptionalStringInput,
}

struct MultipartFileControls {
    path: DynamicPath<RequestDraft, Option<std::path::PathBuf>>,
    file: FormFilePathInput,
}

impl MultipartValueControls {
    fn unavailable(&self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

struct MultipartRow {
    item: ItemPath<RequestDraft, MultipartPartDraft>,
    name: FormInput,
    value_path: DynamicPath<RequestDraft, MultipartPartValueDraft>,
    kind: FormCaseSelect<RequestDraft, MultipartOptions, MultipartPartValueDraft, MultipartKind>,
    active_kind: MultipartKind,
    value: MultipartValueControls,
}

impl MultipartRow {
    fn try_new(
        form: &Entity<Form<RequestDraft>>,
        item: ItemPath<RequestDraft, MultipartPartDraft>,
        window: &mut Window,
        cx: &mut Context<FormDataView>,
    ) -> Result<Self, ResolveError> {
        let name_placeholder = cx.global::<I18n>().t("field-name");
        let name = FormInput::try_new(
            form,
            item.clone().then(MultipartPartDraft::NAME),
            move |window, cx| InputState::new(window, cx).placeholder(name_placeholder),
            window,
            cx,
        )?;
        let value_path = item.clone().then(MultipartPartDraft::VALUE);
        let active_kind = multipart_kind(&value_path.try_get(form, cx)?);
        let options = multipart_options(cx);
        let kind = FormCaseSelect::try_new(
            form,
            value_path.clone(),
            multipart_kind,
            multipart_value_for_kind,
            move |window, cx| SelectState::new(options, None, window, cx),
            window,
            cx,
        )?;
        let value = build_value_controls(form, &value_path, active_kind, window, cx);
        Ok(Self {
            item,
            name,
            value_path,
            kind,
            active_kind,
            value,
        })
    }

    fn reconcile_value(
        &mut self,
        form: &Entity<Form<RequestDraft>>,
        window: &mut Window,
        cx: &mut Context<FormDataView>,
    ) -> bool {
        let Ok(value) = self.value_path.try_get(form, cx) else {
            self.value = MultipartValueControls::Unavailable;
            return true;
        };
        let kind = multipart_kind(&value);
        if kind != self.active_kind || self.value.unavailable() {
            self.active_kind = kind;
            self.value = build_value_controls(form, &self.value_path, kind, window, cx);
        }
        self.value.unavailable()
    }
}

pub(super) struct FormDataView {
    form: Entity<Form<RequestDraft>>,
    parts: DynamicItemsPath<RequestDraft, MultipartPartDraft>,
    rows: HashMap<PathKey, MultipartRow>,
    order: Vec<PathKey>,
    retry_scheduled: bool,
    retry_attempted: HashSet<PathKey>,
    #[cfg(test)]
    fail_next_bind: bool,
    _subscription: Subscription,
}

impl FormDataView {
    pub(super) fn new(
        form: Entity<Form<RequestDraft>>,
        body: DynamicPath<RequestDraft, FormDataDraft>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let parts = body.then(FormDataDraft::PARTS);
        let subscription = cx.subscribe_in(
            &form,
            window,
            |this, _, event: &FormEvent<RequestDraft>, window, cx| {
                let FormEvent::ModelChanged(change) = event else {
                    return;
                };
                let impact = change.impact(&this.parts);
                if impact.structure_changed() || impact.retired() {
                    this.retry_attempted.clear();
                    this.reconcile(window, cx);
                    cx.notify();
                }
            },
        );
        let mut view = Self {
            form,
            parts,
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
        let Ok(items) = self.parts.try_items(&self.form, cx) else {
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
            if !self.rows.contains_key(&key) {
                #[cfg(test)]
                if std::mem::take(&mut self.fail_next_bind) {
                    failed.push(key);
                    continue;
                }
                match MultipartRow::try_new(&self.form, item.clone(), window, cx) {
                    Ok(row) => {
                        self.rows.insert(key, row);
                    }
                    Err(_) => {
                        tracing::warn!("failed to bind a live multipart body row");
                        failed.push(key);
                    }
                }
            }
        }
        for (key, row) in &mut self.rows {
            if row.reconcile_value(&self.form, window, cx) {
                failed.push(key.clone());
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
            .expect("render order only contains successfully bound multipart rows");
        let enabled_path = row.item.clone().then(MultipartPartDraft::ENABLED);
        let name_path = row.item.clone().then(MultipartPartDraft::NAME);
        let enabled = enabled_path.try_get(&self.form, cx).unwrap_or(false);
        let name_error = visible_dynamic_error(enabled, &name_path, &self.form, cx);

        let enabled_form = self.form.clone();
        let enabled_for_click = enabled_path.clone();
        let checkbox = Checkbox::new(child_id("multipart", id, "enabled"))
            .checked(enabled)
            .on_click(move |checked, _, cx| {
                let _ = enabled_for_click.try_set(&enabled_form, *checked, cx);
            });

        let delete_form = self.form.clone();
        let delete_parts = self.parts.clone();
        let delete_item = row.item.clone();
        let delete = Button::new(child_id("multipart", id, "delete"))
            .label(cx.global::<I18n>().t("button-delete"))
            .on_click(move |_, _, cx| {
                let _ = delete_parts.remove(&delete_form, delete_item.clone(), cx);
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
            let parts = self.parts.clone();
            let item = row.item.clone();
            Button::new(child_id("multipart", id, "up"))
                .label(cx.global::<I18n>().t("button-move-up"))
                .on_click(move |_, _, cx| {
                    let _ = parts.move_before(&form, &item, &previous, cx);
                })
        });
        let down = next.map(|next| {
            let form = self.form.clone();
            let parts = self.parts.clone();
            let item = row.item.clone();
            Button::new(child_id("multipart", id, "down"))
                .label(cx.global::<I18n>().t("button-move-down"))
                .on_click(move |_, _, cx| {
                    let _ = parts.move_before(&form, &next, &item, cx);
                })
        });

        v_flex()
            .id(child_id("multipart", id, "row"))
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(checkbox)
                    .child(input_with_error((*row.name).clone(), name_error, cx))
                    .child(div().w(px(140.)).child(row.kind.element()))
                    .children(up)
                    .children(down)
                    .child(delete),
            )
            .child(render_value_controls(&row.value, enabled, &self.form, cx))
            .into_any_element()
    }
}

impl Render for FormDataView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (name, value, add) = {
            let i18n = cx.global::<I18n>();
            (
                i18n.t("field-name"),
                i18n.t("field-value"),
                i18n.t("button-add"),
            )
        };
        let form = self.form.clone();
        let parts = self.parts.clone();
        let add_button = Button::new("add-multipart-part")
            .label(add)
            .on_click(move |_, _, cx| {
                let _ = parts.append(&form, MultipartPartDraft::default(), cx);
            });
        let rows = self
            .order
            .iter()
            .map(|id| self.render_row(id, cx))
            .collect::<Vec<_>>();
        v_flex()
            .flex_1()
            .p_2()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(22.)).child(""))
                    .child(div().flex_1().child(Label::new(name)))
                    .child(div().w(px(140.)).child(Label::new(value)))
                    .child(add_button),
            )
            .children(rows)
    }
}

fn multipart_kind(value: &MultipartPartValueDraft) -> MultipartKind {
    match value {
        MultipartPartValueDraft::Text(_) => MultipartKind::Text,
        MultipartPartValueDraft::File(_) => MultipartKind::File,
    }
}

fn multipart_value_for_kind(kind: MultipartKind) -> MultipartPartValueDraft {
    match kind {
        MultipartKind::Text => MultipartPartValueDraft::text(),
        MultipartKind::File => MultipartPartValueDraft::file(),
    }
}

fn multipart_options(cx: &App) -> MultipartOptions {
    let i18n = cx.global::<I18n>();
    vec![
        MultipartOption {
            kind: MultipartKind::Text,
            title: i18n.t("multipart-text").into(),
        },
        MultipartOption {
            kind: MultipartKind::File,
            title: i18n.t("multipart-file").into(),
        },
    ]
}

fn build_value_controls(
    form: &Entity<Form<RequestDraft>>,
    value_path: &DynamicPath<RequestDraft, MultipartPartValueDraft>,
    kind: MultipartKind,
    window: &mut Window,
    cx: &mut Context<FormDataView>,
) -> MultipartValueControls {
    match kind {
        MultipartKind::Text => {
            let Ok(Some(text)) = value_path
                .clone()
                .case(MultipartPartValueDraft::TEXT)
                .resolve(form, cx)
            else {
                return MultipartValueControls::Unavailable;
            };
            let value = FormInput::try_new(
                form,
                text.clone().then(MultipartTextDraft::VALUE),
                InputState::new,
                window,
                cx,
            );
            let content_type_path = text.then(MultipartTextDraft::CONTENT_TYPE);
            let content_type = OptionalStringInput::try_new(
                form,
                content_type_path.clone(),
                cx.global::<I18n>().t("field-content-type").into(),
                window,
                cx,
            );
            match (value, content_type) {
                (Ok(value), Ok(content_type)) => {
                    MultipartValueControls::Text(Box::new(MultipartTextControls {
                        value,
                        content_type_path,
                        content_type,
                    }))
                }
                _ => MultipartValueControls::Unavailable,
            }
        }
        MultipartKind::File => {
            let Ok(Some(file_path)) = value_path
                .clone()
                .case(MultipartPartValueDraft::FILE)
                .resolve(form, cx)
            else {
                return MultipartValueControls::Unavailable;
            };
            let path = file_path.clone().then(MultipartFileDraft::PATH);
            let file = FormFilePathInput::try_new(form, path.clone(), file_labels(cx), window, cx);
            match file {
                Ok(file) => {
                    MultipartValueControls::File(Box::new(MultipartFileControls { path, file }))
                }
                _ => MultipartValueControls::Unavailable,
            }
        }
    }
}

fn render_value_controls(
    controls: &MultipartValueControls,
    show_errors: bool,
    form: &Entity<Form<RequestDraft>>,
    cx: &mut App,
) -> AnyElement {
    match controls {
        MultipartValueControls::Text(text_controls) => {
            let MultipartTextControls {
                value,
                content_type_path,
                content_type,
            } = text_controls.as_ref();
            h_flex()
                .w_full()
                .gap_2()
                .child(div().flex_1().child(Input::new(value)))
                .child(input_with_error(
                    (**content_type).clone(),
                    visible_dynamic_error(show_errors, content_type_path, form, cx),
                    cx,
                ))
                .into_any_element()
        }
        MultipartValueControls::File(file_controls) => {
            let MultipartFileControls { path, file } = file_controls.as_ref();
            v_flex()
                .w_full()
                .gap_2()
                .child((**file).clone())
                .when_some(
                    visible_dynamic_error(show_errors, path, form, cx),
                    |this, error| {
                        this.child(Label::new(error).text_xs().text_color(cx.theme().danger))
                    },
                )
                .into_any_element()
        }
        MultipartValueControls::Unavailable => div().into_any_element(),
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

fn visible_dynamic_error<T: Clone + PartialEq + 'static>(
    enabled: bool,
    path: &DynamicPath<RequestDraft, T>,
    form: &Entity<Form<RequestDraft>>,
    cx: &App,
) -> Option<SharedString> {
    if !enabled {
        return None;
    }
    path.try_errors(form, cx)
        .unwrap_or_default()
        .first()
        .map(|issue| validation_message(issue.message(), cx))
}

fn child_id(scope: &'static str, key: &PathKey, role: &'static str) -> ElementId {
    ElementId::NamedChild(
        Arc::new(ElementId::from(key)),
        format!("{scope}-{role}").into(),
    )
}

#[cfg(test)]
mod tests {
    use gpui::{TestAppContext, VisualTestContext, WindowHandle};
    use gpui_component::input::InputEvent;

    use crate::{
        features::request::{draft::RequestBodyDraft, validation::RequestValidator},
        foundation::i18n::init_i18n,
    };

    use super::*;

    fn text_part(name: &str, value: &str) -> MultipartPartDraft {
        MultipartPartDraft {
            enabled: true,
            name: name.into(),
            value: MultipartPartValueDraft::Text(MultipartTextDraft {
                value: value.into(),
                content_type: None,
            }),
        }
    }

    fn file_part(name: &str) -> MultipartPartDraft {
        MultipartPartDraft {
            enabled: true,
            name: name.into(),
            value: MultipartPartValueDraft::file(),
        }
    }

    fn open_form_data(
        parts: Vec<MultipartPartDraft>,
        cx: &mut TestAppContext,
    ) -> (Entity<Form<RequestDraft>>, WindowHandle<FormDataView>) {
        cx.update(|cx| {
            gpui_component::init(cx);
            init_i18n(cx);
            let draft = RequestDraft {
                url: "https://example.com".into(),
                body: RequestBodyDraft::FormData(FormDataDraft { parts }),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft).with_validator(RequestValidator));
            let payload = RequestDraft::BODY
                .case(RequestBodyDraft::FORM_DATA)
                .resolve(&form, cx)
                .expect("resolve FormData")
                .expect("FormData is active");
            let view_form = form.clone();
            let window = cx
                .open_window(Default::default(), move |window, cx| {
                    cx.new(|cx| FormDataView::new(view_form, payload, window, cx))
                })
                .expect("open FormData test window");
            (form, window)
        })
    }

    fn form_data_parts(
        form: &Entity<Form<RequestDraft>>,
        cx: &gpui::App,
    ) -> DynamicItemsPath<RequestDraft, MultipartPartDraft> {
        RequestDraft::BODY
            .case(RequestBodyDraft::FORM_DATA)
            .resolve(form, cx)
            .expect("resolve FormData")
            .expect("FormData is active")
            .then(FormDataDraft::PARTS)
    }

    #[gpui::test]
    fn multipart_reorder_and_leaf_or_validation_changes_preserve_native_rows(
        cx: &mut TestAppContext,
    ) {
        let (form, window) = open_form_data(
            vec![text_part("first", "one"), text_part("second", "two")],
            cx,
        );
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("FormData root");
        let (parts, first, second) = cx.update(|_, cx| {
            let parts = form_data_parts(&form, cx);
            let items = parts.try_items(&form, cx).expect("live parts");
            (parts, items[0].clone(), items[1].clone())
        });
        let first_key = first.key();
        let second_key = second.key();
        let (first_name, first_kind, first_value, second_name) = cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                let first = &view.rows[&first_key];
                let second = &view.rows[&second_key];
                let MultipartValueControls::Text(text) = &first.value else {
                    panic!("first part is Text");
                };
                (
                    first.name.entity_id(),
                    first.kind.entity_id(),
                    text.value.entity_id(),
                    second.name.entity_id(),
                )
            })
        });

        cx.update(|_, cx| {
            first
                .clone()
                .then(MultipartPartDraft::NAME)
                .try_set(&form, "renamed".into(), cx)
                .expect("set leaf value");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(
                form.update(cx, |form, cx| form.prepare(cx)).is_ok(),
                "validation-only publication succeeds"
            );
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                let first = &view.rows[&first_key];
                let MultipartValueControls::Text(text) = &first.value else {
                    panic!("first part remains Text");
                };
                assert_eq!(first.name.entity_id(), first_name);
                assert_eq!(first.kind.entity_id(), first_kind);
                assert_eq!(text.value.entity_id(), first_value);
            });
        });

        cx.update(|_, cx| {
            parts
                .move_before(&form, &second, &first, cx)
                .expect("reorder multipart parts");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert_eq!(view.order, [second_key.clone(), first_key.clone()]);
                assert_eq!(view.rows[&first_key].name.entity_id(), first_name);
                assert_eq!(view.rows[&second_key].name.entity_id(), second_name);
            });
        });
    }

    #[gpui::test]
    fn multipart_remove_reinsert_and_text_file_case_retire_stale_writers(cx: &mut TestAppContext) {
        let (form, window) = open_form_data(vec![text_part("field", "keep")], cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("FormData root");
        let (parts, original) = cx.update(|_, cx| {
            let parts = form_data_parts(&form, cx);
            let original = parts.try_items(&form, cx).expect("live parts")[0].clone();
            (parts, original)
        });
        let original_key = original.key();
        let (stale_name, original_name_entity) = cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                let row = &view.rows[&original_key];
                ((*row.name).clone(), row.name.entity_id())
            })
        });

        let replacement = cx.update(|_, cx| {
            let value = parts
                .remove(&form, original.clone(), cx)
                .expect("remove original part");
            parts
                .append(&form, value, cx)
                .expect("append replacement occurrence")
        });
        let replacement_key = replacement.key();
        cx.run_until_parked();
        let stale_text = cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert!(!view.rows.contains_key(&original_key));
                assert_ne!(
                    view.rows[&replacement_key].name.entity_id(),
                    original_name_entity
                );
                let MultipartValueControls::Text(text) = &view.rows[&replacement_key].value else {
                    panic!("replacement starts as Text");
                };
                (*text.value).clone()
            })
        });

        cx.update(|window, cx| {
            stale_name.update(cx, |state, cx| {
                state.set_value("stale occurrence", window, cx);
                cx.emit(InputEvent::Change);
            });
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert_eq!(
                replacement
                    .clone()
                    .then(MultipartPartDraft::NAME)
                    .try_get(&form, cx)
                    .expect("replacement name"),
                "field"
            );
        });

        let value_path = replacement.clone().then(MultipartPartDraft::VALUE);
        let old_text_path = cx.update(|_, cx| {
            value_path
                .clone()
                .case(MultipartPartValueDraft::TEXT)
                .resolve(&form, cx)
                .expect("resolve Text")
                .expect("Text active")
                .then(MultipartTextDraft::VALUE)
        });
        cx.update(|_, cx| {
            value_path
                .try_set(&form, MultipartPartValueDraft::file(), cx)
                .expect("switch part to File");
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(matches!(
                old_text_path.try_get(&form, cx),
                Err(ResolveError::Retired { .. })
            ));
            root.read_with(cx, |view, _| {
                let row = &view.rows[&replacement_key];
                assert_eq!(row.active_kind, MultipartKind::File);
                assert!(matches!(row.value, MultipartValueControls::File(_)));
            });
        });
        cx.update(|window, cx| {
            stale_text.update(cx, |state, cx| {
                state.set_value("retired Text writer", window, cx);
                cx.emit(InputEvent::Change);
            });
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            assert!(matches!(
                value_path.try_get(&form, cx).expect("live value"),
                MultipartPartValueDraft::File(_)
            ));
        });
    }

    #[gpui::test]
    fn deleted_multipart_part_cancels_picker_and_late_completion_is_a_noop(
        cx: &mut TestAppContext,
    ) {
        let (form, window) = open_form_data(vec![file_part("upload")], cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("FormData root");
        let (parts, original) = cx.update(|_, cx| {
            let parts = form_data_parts(&form, cx);
            let original = parts.try_items(&form, cx).expect("live parts")[0].clone();
            (parts, original)
        });
        let original_key = original.key();

        cx.update(|_, cx| {
            root.update(cx, |view, cx| {
                let MultipartValueControls::File(file) = &view.rows[&original_key].value else {
                    panic!("part is File");
                };
                file.file.test_begin_select(cx);
            });
        });
        assert!(cx.did_prompt_for_paths());
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                let MultipartValueControls::File(file) = &view.rows[&original_key].value else {
                    panic!("part is File");
                };
                assert!(file.file.test_has_picker_task());
            });
        });

        let replacement = cx.update(|window, cx| {
            let replacement = parts
                .append(
                    &form,
                    parts
                        .remove(&form, original.clone(), cx)
                        .expect("remove picker owner"),
                    cx,
                )
                .expect("append fresh occurrence");
            root.update(cx, |view, cx| view.reconcile(window, cx));
            replacement
        });
        let replacement_key = replacement.key();
        cx.simulate_path_prompt_response(|_| {
            Some(vec![std::env::temp_dir().join("late-multipart-picker.bin")])
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            let path = replacement
                .clone()
                .then(MultipartPartDraft::VALUE)
                .case(MultipartPartValueDraft::FILE)
                .resolve(&form, cx)
                .expect("resolve replacement File")
                .expect("replacement File active")
                .then(MultipartFileDraft::PATH);
            assert_eq!(path.try_get(&form, cx).expect("replacement path"), None);
            root.read_with(cx, |view, _| {
                assert!(!view.rows.contains_key(&original_key));
                assert!(view.rows.contains_key(&replacement_key));
            });
        });
    }

    #[gpui::test]
    fn multipart_failed_binding_retries_live_key_but_not_a_retired_key(cx: &mut TestAppContext) {
        let (form, window) = open_form_data(vec![text_part("field", "value")], cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("FormData root");
        let (parts, item) = cx.update(|_, cx| {
            let parts = form_data_parts(&form, cx);
            let item = parts.try_items(&form, cx).expect("live parts")[0].clone();
            (parts, item)
        });
        let key = item.key();

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
            root.read_with(cx, |view, _| assert!(view.rows.contains_key(&key)));
        });

        cx.update(|window, cx| {
            root.update(cx, |view, cx| {
                view.rows.remove(&key).expect("drop live row binding again");
                view.retry_attempted.clear();
                view.fail_next_bind = true;
                view.reconcile(window, cx);
                assert!(view.retry_scheduled);
                parts
                    .remove(&view.form, item.clone(), cx)
                    .expect("retire failed row");
            });
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            root.read_with(cx, |view, _| {
                assert!(!view.rows.contains_key(&key));
                assert!(!view.order.contains(&key));
                assert!(!view.retry_scheduled);
            });
        });
    }

    #[gpui::test]
    fn disabled_multipart_part_hides_previous_submit_issues(cx: &mut TestAppContext) {
        let (form, window) = open_form_data(vec![text_part("", "value")], cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let item = cx.update(|_, cx| {
            form_data_parts(&form, cx)
                .try_items(&form, cx)
                .expect("live parts")[0]
                .clone()
        });
        let name = item.clone().then(MultipartPartDraft::NAME);
        let enabled = item.then(MultipartPartDraft::ENABLED);
        cx.update(|_, cx| {
            assert!(form.update(cx, |form, cx| form.prepare(cx)).is_err());
            assert!(!name.try_errors(&form, cx).expect("live name").is_empty());
            enabled
                .try_set(&form, false, cx)
                .expect("disable multipart part");
            let stale_issues = name.try_errors(&form, cx).expect("name remains live");
            assert!(visible_dynamic_error(false, &name, &form, cx).is_none());
            assert!(
                !stale_issues.is_empty(),
                "the renderer is hiding an old issue"
            );
        });
    }
}
