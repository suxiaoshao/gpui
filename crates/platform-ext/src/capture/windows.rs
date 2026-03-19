use crate::{
    CaptureError,
    capture::{ImageFrame, shared::decode_image_file},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;
use windows::{
    ApplicationModel::DataTransfer::SharedStorageAccessManager, Foundation::Uri, System::Launcher,
    core::HSTRING,
};

pub(crate) mod async_support;
pub(crate) mod image_frame;

use async_support::wait_async_operation;

const CALLBACK_SCHEME: &str = "ai-chat-screenclip";
const CALLBACK_HOST: &str = "capture-response";
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) fn capture_user_selected_area() -> Result<ImageFrame, CaptureError> {
    let request_id = next_request_id();
    prepare_request_dir()?;
    cleanup_request_artifacts(&request_id);

    let launch_uri = build_capture_uri(&request_id);
    let uri = Uri::CreateUri(&HSTRING::from(launch_uri)).map_err(map_capture_error)?;
    let launched = wait_async_operation(Launcher::LaunchUriAsync(&uri).map_err(map_capture_error)?)
        .map_err(map_capture_error)?;
    if !launched {
        return Err(CaptureError::BackendUnavailable(
            "failed to launch snipping tool",
        ));
    }

    wait_for_capture_result(&request_id)
}

pub(super) fn handle_capture_callback_url(url: &str) -> Result<bool, CaptureError> {
    let Some(callback) = parse_callback_url(url)? else {
        return Ok(false);
    };

    prepare_request_dir()?;
    cleanup_request_artifacts(&callback.request_id);

    match callback.code {
        200 => write_success_artifact(&callback.request_id, callback.token.as_deref())?,
        499 => write_cancelled_artifact(&callback.request_id)?,
        _ => write_error_artifact(
            &callback.request_id,
            callback
                .reason
                .as_deref()
                .unwrap_or("snipping tool callback failed"),
        )?,
    }

    Ok(true)
}

struct CaptureCallback {
    request_id: String,
    code: u16,
    token: Option<String>,
    reason: Option<String>,
}

fn build_capture_uri(request_id: &str) -> String {
    let redirect_uri =
        format!("{CALLBACK_SCHEME}://{CALLBACK_HOST}?client-request-id={request_id}");
    let redirect_uri =
        url::form_urlencoded::byte_serialize(redirect_uri.as_bytes()).collect::<String>();
    format!(
        "ms-screenclip://capture/image?api-version=1.0&user-agent=ai-chat&rectangle&enabledModes=RectangleSnip&redirect-uri={redirect_uri}"
    )
}

fn parse_callback_url(url: &str) -> Result<Option<CaptureCallback>, CaptureError> {
    let url = Url::parse(url)
        .map_err(|err| CaptureError::SystemFailure(format!("invalid callback url: {err}")))?;
    if url.scheme() != CALLBACK_SCHEME || url.host_str() != Some(CALLBACK_HOST) {
        return Ok(None);
    }

    let query = query_map(&url);
    let request_id = query
        .get("client-request-id")
        .cloned()
        .ok_or(CaptureError::InvalidInput(
            "capture callback missing client-request-id",
        ))?;
    let code = query
        .get("code")
        .ok_or(CaptureError::InvalidInput(
            "capture callback missing status code",
        ))?
        .parse::<u16>()
        .map_err(|_| {
            CaptureError::InvalidInput("capture callback contained invalid status code")
        })?;

    Ok(Some(CaptureCallback {
        request_id,
        code,
        token: query.get("token").cloned(),
        reason: query.get("reason").cloned(),
    }))
}

fn query_map(url: &Url) -> HashMap<String, String> {
    url.query_pairs()
        .map(|(key, value)| (key.to_ascii_lowercase(), value.into_owned()))
        .collect()
}

