use crate::errors::{AiChatError, AiChatResult};
use anyhow::Result as AnyResult;
use gpui::{
    App, AppContext, AsyncWindowContext, Context, Entity, Flatten, Global, VisualContext,
    WeakEntity, Window,
};

pub(crate) trait EntityResultExt<T> {
    fn update_result<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> AiChatResult<R>
    where
        C: AppContext,
        AnyResult<C::Result<R>>: Flatten<R>;
}

pub(crate) trait WeakEntityResultExt<T> {
    fn update_result<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> AiChatResult<R>
    where
        C: AppContext,
        AnyResult<C::Result<R>>: Flatten<R>;

    fn update_in_result<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> AiChatResult<R>
    where
        C: VisualContext,
        AnyResult<C::Result<R>>: Flatten<R>;

    fn read_with_result<C, R>(&self, cx: &C, read: impl FnOnce(&T, &App) -> R) -> AiChatResult<R>
    where
        C: AppContext,
        AnyResult<C::Result<R>>: Flatten<R>;
}

pub(crate) trait AsyncWindowContextResultExt {
    fn read_global_result<G, R>(
        &mut self,
        read: impl FnOnce(&G, &Window, &App) -> R,
    ) -> AiChatResult<R>
    where
        G: Global;
}

impl<T: 'static> EntityResultExt<T> for Entity<T> {
    fn update_result<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> AiChatResult<R>
    where
        C: AppContext,
        AnyResult<C::Result<R>>: Flatten<R>,
    {
        Flatten::flatten(Ok(self.update(cx, update))).map_err(|_| AiChatError::GpuiError)
    }
}

impl<T: 'static> WeakEntityResultExt<T> for WeakEntity<T> {
    fn update_result<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Context<T>) -> R,
    ) -> AiChatResult<R>
    where
        C: AppContext,
        AnyResult<C::Result<R>>: Flatten<R>,
    {
        self.update(cx, update).map_err(|_| AiChatError::GpuiError)
    }

    fn update_in_result<C, R>(
        &self,
        cx: &mut C,
        update: impl FnOnce(&mut T, &mut Window, &mut Context<T>) -> R,
    ) -> AiChatResult<R>
    where
        C: VisualContext,
        AnyResult<C::Result<R>>: Flatten<R>,
    {
        self.update_in(cx, update)
            .map_err(|_| AiChatError::GpuiError)
    }

    fn read_with_result<C, R>(&self, cx: &C, read: impl FnOnce(&T, &App) -> R) -> AiChatResult<R>
    where
        C: AppContext,
        AnyResult<C::Result<R>>: Flatten<R>,
    {
        self.read_with(cx, read).map_err(|_| AiChatError::GpuiError)
    }
}

impl AsyncWindowContextResultExt for AsyncWindowContext {
    fn read_global_result<G, R>(
        &mut self,
        read: impl FnOnce(&G, &Window, &App) -> R,
    ) -> AiChatResult<R>
    where
        G: Global,
    {
        self.read_global(read).map_err(|_| AiChatError::GpuiError)
    }
}
