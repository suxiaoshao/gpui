use std::ops::Deref;

use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_component::input::{EditorState, InputEvent, InputState, TextareaState};
use gpui_form::{
    ControlBinding, ControlProjection, DynamicPath, Form, FormSchema, IntoTotalPath, ResolveError,
};

// All three controls share the same binding and event contract.
macro_rules! text_control {
    ($name:ident, $state:ty) => {
        pub struct $name {
            #[allow(dead_code)]
            subscriptions: Vec<Subscription>,
            _binding: ControlBinding,
            editor: crate::editor::EditorAccess,
            state: Entity<$state>,
        }

        impl $name {
            /// Explicitly discard this source editor's accepted draft and queued input.
            pub fn discard_draft(
                &self,
                cx: &mut gpui::App,
            ) -> Result<gpui_form::LifecycleReport, gpui_form::LifecycleError> {
                self.editor.discard_draft(cx)
            }
            pub fn editor_status(
                &self,
                cx: &gpui::App,
            ) -> Result<gpui_form::EditorStatus, gpui_form::LifecycleError> {
                self.editor.editor_status(cx)
            }

            pub fn new<Root, Owner, Path, Build>(
                form: &Entity<Form<Root>>,
                path: Path,
                build: Build,
                window: &mut Window,
                cx: &mut Context<Owner>,
            ) -> Self
            where
                Root: FormSchema,
                Owner: 'static,
                Path: IntoTotalPath<Root, String>,
                Build: FnOnce(&mut Window, &mut Context<$state>) -> $state,
            {
                let path = path.into_total_path();
                let value = path.get(form, cx);
                let state = cx.new(|cx| build(window, cx));
                state.update(cx, |state, cx| state.set_value(value, window, cx));
                let (binding, writer) = path.bind_control_in(
                    form,
                    &state,
                    |state, projection, window, cx| match projection {
                        ControlProjection::Value(value) => state.set_value(value, window, cx),
                        ControlProjection::Retired => {}
                    },
                    window,
                    cx,
                );
                let editor = crate::editor::EditorAccess::new(&writer);
                let state_subscription = cx.subscribe_in(
                    &state,
                    window,
                    move |_, state, event: &InputEvent, window, cx| match event {
                        InputEvent::Change => {
                            writer.defer_set(state.read(cx).value().to_string(), window, cx);
                        }
                        InputEvent::Blur => writer.defer_blur(window, cx),
                        InputEvent::Focus | InputEvent::PressEnter { .. } => {}
                    },
                );
                Self {
                    subscriptions: vec![state_subscription],
                    _binding: binding,
                    editor,
                    state,
                }
            }

            pub fn try_new<Root, Owner, Build>(
                form: &Entity<Form<Root>>,
                path: DynamicPath<Root, String>,
                build: Build,
                window: &mut Window,
                cx: &mut Context<Owner>,
            ) -> Result<Self, ResolveError>
            where
                Root: FormSchema,
                Owner: 'static,
                Build: FnOnce(&mut Window, &mut Context<$state>) -> $state,
            {
                let value = path.try_get(form, cx)?;
                let state = cx.new(|cx| build(window, cx));
                state.update(cx, |state, cx| state.set_value(value, window, cx));
                let (binding, writer) = path.try_bind_control_in(
                    form,
                    &state,
                    |state, projection, window, cx| match projection {
                        ControlProjection::Value(value) => state.set_value(value, window, cx),
                        ControlProjection::Retired => {}
                    },
                    window,
                    cx,
                )?;
                let editor = crate::editor::EditorAccess::new(&writer);
                let state_subscription = cx.subscribe_in(
                    &state,
                    window,
                    move |_, state, event: &InputEvent, window, cx| match event {
                        InputEvent::Change => {
                            writer.defer_set(state.read(cx).value().to_string(), window, cx);
                        }
                        InputEvent::Blur => writer.defer_blur(window, cx),
                        InputEvent::Focus | InputEvent::PressEnter { .. } => {}
                    },
                );
                Ok(Self {
                    subscriptions: vec![state_subscription],
                    _binding: binding,
                    editor,
                    state,
                })
            }
        }

        impl Deref for $name {
            type Target = Entity<$state>;

            fn deref(&self) -> &Self::Target {
                &self.state
            }
        }
    };
}

