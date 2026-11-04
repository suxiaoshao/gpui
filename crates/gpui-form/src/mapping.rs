use crate::{
    ControlBinding, ControlProjection, ControlWriter, DynamicPath, EditResult, EditorKey, Form,
    FormSchema, IntoTotalPath, PathCore, ResolveError,
};
use gpui::{App, Context, Entity, Window};
use std::{any::TypeId, cell::RefCell, rc::Rc};

pub trait ControlMapping<S, D>: 'static {
    type Context: Clone + 'static;
    fn format(&self, stored: &S, context: &Self::Context) -> D;
    fn parse(
        &self,
        display: D,
        stored: &S,
        baseline: Option<&S>,
        context: &Self::Context,
    ) -> EditResult<S, D>;
}

pub struct MappedControlBinding<C> {
    binding: ControlBinding,
    context: Rc<RefCell<C>>,
    editor: EditorKey,
}
impl<C> MappedControlBinding<C> {
    pub fn refresh(&self, context: C, cx: &mut App) {
        *self.context.borrow_mut() = context;
        self.binding.refresh(cx);
    }
    pub fn discard_draft(
        &self,
        cx: &mut App,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        self.binding.discard_editor_draft(&self.editor, cx)
    }
    pub fn editor_status(&self, cx: &App) -> Result<crate::EditorStatus, crate::LifecycleError> {
        self.binding.editor_status(&self.editor, cx)
    }
}
pub struct MappedControlWriter<M: FormSchema, S: 'static, D, Map: ControlMapping<S, D>> {
    writer: ControlWriter<M, S>,
    core: PathCore<M, S>,
    mapping: Rc<Map>,
    context: Rc<RefCell<Map::Context>>,
    marker: std::marker::PhantomData<fn() -> D>,
}
impl<M: FormSchema, S: 'static, D, Map: ControlMapping<S, D>> Clone
    for MappedControlWriter<M, S, D, Map>
{
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            core: self.core.clone(),
            mapping: self.mapping.clone(),
            context: self.context.clone(),
            marker: std::marker::PhantomData,
        }
    }
}
impl<
    M: FormSchema,
    S: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
    Map: ControlMapping<S, D>,
> MappedControlWriter<M, S, D, Map>
{
    pub fn discard_draft(
        &self,
        cx: &mut App,
    ) -> Result<crate::LifecycleReport, crate::LifecycleError> {
        self.writer.discard_draft(cx)
    }
    pub fn editor_status(&self, cx: &App) -> Result<crate::EditorStatus, crate::LifecycleError> {
        self.writer.editor_status(cx)
    }
    pub fn defer_edit<Owner: 'static>(&self, display: D, window: &Window, cx: &mut Context<Owner>) {
        let Some(form) = self.writer.form.upgrade() else {
            return;
        };
        let result = {
            let form = form.read(cx);
            if self.core.check(form).is_err() {
                return;
            }
            let topology = form.topology().snapshot();
            let Ok(stored) = self.core.access.get(form.value(), &topology) else {
                return;
            };
            // Parse now: queued input must never be reinterpreted after a context refresh.
            let context = self.context.borrow().clone();
            self.mapping.parse(
                display.clone(),
                stored,
                form.baseline_for(&self.core).as_ref(),
                &context,
            )
        };
        match result {
            EditResult::Parsed(value) => self.writer.defer_mapped_update(
                display,
                TypeId::of::<Map>(),
                crate::StorageUpdate::new(move |_| Ok(value)),
                window,
                cx,
            ),
            EditResult::Update(update) => {
                self.writer
                    .defer_mapped_update(display, TypeId::of::<Map>(), update, window, cx)
            }
            EditResult::Draft {
                display,
                code,
                message,
            } => self.writer.defer_mapped_draft(
                display,
                TypeId::of::<Map>(),
                code,
                message,
                window,
                cx,
            ),
        }
    }
    pub fn defer_blur<Owner: 'static>(&self, window: &Window, cx: &mut Context<Owner>) {
        self.writer.defer_blur(window, cx);
    }
}

