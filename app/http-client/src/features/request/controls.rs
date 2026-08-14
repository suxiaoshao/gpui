use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    ops::Deref,
    path::PathBuf,
    rc::Rc,
};

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, ParentElement as _, PathPromptOptions, Render,
    SharedString, Styled as _, Subscription, Task, Window,
};
use gpui_component::{
    Disableable as _,
    button::Button,
    h_flex,
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{Select, SelectEvent, SelectState},
};
use gpui_form::{
    ControlBinding, ControlProjection, DynamicPath, Form, FormSchema, IntoTotalPath, ResolveError,
};

pub(super) struct FormScalarSelect<Root, D, Value>
where
    Root: FormSchema,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Value>,
    Value: Clone + PartialEq + 'static,
{
    state: Entity<SelectState<D>>,
    retired: Rc<Cell<bool>>,
    _binding: ControlBinding,
    _subscription: Subscription,
    _marker: PhantomData<fn() -> (Root, Value)>,
}

impl<Root, D, Value> FormScalarSelect<Root, D, Value>
where
    Root: FormSchema,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Value>,
    Value: Clone + PartialEq + 'static,
{
    pub(super) fn new<Owner, Path, Build>(
        form: &Entity<Form<Root>>,
        path: Path,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Owner: 'static,
        Path: IntoTotalPath<Root, Value>,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let path = path.into_total_path();
        let initial = path.get(form, cx);
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| {
            state.set_selected_value(&initial, window, cx)
        });

        let retired = Rc::new(Cell::new(false));
        let projector_retired = retired.clone();
        let (binding, writer) = path.bind_control_in(
            form,
            &state,
            move |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    projector_retired.set(false);
                    state.set_selected_value(&value, window, cx);
                }
                ControlProjection::Retired => {
                    projector_retired.set(true);
                    cx.notify();
                }
            },
            window,
            cx,
        );

        let callback_retired = retired.clone();
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                if callback_retired.get() {
                    return;
                }
                let SelectEvent::Confirm(value) = event;
                if let Some(value) = value {
                    writer.defer_set(value.clone(), window, cx);
                }
            },
        );

        Self {
            state,
            retired,
            _binding: binding,
            _subscription: subscription,
            _marker: PhantomData,
        }
    }

    pub(super) fn try_new<Owner, Build>(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, Value>,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, ResolveError>
    where
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let initial = path.try_get(form, cx)?;
        let state = cx.new(|cx| build(window, cx));
        state.update(cx, |state, cx| {
            state.set_selected_value(&initial, window, cx)
        });

        let retired = Rc::new(Cell::new(false));
        let projector_retired = retired.clone();
        let (binding, writer) = path.try_bind_control_in(
            form,
            &state,
            move |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    projector_retired.set(false);
                    state.set_selected_value(&value, window, cx);
                }
                ControlProjection::Retired => {
                    projector_retired.set(true);
                    cx.notify();
                }
            },
            window,
            cx,
        )?;

        let callback_retired = retired.clone();
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                if callback_retired.get() {
                    return;
                }
                let SelectEvent::Confirm(value) = event;
                if let Some(value) = value {
                    writer.defer_set(value.clone(), window, cx);
                }
            },
        );

        Ok(Self {
            state,
            retired,
            _binding: binding,
            _subscription: subscription,
            _marker: PhantomData,
        })
    }

    pub(super) fn element(&self) -> Select<D> {
        Select::new(&self.state).disabled(self.retired.get())
    }
}

