use crate::{
    CaptureError,
    capture::{ImageFrame, shared::decode_image_file},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;
use windows::{
    ApplicationModel::DataTransfer::SharedStorageAccessManager,
    Foundation::Uri,
    System::Launcher,
    Win32::Foundation::{APPMODEL_ERROR_NO_PACKAGE, APPMODEL_ERROR_PACKAGE_NOT_AVAILABLE},
    core::{HRESULT, HSTRING},
};

pub(crate) mod async_support;
pub(crate) mod image_frame;

use async_support::wait_async_operation;

const CALLBACK_SCHEME: &str = "ai-chat-screenclip";
const CALLBACK_HOST: &str = "capture-response";
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(300);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const PACKAGED_BUILD_REQUIRED_MESSAGE: &str =
    "windows screen capture requires a packaged build with package identity";

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

    let result = match callback.code {
        200 => write_success_artifact(&callback.request_id, callback.token.as_deref()),
        499 => write_cancelled_artifact(&callback.request_id),
        _ => write_error_artifact(
            &callback.request_id,
            callback
                .reason
                .as_deref()
                .unwrap_or("snipping tool callback failed"),
        ),
    };

    if let Err(err) = result {
        write_error_artifact(&callback.request_id, &err.to_string())?;
    }

    Ok(true)
}

#[derive(Debug)]
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
    if !is_valid_request_id(&request_id) {
        return Err(CaptureError::InvalidInput(
            "capture callback contained invalid client-request-id",
        ));
    }
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
    let storage_file = match wait_async_operation(
        SharedStorageAccessManager::RedeemTokenForFileAsync(&token).map_err(map_capture_error)?,
    )
    .map_err(map_capture_error)
    {
        Ok(file) => file,
        Err(err) if requires_packaged_build(&err) => {
            return Err(CaptureError::BackendUnavailable(
                PACKAGED_BUILD_REQUIRED_MESSAGE,
            ));
        }
        Err(err) => return Err(err),
    };
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
    debug_assert!(is_valid_request_id(request_id));
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

fn requires_packaged_build(error: &CaptureError) -> bool {
    let CaptureError::SystemFailure(message) = error else {
        return false;
    };
    let no_package = HRESULT::from_win32(APPMODEL_ERROR_NO_PACKAGE.0);
    let package_unavailable = HRESULT::from_win32(APPMODEL_ERROR_PACKAGE_NOT_AVAILABLE.0);
    let no_package_code = format!("{:#010x}", no_package.0 as u32);
    let package_unavailable_code = format!("{:#010x}", package_unavailable.0 as u32);
    message.contains(&no_package_code) || message.contains(&package_unavailable_code)
}

fn is_valid_request_id(request_id: &str) -> bool {
    !request_id.is_empty()
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::{
        CALLBACK_HOST, CALLBACK_SCHEME, PACKAGED_BUILD_REQUIRED_MESSAGE, is_valid_request_id,
        parse_callback_url, requires_packaged_build,
    };
    use crate::CaptureError;

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

    #[test]
    fn rejects_callback_without_request_id() {
        let error = parse_callback_url(&format!("{CALLBACK_SCHEME}://{CALLBACK_HOST}?code=200"))
            .expect_err("missing request id should fail");

        assert_eq!(
            error,
            CaptureError::InvalidInput("capture callback missing client-request-id")
        );
    }

    #[test]
    fn rejects_callback_without_status_code() {
        let error = parse_callback_url(&format!(
            "{CALLBACK_SCHEME}://{CALLBACK_HOST}?client-request-id=req-1"
        ))
        .expect_err("missing code should fail");

        assert_eq!(
            error,
            CaptureError::InvalidInput("capture callback missing status code")
        );
    }

    #[test]
    fn package_identity_errors_are_detected() {
        let error = CaptureError::SystemFailure(format!(
            "operation failed with HRESULT {:#010x}: {}",
            windows::core::HRESULT::from_win32(
                windows::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE.0
            )
            .0 as u32,
            PACKAGED_BUILD_REQUIRED_MESSAGE
        ));

        assert!(requires_packaged_build(&error));
    }

    #[test]
    fn rejects_callback_with_unsafe_request_id() {
        let error = parse_callback_url(&format!(
            "{CALLBACK_SCHEME}://{CALLBACK_HOST}?client-request-id=..%5Cevil&code=200"
        ))
        .expect_err("should reject unsafe request id");

        assert!(matches!(error, CaptureError::InvalidInput(_)));
    }

    #[test]
    fn validates_generated_request_id_shape() {
        assert!(is_valid_request_id("123-456"));
        assert!(!is_valid_request_id("../456"));
        assert!(!is_valid_request_id("..\\456"));
        assert!(!is_valid_request_id(""));
    }
}