#[derive(Debug)]
pub enum MappingBindError {
    Resolve(ResolveError),
    EditorType(crate::EditorTypeMismatch),
}
impl std::fmt::Display for MappingBindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mapping binding failed: {self:?}")
    }
}
impl std::error::Error for MappingBindError {}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn bind_mapped_control_in<M, S, D, Map, Owner, Path>(
    form: &Entity<Form<M>>,
    path: Path,
    editor: impl Into<EditorKey>,
    mapping: Map,
    context: Map::Context,
    owner: &Entity<Owner>,
    project: impl Fn(&mut Owner, ControlProjection<D>, &mut Window, &mut Context<Owner>) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Result<
    (
        MappedControlBinding<Map::Context>,
        MappedControlWriter<M, S, D, Map>,
    ),
    MappingBindError,
>
where
    M: FormSchema,
    S: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
    Map: ControlMapping<S, D>,
    Owner: 'static,
    Path: IntoTotalPath<M, S>,
{
    bind(
        form,
        path.into_total_path().core,
        false,
        editor.into(),
        mapping,
        context,
        owner,
        project,
        window,
        cx,
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn try_bind_mapped_control_in<M, S, D, Map, Owner>(
    form: &Entity<Form<M>>,
    path: DynamicPath<M, S>,
    editor: impl Into<EditorKey>,
    mapping: Map,
    context: Map::Context,
    owner: &Entity<Owner>,
    project: impl Fn(&mut Owner, ControlProjection<D>, &mut Window, &mut Context<Owner>) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Result<
    (
        MappedControlBinding<Map::Context>,
        MappedControlWriter<M, S, D, Map>,
    ),
    MappingBindError,
>
where
    M: FormSchema,
    S: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
    Map: ControlMapping<S, D>,
    Owner: 'static,
{
    path.core
        .check(form.read(cx))
        .map_err(MappingBindError::Resolve)?;
    bind(
        form,
        path.core,
        true,
        editor.into(),
        mapping,
        context,
        owner,
        project,
        window,
        cx,
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn bind<M, S, D, Map, Owner>(
    form: &Entity<Form<M>>,
    core: PathCore<M, S>,
    dynamic: bool,
    editor: EditorKey,
    mapping: Map,
    context: Map::Context,
    owner: &Entity<Owner>,
    project: impl Fn(&mut Owner, ControlProjection<D>, &mut Window, &mut Context<Owner>) + 'static,
    window: &mut Window,
    cx: &mut App,
) -> Result<
    (
        MappedControlBinding<Map::Context>,
        MappedControlWriter<M, S, D, Map>,
    ),
    MappingBindError,
>
where
    M: FormSchema,
    S: Clone + PartialEq + 'static,
    D: Clone + PartialEq + 'static,
    Map: ControlMapping<S, D>,
    Owner: 'static,
{
    let key = form
        .read(cx)
        .topology()
        .key(&core.address)
        .expect("bound path");
    let draft = form
        .read(cx)
        .editor_draft::<D>(&key, &editor, TypeId::of::<Map>())
        .map_err(MappingBindError::EditorType)?;
    let mapping = Rc::new(mapping);
    let context = Rc::new(RefCell::new(context));
    let project = Rc::new(project);
    let initial = draft.unwrap_or_else(|| {
        mapping.format(
            core.access
                .get(form.read(cx).value(), &form.read(cx).topology().snapshot())
                .expect("checked path"),
            &context.borrow(),
        )
    });
    owner.update(cx, |owner, cx| {
        project(owner, ControlProjection::Value(initial), window, cx)
    });
    let map = mapping.clone();
    let ctx = context.clone();
    let project_callback = project.clone();
    let weak_form = form.downgrade();
    let projection_key = key.clone();
    let projection_editor = editor.clone();
    let callback = move |owner: &mut Owner,
                         projection,
                         window: &mut Window,
                         cx: &mut Context<Owner>| {
        let projection = match projection {
            ControlProjection::Retired => ControlProjection::Retired,
            ControlProjection::Value(value) => {
                let draft = weak_form.upgrade().and_then(|form| {
                    form.read(cx)
                        .editor_draft::<D>(&projection_key, &projection_editor, TypeId::of::<Map>())
                        .ok()
                        .flatten()
                });
                ControlProjection::Value(draft.unwrap_or_else(|| map.format(&value, &ctx.borrow())))
            }
        };
        project_callback(owner, projection, window, cx);
    };
    let (binding, writer) = if dynamic {
        crate::control::bind_dynamic_in(
            form,
            DynamicPath { core: core.clone() },
            owner,
            callback,
            window,
            cx,
        )
        .map_err(MappingBindError::Resolve)?
    } else {
        crate::control::bind_total_in(
            form,
            crate::TotalPath { core: core.clone() },
            owner,
            callback,
            window,
            cx,
        )
    };
    let writer = writer.with_editor_key(editor.clone(), cx);
    Ok((
        MappedControlBinding {
            binding,
            context: context.clone(),
            editor,
        },
        MappedControlWriter {
            writer,
            core,
            mapping,
            context,
            marker: std::marker::PhantomData,
        },
    ))
}