text_control!(FormInput, InputState);
text_control!(FormTextarea, TextareaState);
text_control!(FormEditor, EditorState);

/// Native text input backed by an application storage/display mapping.
pub struct FormMappedInput<C> {
    _subscription: Subscription,
    binding: gpui_form::MappedControlBinding<C>,
    state: Entity<InputState>,
}
impl<C: Clone + 'static> FormMappedInput<C> {
    #[allow(clippy::too_many_arguments)]
    pub fn new<Root, S, Map, Owner, Path, Build>(
        form: &Entity<Form<Root>>,
        path: Path,
        editor: impl Into<gpui_form::EditorKey>,
        mapping: Map,
        context: C,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, gpui_form::MappingBindError>
    where
        Root: FormSchema,
        S: Clone + PartialEq + 'static,
        Map: gpui_form::ControlMapping<S, String, Context = C>,
        Owner: 'static,
        Path: IntoTotalPath<Root, S>,
        Build: FnOnce(&mut Window, &mut Context<InputState>) -> InputState,
    {
        let state = cx.new(|cx| build(window, cx));
        let (binding, writer) = gpui_form::bind_mapped_control_in(
            form,
            path,
            editor,
            mapping,
            context,
            &state,
            |state, projection, window, cx| {
                if let ControlProjection::Value(value) = projection {
                    state.set_value(value, window, cx);
                }
            },
            window,
            cx,
        )?;
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    writer.defer_edit(state.read(cx).value().to_string(), window, cx)
                }
                InputEvent::Blur => writer.defer_blur(window, cx),
                _ => {}
            },
        );
        Ok(Self {
            _subscription: subscription,
            binding,
            state,
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn try_new<Root, S, Map, Owner, Build>(
        form: &Entity<Form<Root>>,
        path: DynamicPath<Root, S>,
        editor: impl Into<gpui_form::EditorKey>,
        mapping: Map,
        context: C,
        build: Build,
        window: &mut Window,
        cx: &mut Context<Owner>,
    ) -> Result<Self, gpui_form::MappingBindError>
    where
        Root: FormSchema,
        S: Clone + PartialEq + 'static,
        Map: gpui_form::ControlMapping<S, String, Context = C>,
        Owner: 'static,
        Build: FnOnce(&mut Window, &mut Context<InputState>) -> InputState,
    {
        let state = cx.new(|cx| build(window, cx));
        let (binding, writer) = gpui_form::try_bind_mapped_control_in(
            form,
            path,
            editor,
            mapping,
            context,
            &state,
            |state, projection, window, cx| {
                if let ControlProjection::Value(value) = projection {
                    state.set_value(value, window, cx);
                }
            },
            window,
            cx,
        )?;
        let subscription = cx.subscribe_in(
            &state,
            window,
            move |_, state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    writer.defer_edit(state.read(cx).value().to_string(), window, cx)
                }
                InputEvent::Blur => writer.defer_blur(window, cx),
                _ => {}
            },
        );
        Ok(Self {
            _subscription: subscription,
            binding,
            state,
        })
    }

    pub fn discard_draft(
        &self,
        cx: &mut gpui::App,
    ) -> Result<gpui_form::LifecycleReport, gpui_form::LifecycleError> {
        self.binding.discard_draft(cx)
    }
    pub fn editor_status(
        &self,
        cx: &gpui::App,
    ) -> Result<gpui_form::EditorStatus, gpui_form::LifecycleError> {
        self.binding.editor_status(cx)
    }

    pub fn refresh(&self, context: C, cx: &mut gpui::App) {
        self.binding.refresh(context, cx);
    }
}
impl<C> Deref for FormMappedInput<C> {
    type Target = Entity<InputState>;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