impl<Root, D, Value> Deref for FormScalarSelect<Root, D, Value>
where
    Root: FormSchema,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Value>,
    Value: Clone + PartialEq + 'static,
{
    type Target = Entity<SelectState<D>>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

pub(super) struct FormCaseSelect<Root, D, Enum, Kind>
where
    Root: FormSchema,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Kind>,
    Enum: Clone + PartialEq + 'static,
    Kind: Copy + PartialEq + 'static,
{
    state: Entity<SelectState<D>>,
    current_kind: Rc<Cell<Kind>>,
    retired: Rc<Cell<bool>>,
    _binding: ControlBinding,
    _subscription: Subscription,
    _marker: PhantomData<fn() -> (Root, Enum)>,
}

impl<Root, D, Enum, Kind> FormCaseSelect<Root, D, Enum, Kind>
where
    Root: FormSchema,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Kind>,
    Enum: Clone + PartialEq + 'static,
    Kind: Copy + PartialEq + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new<Owner, Path, Build>(
        form: &Entity<Form<Root>>,
        path: Path,
        kind: fn(&Enum) -> Kind,
        build_value: fn(Kind) -> Enum,
        build_state: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Owner: 'static,
        Path: IntoTotalPath<Root, Enum>,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let path = path.into_total_path();
        let initial_kind = kind(&path.get(form, cx));
        let state = cx.new(|cx| build_state(window, cx));
        state.update(cx, |state, cx| {
            state.set_selected_value(&initial_kind, window, cx)
        });

        let current_kind = Rc::new(Cell::new(initial_kind));
        let retired = Rc::new(Cell::new(false));
        let projector_kind = current_kind.clone();
        let projector_retired = retired.clone();
        let (binding, writer) = path.bind_control_in(
            form,
            &state,
            move |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    let next_kind = kind(&value);
                    projector_retired.set(false);
                    projector_kind.set(next_kind);
                    state.set_selected_value(&next_kind, window, cx);
                }
                ControlProjection::Retired => {
                    projector_retired.set(true);
                    cx.notify();
                }
            },
            window,
            cx,
        );

        let callback_kind = current_kind.clone();
        let callback_retired = retired.clone();
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                if callback_retired.get() {
                    return;
                }
                let SelectEvent::Confirm(selected) = event;
                let Some(selected) = *selected else {
                    return;
                };
                if callback_kind.get() == selected {
                    return;
                }

                // SelectState has already committed its native selected index before emitting
                // Confirm. Record the matching kind before deferring the Form write so this
                // adapter does not wait for a self-projection that intentionally never arrives.
                callback_kind.set(selected);
                writer.defer_set(build_value(selected), window, cx);
            },
        );

        Self {
            state,
            current_kind,
            retired,
            _binding: binding,
            _subscription: subscription,
            _marker: PhantomData,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn try_new<Owner, Build>(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, Enum>,
        kind: fn(&Enum) -> Kind,
        build_value: fn(Kind) -> Enum,
        build_state: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, ResolveError>
    where
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<SelectState<D>>) -> SelectState<D>,
    {
        let initial_kind = kind(&path.try_get(form, cx)?);
        let state = cx.new(|cx| build_state(window, cx));
        state.update(cx, |state, cx| {
            state.set_selected_value(&initial_kind, window, cx)
        });

        let current_kind = Rc::new(Cell::new(initial_kind));
        let retired = Rc::new(Cell::new(false));
        let projector_kind = current_kind.clone();
        let projector_retired = retired.clone();
        let (binding, writer) = path.try_bind_control_in(
            form,
            &state,
            move |state, projection, window, cx| match projection {
                ControlProjection::Value(value) => {
                    let next_kind = kind(&value);
                    projector_retired.set(false);
                    projector_kind.set(next_kind);
                    state.set_selected_value(&next_kind, window, cx);
                }
                ControlProjection::Retired => {
                    projector_retired.set(true);
                    cx.notify();
                }
            },
            window,
            cx,
        )?;

        let callback_kind = current_kind.clone();
        let callback_retired = retired.clone();
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, _, event: &SelectEvent<D>, window, cx| {
                if callback_retired.get() {
                    return;
                }
                let SelectEvent::Confirm(selected) = event;
                let Some(selected) = *selected else {
                    return;
                };
                if callback_kind.get() == selected {
                    return;
                }
                callback_kind.set(selected);
                writer.defer_set(build_value(selected), window, cx);
            },
        );

        Ok(Self {
            state,
            current_kind,
            retired,
            _binding: binding,
            _subscription: subscription,
            _marker: PhantomData,
        })
    }

    pub(super) fn element(&self) -> Select<D> {
        Select::new(&self.state).disabled(self.retired.get())
    }

    pub(super) fn current_kind(&self) -> Kind {
        self.current_kind.get()
    }
}

