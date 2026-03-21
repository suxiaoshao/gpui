use crate::{
    CaptureError,
    capture::{ImageFrame, shared::decode_image_file},
};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};
use std::{
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn capture_user_selected_area() -> Result<ImageFrame, CaptureError> {
    ensure_capture_access()?;
    let output_path = temp_capture_path();
    let output = Command::new("screencapture")
        .arg("-i")
        .arg("-s")
        .arg("-x")
        .arg(&output_path)
        .output()
        .map_err(|err| {
            CaptureError::SystemFailure(format!("failed to launch screencapture: {err}"))
        })?;

    if output.status.success() {
        let result = decode_image_file(&output_path).map_err(CaptureError::SystemFailure);
        let _ = std::fs::remove_file(&output_path);
        return result;
    }

    let file_exists = output_path.exists();
    let _ = std::fs::remove_file(&output_path);
    if !file_exists {
        return Err(CaptureError::Cancelled);
    }

    Err(CaptureError::SystemFailure(format!(
        "screencapture exited with status {}",
        output.status
    )))
}

pub(super) fn handle_capture_callback_url(_: &str) -> Result<bool, CaptureError> {
    Ok(false)
}

fn ensure_capture_access() -> Result<(), CaptureError> {
    if CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() {
        Ok(())
    } else {
        Err(CaptureError::PermissionDenied)
    }
}

fn temp_capture_path() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "platform-ext-capture-{}-{suffix}.png",
        std::process::id()
    ))
}
