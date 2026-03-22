use std::sync::mpsc;

use windows_core::RuntimeType;
use windows_future::{
    AsyncOperationCompletedHandler, AsyncOperationWithProgressCompletedHandler, AsyncStatus,
    IAsyncOperation, IAsyncOperationWithProgress,
};

pub(crate) mod image_frame;

pub(crate) fn wait_async_operation<T>(operation: IAsyncOperation<T>) -> windows_core::Result<T>
where
    T: RuntimeType + Clone,
{
    if operation.Status()? == AsyncStatus::Started {
        let (tx, rx) = mpsc::channel();
        operation.SetCompleted(&AsyncOperationCompletedHandler::new(move |_, _| {
            let _ = tx.send(());
            Ok(())
        }))?;
        let _ = rx.recv();
    }
    operation.GetResults()
}

pub(crate) fn wait_async_operation_with_progress<T, P>(
    operation: IAsyncOperationWithProgress<T, P>,
) -> windows_core::Result<T>
where
    T: RuntimeType + Clone,
    P: RuntimeType + Clone,
{
    if operation.Status()? == AsyncStatus::Started {
        let (tx, rx) = mpsc::channel();
        operation.SetCompleted(&AsyncOperationWithProgressCompletedHandler::new(
            move |_, _| {
                let _ = tx.send(());
                Ok(())
            },
        ))?;
        let _ = rx.recv();
    }
    operation.GetResults()
}