impl<Root, D, Enum, Kind> Deref for FormCaseSelect<Root, D, Enum, Kind>
where
    Root: FormSchema,
    D: SearchableListDelegate + 'static,
    D::Item: SearchableListItem<Value = Kind>,
    Enum: Clone + PartialEq + 'static,
    Kind: Copy + PartialEq + 'static,
{
    type Target = Entity<SelectState<D>>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[derive(Clone)]
pub(super) struct FilePathLabels {
    pub(super) select: SharedString,
    pub(super) change: SharedString,
    pub(super) clear: SharedString,
    pub(super) empty: SharedString,
}

#[derive(Clone, Copy)]
enum FilePathEvent {
    Select,
    Clear,
}

pub(super) struct FilePathState {
    path: Option<PathBuf>,
    retired: bool,
    labels: FilePathLabels,
}

impl FilePathState {
    pub(super) fn new(labels: FilePathLabels) -> Self {
        Self {
            path: None,
            retired: false,
            labels,
        }
    }

    fn set_path(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        self.path = path;
        cx.notify();
    }

    fn select(&mut self, cx: &mut Context<Self>) {
        cx.emit(FilePathEvent::Select);
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_path(None, cx);
        cx.emit(FilePathEvent::Clear);
    }

    fn retire(&mut self, cx: &mut Context<Self>) {
        self.retired = true;
        cx.notify();
    }
}

impl EventEmitter<FilePathEvent> for FilePathState {}

impl Render for FilePathState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let select_label = if self.path.is_some() {
            self.labels.change.clone()
        } else {
            self.labels.select.clone()
        };
        let path_label: SharedString = self
            .path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned().into())
            .unwrap_or_else(|| self.labels.empty.clone());

        h_flex()
            .gap_2()
            .child(gpui::div().flex_1().truncate().child(path_label))
            .child(
                Button::new(("request-file-select", cx.entity_id()))
                    .label(select_label)
                    .disabled(self.retired)
                    .on_click(cx.listener(|this, _, _, cx| this.select(cx))),
            )
            .child(
                Button::new(("request-file-clear", cx.entity_id()))
                    .label(self.labels.clear.clone())
                    .disabled(self.retired || self.path.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.clear(cx))),
            )
    }
}

pub(super) struct FormFilePathInput {
    state: Entity<FilePathState>,
    picker_task: Rc<RefCell<Option<Task<()>>>>,
    _binding: ControlBinding,
    _subscription: Subscription,
}