fn wait_for_capture_result(request_id: &str) -> Result<ImageFrame, CaptureError> {
    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    let image_path = request_image_path(request_id);
    let cancel_path = request_cancel_path(request_id);
    let error_path = request_error_path(request_id);

    loop {
        if image_path.exists() {
            let result = decode_image_file(&image_path).map_err(CaptureError::SystemFailure);
            cleanup_request_artifacts(request_id);
            return result;
        }
        if cancel_path.exists() {
            cleanup_request_artifacts(request_id);
            return Err(CaptureError::Cancelled);
        }
        if error_path.exists() {
            let message = std::fs::read_to_string(&error_path)
                .unwrap_or_else(|_| "failed to read snipping tool error message".to_string());
            cleanup_request_artifacts(request_id);
            return Err(CaptureError::SystemFailure(message));
        }
        if Instant::now() >= deadline {
            cleanup_request_artifacts(request_id);
            return Err(CaptureError::SystemFailure(
                "timed out waiting for snipping tool result".into(),
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn write_success_artifact(request_id: &str, token: Option<&str>) -> Result<(), CaptureError> {
    let token = token.ok_or(CaptureError::InvalidInput("capture callback missing token"))?;
    let token = HSTRING::from(token);
    let storage_file = wait_async_operation(
        SharedStorageAccessManager::RedeemTokenForFileAsync(&token).map_err(map_capture_error)?,
    )
    .map_err(map_capture_error)?;
    let source_path = storage_file.Path().map_err(map_capture_error)?.to_string();
    let source_path = PathBuf::from(source_path);

    let final_path = request_image_path(request_id);
    let staging_path = final_path.with_extension("tmp");
    std::fs::copy(&source_path, &staging_path).map_err(|err| {
        CaptureError::SystemFailure(format!(
            "failed to copy screenshot artifact from {}: {err}",
            source_path.display()
        ))
    })?;
    std::fs::rename(&staging_path, &final_path).map_err(|err| {
        CaptureError::SystemFailure(format!(
            "failed to finalize screenshot artifact {}: {err}",
            final_path.display()
        ))
    })?;
    let _ = SharedStorageAccessManager::RemoveFile(&token);
    Ok(())
}

fn write_cancelled_artifact(request_id: &str) -> Result<(), CaptureError> {
    std::fs::write(request_cancel_path(request_id), []).map_err(|err| {
        CaptureError::SystemFailure(format!("failed to write cancel artifact: {err}"))
    })
}

fn write_error_artifact(request_id: &str, message: &str) -> Result<(), CaptureError> {
    std::fs::write(request_error_path(request_id), message).map_err(|err| {
        CaptureError::SystemFailure(format!("failed to write error artifact: {err}"))
    })
}

fn prepare_request_dir() -> Result<(), CaptureError> {
    std::fs::create_dir_all(request_dir()).map_err(|err| {
        CaptureError::SystemFailure(format!("failed to create capture request dir: {err}"))
    })
}

fn cleanup_request_artifacts(request_id: &str) {
    let _ = std::fs::remove_file(request_image_path(request_id));
    let _ = std::fs::remove_file(request_cancel_path(request_id));
    let _ = std::fs::remove_file(request_error_path(request_id));
}

fn request_dir() -> PathBuf {
    std::env::temp_dir().join("platform-ext-screenclip")
}

fn request_image_path(request_id: &str) -> PathBuf {
    request_artifact_path(request_id, "img")
}

fn request_cancel_path(request_id: &str) -> PathBuf {
    request_artifact_path(request_id, "cancelled")
}

fn request_error_path(request_id: &str) -> PathBuf {
    request_artifact_path(request_id, "error")
}

fn request_artifact_path(request_id: &str, extension: &str) -> PathBuf {
    request_dir().join(format!("{request_id}.{extension}"))
}

fn next_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn map_capture_error(err: windows_core::Error) -> CaptureError {
    CaptureError::SystemFailure(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{CALLBACK_HOST, CALLBACK_SCHEME, parse_callback_url};

    #[test]
    fn ignores_non_capture_callback_urls() {
        let callback = parse_callback_url("https://example.com").expect("should parse");
        assert!(callback.is_none());
    }

    #[test]
    fn parses_capture_callback_url() {
        let callback = parse_callback_url(&format!(
            "{CALLBACK_SCHEME}://{CALLBACK_HOST}?client-request-id=req-1&code=200&reason=Success&token=abc"
        ))
        .expect("should parse")
        .expect("should be handled");

        assert_eq!(callback.request_id, "req-1");
        assert_eq!(callback.code, 200);
        assert_eq!(callback.token.as_deref(), Some("abc"));
        assert_eq!(callback.reason.as_deref(), Some("Success"));
    }
}