impl FormFilePathInput {
    #[allow(dead_code)]
    pub(super) fn new<Root, Owner, Path>(
        form: &Entity<Form<Root>>,
        path: Path,
        labels: FilePathLabels,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Self
    where
        Root: FormSchema,
        Owner: 'static,
        Path: IntoTotalPath<Root, Option<PathBuf>>,
    {
        let path = path.into_total_path();
        let initial = path.get(form, cx);
        let state = cx.new(|_| FilePathState::new(labels));
        state.update(cx, |state, cx| state.set_path(initial, cx));

        let picker_task = Rc::new(RefCell::new(None));
        let projector_task = picker_task.clone();
        let (binding, writer) = path.bind_control_in(
            form,
            &state,
            move |state, projection, _window, cx| match projection {
                ControlProjection::Value(path) => state.set_path(path, cx),
                ControlProjection::Retired => {
                    projector_task.borrow_mut().take();
                    state.retire(cx);
                }
            },
            window,
            cx,
        );
        let subscription =
            subscribe_file_path_events(&state, writer, picker_task.clone(), window, cx);

        Self {
            state,
            picker_task,
            _binding: binding,
            _subscription: subscription,
        }
    }

    pub(super) fn try_new<Root, Owner>(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, Option<PathBuf>>,
        labels: FilePathLabels,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, ResolveError>
    where
        Root: FormSchema,
        Owner: 'static,
    {
        let initial = path.try_get(form, cx)?;
        let state = cx.new(|_| FilePathState::new(labels));
        state.update(cx, |state, cx| state.set_path(initial, cx));

        let picker_task = Rc::new(RefCell::new(None));
        let projector_task = picker_task.clone();
        let (binding, writer) = path.try_bind_control_in(
            form,
            &state,
            move |state, projection, _window, cx| match projection {
                ControlProjection::Value(path) => state.set_path(path, cx),
                ControlProjection::Retired => {
                    projector_task.borrow_mut().take();
                    state.retire(cx);
                }
            },
            window,
            cx,
        )?;
        let subscription =
            subscribe_file_path_events(&state, writer, picker_task.clone(), window, cx);

        Ok(Self {
            state,
            picker_task,
            _binding: binding,
            _subscription: subscription,
        })
    }

    #[cfg(test)]
    pub(super) fn test_begin_select<Owner>(&self, cx: &mut Context<Owner>) {
        self.state.update(cx, |state, cx| state.select(cx));
    }

    #[cfg(test)]
    pub(super) fn test_has_picker_task(&self) -> bool {
        self.picker_task.borrow().is_some()
    }
}

fn subscribe_file_path_events<Root, Owner>(
    state: &Entity<FilePathState>,
    writer: gpui_form::ControlWriter<Root, Option<PathBuf>>,
    picker_task: Rc<RefCell<Option<Task<()>>>>,
    window: &mut Window,
    cx: &mut Context<Owner>,
) -> Subscription
where
    Root: FormSchema,
    Owner: 'static,
{
    let weak_state = state.downgrade();
    cx.subscribe_in(
        state,
        window,
        move |_, state, event: &FilePathEvent, window, cx| match event {
            FilePathEvent::Select if !state.read(cx).retired => {
                let prompt = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                let weak_state = weak_state.clone();
                let writer = writer.clone();
                let task = window.spawn(cx, async move |cx| {
                    let path = match prompt.await {
                        Ok(Ok(Some(paths))) => paths.into_iter().next(),
                        Ok(Ok(None)) | Ok(Err(_)) | Err(_) => None,
                    };
                    let Some(path) = path.filter(|path| path.is_absolute()) else {
                        return;
                    };
                    let _ = weak_state.update_in(cx, |state, window, cx| {
                        if state.retired {
                            return;
                        }
                        state.set_path(Some(path.clone()), cx);
                        writer.defer_set(Some(path), window, cx);
                    });
                });
                *picker_task.borrow_mut() = Some(task);
            }
            FilePathEvent::Clear if !state.read(cx).retired => {
                picker_task.borrow_mut().take();
                writer.defer_set(None, window, cx);
            }
            FilePathEvent::Select | FilePathEvent::Clear => {}
        },
    )
}

impl Deref for FormFilePathInput {
    type Target = Entity<FilePathState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for FormFilePathInput {
    fn drop(&mut self) {
        self.picker_task.borrow_mut().take();
    }
}

#[cfg(test)]
mod tests {
    use gpui::{IntoElement, TestAppContext, VisualTestContext, WindowHandle, div};

    use super::*;
    use crate::features::request::draft::{BinaryBodyDraft, RequestBodyDraft, RequestDraft};

    struct PickerHarness {
        form: Entity<Form<RequestDraft>>,
        state: Entity<FilePathState>,
        control: FormFilePathInput,
    }

    impl PickerHarness {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let draft = RequestDraft {
                body: RequestBodyDraft::Binary(BinaryBodyDraft::default()),
                ..RequestDraft::default()
            };
            let form = cx.new(|_| Form::new(draft));
            let binary = RequestDraft::BODY
                .case(RequestBodyDraft::BINARY)
                .resolve(&form, cx)
                .expect("resolve Binary body")
                .expect("Binary body starts active");
            let control = FormFilePathInput::try_new(
                &form,
                binary.then(BinaryBodyDraft::FILE),
                FilePathLabels {
                    select: "Select".into(),
                    change: "Change".into(),
                    clear: "Clear".into(),
                    empty: "None".into(),
                },
                window,
                cx,
            )
            .expect("bind Binary file path");
            let state = (*control).clone();
            Self {
                form,
                state,
                control,
            }
        }
    }

    impl Render for PickerHarness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn open_picker_harness(cx: &mut TestAppContext) -> WindowHandle<PickerHarness> {
        cx.update(|cx| {
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| PickerHarness::new(window, cx))
            })
            .expect("open picker test window")
        })
    }

    fn picker_entities(
        root: &Entity<PickerHarness>,
        cx: &mut VisualTestContext,
    ) -> (Entity<Form<RequestDraft>>, Entity<FilePathState>) {
        cx.update(|_, cx| root.read_with(cx, |root, _| (root.form.clone(), root.state.clone())))
    }

    fn active_binary_file(
        form: &Entity<Form<RequestDraft>>,
        cx: &mut VisualTestContext,
    ) -> Option<PathBuf> {
        cx.update(|_, cx| {
            RequestDraft::BODY
                .case(RequestBodyDraft::BINARY)
                .resolve(form, cx)
                .expect("resolve current Binary body")
                .expect("Binary body is active")
                .then(BinaryBodyDraft::FILE)
                .try_get(form, cx)
                .expect("read current Binary file")
        })
    }

    fn emit_picker_event(
        state: &Entity<FilePathState>,
        event: FilePathEvent,
        cx: &mut VisualTestContext,
    ) {
        cx.update(|_, cx| {
            state.update(cx, |state, cx| match event {
                FilePathEvent::Select => state.select(cx),
                FilePathEvent::Clear => state.clear(cx),
            });
        });
    }

    fn begin_picker(root: &Entity<PickerHarness>, cx: &mut VisualTestContext) {
        cx.update(|_, cx| {
            root.update(cx, |root, cx| root.control.test_begin_select(cx));
        });
    }

    fn has_picker_task(root: &Entity<PickerHarness>, cx: &mut VisualTestContext) -> bool {
        cx.update(|_, cx| root.read_with(cx, |root, _| root.control.test_has_picker_task()))
    }

    #[gpui::test]
    fn picker_writes_only_absolute_paths_and_clear_and_cancel_are_explicit(
        cx: &mut TestAppContext,
    ) {
        let window = open_picker_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("picker harness root");
        let (form, state) = picker_entities(&root, &mut cx);

        begin_picker(&root, &mut cx);
        assert!(cx.did_prompt_for_paths());
        assert!(has_picker_task(&root, &mut cx));
        let selected = std::env::temp_dir().join("http-client-request-body.bin");
        cx.simulate_path_prompt_response({
            let selected = selected.clone();
            move |options| {
                assert!(options.files);
                assert!(!options.directories);
                assert!(!options.multiple);
                Some(vec![selected])
            }
        });
        cx.run_until_parked();
        assert_eq!(active_binary_file(&form, &mut cx), Some(selected.clone()));
        cx.update(|_, cx| assert_eq!(state.read(cx).path, Some(selected.clone())));

        begin_picker(&root, &mut cx);
        cx.simulate_path_prompt_response(|_| None);
        cx.run_until_parked();
        assert_eq!(active_binary_file(&form, &mut cx), Some(selected.clone()));

        begin_picker(&root, &mut cx);
        cx.simulate_path_prompt_response(|_| Some(vec![PathBuf::from("relative.bin")]));
        cx.run_until_parked();
        assert_eq!(active_binary_file(&form, &mut cx), Some(selected));

        emit_picker_event(&state, FilePathEvent::Clear, &mut cx);
        cx.run_until_parked();
        assert_eq!(active_binary_file(&form, &mut cx), None);
        cx.update(|_, cx| assert!(state.read(cx).path.is_none()));
    }

    #[gpui::test]
    fn retired_picker_drops_task_and_cannot_write_a_fresh_occurrence(cx: &mut TestAppContext) {
        let window = open_picker_harness(cx);
        let mut cx = VisualTestContext::from_window(window.into(), cx);
        let root = window.root(&mut cx).expect("picker harness root");
        let (form, state) = picker_entities(&root, &mut cx);

        begin_picker(&root, &mut cx);
        assert!(cx.did_prompt_for_paths());
        assert!(has_picker_task(&root, &mut cx));
        cx.update(|_, cx| {
            RequestDraft::BODY.set(&form, RequestBodyDraft::None, cx);
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            let root = root.read(cx);
            assert!(root.control.picker_task.borrow().is_none());
            assert!(state.read(cx).retired);
            RequestDraft::BODY.set(&form, RequestBodyDraft::binary(), cx);
        });
        cx.run_until_parked();

        cx.simulate_path_prompt_response(|_| {
            Some(vec![std::env::temp_dir().join("late-picker-result.bin")])
        });
        cx.run_until_parked();

        assert_eq!(active_binary_file(&form, &mut cx), None);
    }
}
