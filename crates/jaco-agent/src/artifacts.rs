use std::{
    io::{BufReader, Read},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose};
use image::{GenericImageView as _, ImageFormat, ImageReader, Limits};
use jaco_core::{
    AttachmentKind, AttachmentMetadata, AttachmentSource, AttachmentStorageKind, ConversationId,
    ProviderId, RunErrorPayload, new_id,
};
use jaco_db::NewAttachment;
use reqwest::header;
use rig::message::{DocumentSourceKind, Image, ImageMediaType};
use sha2::{Digest as _, Sha256};
use tokio::{fs, io::AsyncWriteExt as _};
use tokio_util::sync::CancellationToken;
use url::{Host, Url};

const MAX_IMAGES: usize = 10;
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_DIMENSION: u32 = 16_384;
const MAX_PIXELS: u64 = 100_000_000;
const MAX_DECODER_OUTPUT_BYTES: u64 = 400 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REDIRECTS: usize = 3;
const GENERATED_FILE_PREFIX: &str = ".jaco-generated-";

#[derive(Clone)]
pub struct ManagedArtifactStore {
    conversation_id: ConversationId,
    conversation_dir: Arc<PathBuf>,
    limits: GeneratedImageLimits,
}

impl ManagedArtifactStore {
    pub fn new(conversation_id: ConversationId, conversation_dir: PathBuf) -> Self {
        Self {
            conversation_id,
            conversation_dir: Arc::new(conversation_dir),
            limits: GeneratedImageLimits::production(),
        }
    }

    pub(crate) fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub(crate) async fn prepare(
        &self,
        candidates: Vec<GeneratedImageCandidate>,
        provider_id: &ProviderId,
        cancellation: &CancellationToken,
    ) -> ArtifactResult<PreparedGeneratedArtifacts> {
        if candidates.is_empty() {
            return Ok(PreparedGeneratedArtifacts::armed());
        }
        if candidates.len() > self.limits.max_images {
            return Err(GeneratedArtifactError::limit());
        }
        cancellation_check(cancellation)?;
        self.prepare_directories().await?;

        let mut prepared = PreparedGeneratedArtifacts::armed();
        let mut response_bytes = 0_u64;
        for candidate in candidates {
            cancellation_check(cancellation)?;
            let attachment_id = new_id();
            let stage_path = self.pending_dir().join(format!("{attachment_id}.part"));
            reject_existing_path(&stage_path).await?;
            prepared.cleanup_paths.push(stage_path.clone());

            let declared_mime = candidate.declared_mime.clone();
            let download_mime = match &candidate.source {
                GeneratedImageSource::Base64(payload) => {
                    write_base64(payload, &stage_path, self.limits.max_image_bytes).await?;
                    None
                }
                GeneratedImageSource::Url(locator) => {
                    download_url(
                        locator,
                        &stage_path,
                        self.limits.max_image_bytes,
                        cancellation,
                    )
                    .await?
                }
            };
            cancellation_check(cancellation)?;

            let declared_mimes = declared_mime
                .into_iter()
                .chain(download_mime)
                .collect::<Vec<_>>();
            let metadata = inspect_image(stage_path.clone(), declared_mimes, self.limits).await?;
            cancellation_check(cancellation)?;
            response_bytes = response_bytes
                .checked_add(metadata.size_bytes)
                .ok_or_else(GeneratedArtifactError::limit)?;
            if response_bytes > self.limits.max_response_bytes {
                return Err(GeneratedArtifactError::limit());
            }

            let final_path = self.conversation_dir.join(format!(
                "{GENERATED_FILE_PREFIX}{attachment_id}.{}",
                metadata.extension
            ));
            reject_existing_path(&final_path).await?;
            fs::rename(&stage_path, &final_path)
                .await
                .map_err(GeneratedArtifactError::storage)?;
            prepared.cleanup_paths.pop();
            prepared.cleanup_paths.push(final_path.clone());
            require_regular_file(&final_path).await?;
            sync_directory(&self.conversation_dir).await?;
            cancellation_check(cancellation)?;

            let final_path_text = final_path.to_string_lossy().into_owned();
            prepared.images.push(PreparedGeneratedImage {
                attachment: NewAttachment {
                    id: attachment_id,
                    conversation_id: self.conversation_id.clone(),
                    kind: AttachmentKind::Image,
                    storage_kind: AttachmentStorageKind::GeneratedFile,
                    mime_type: Some(metadata.mime_type.to_string()),
                    name: Some(format!(
                        "generated-image-{}.{}",
                        candidate.ordinal, metadata.extension
                    )),
                    path: Some(final_path_text.clone()),
                    external_uri: None,
                    provider_id: Some(provider_id.clone()),
                    provider_file_id: None,
                    sha256: Some(metadata.sha256),
                    size_bytes: Some(
                        i64::try_from(metadata.size_bytes)
                            .map_err(|_| GeneratedArtifactError::limit())?,
                    ),
                    metadata: AttachmentMetadata {
                        source: AttachmentSource::GeneratedFile {
                            path: final_path_text,
                        },
                        width: Some(metadata.width),
                        height: Some(metadata.height),
                        duration_ms: None,
                        preview_attachment_id: None,
                    },
                },
                final_path,
            });
        }
        Ok(prepared)
    }

    async fn prepare_directories(&self) -> ArtifactResult<()> {
        let root = self.conversation_dir.parent().ok_or_else(|| {
            GeneratedArtifactError::storage(std::io::Error::other(
                "managed conversation directory has no attachments root",
            ))
        })?;
        ensure_directory(root).await?;
        ensure_directory(&self.conversation_dir).await?;
        ensure_directory(&self.pending_dir()).await?;
        Ok(())
    }

    fn pending_dir(&self) -> PathBuf {
        self.conversation_dir.join(".pending")
    }

    #[cfg(test)]
    fn with_limits(mut self, limits: GeneratedImageLimits) -> Self {
        self.limits = limits;
        self
    }
}

#[derive(Clone, Copy)]
struct GeneratedImageLimits {
    max_images: usize,
    max_image_bytes: u64,
    max_response_bytes: u64,
    max_dimension: u32,
    max_pixels: u64,
    max_decoder_output_bytes: u64,
}

impl GeneratedImageLimits {
    const fn production() -> Self {
        Self {
            max_images: MAX_IMAGES,
            max_image_bytes: MAX_IMAGE_BYTES,
            max_response_bytes: MAX_RESPONSE_BYTES,
            max_dimension: MAX_DIMENSION,
            max_pixels: MAX_PIXELS,
            max_decoder_output_bytes: MAX_DECODER_OUTPUT_BYTES,
        }
    }
}

#[derive(Debug)]
pub(crate) struct GeneratedImageCandidate {
    pub(crate) ordinal: usize,
    pub(crate) source: GeneratedImageSource,
    pub(crate) declared_mime: Option<String>,
}

#[derive(Debug)]
pub(crate) enum GeneratedImageSource {
    Base64(String),
    Url(String),
}

impl GeneratedImageCandidate {
    pub(crate) fn from_rig_image(ordinal: usize, image: &Image) -> ArtifactResult<Self> {
        let marked = image
            .additional_params
            .as_ref()
            .and_then(|params| params.wire_extras("openrouter"))
            .is_some_and(|params| {
                params
                    .get("response_only")
                    .and_then(|value| value.as_bool())
                    == Some(true)
                    && params.get("source").and_then(|value| value.as_str())
                        == Some("assistant.images")
            });
        if !marked {
            return Err(GeneratedArtifactError::invalid());
        }
        let source = match &image.data {
            DocumentSourceKind::Base64(payload) => GeneratedImageSource::Base64(payload.clone()),
            DocumentSourceKind::Url(locator) => GeneratedImageSource::Url(locator.clone()),
            DocumentSourceKind::FileId(_)
            | DocumentSourceKind::Raw(_)
            | DocumentSourceKind::String(_)
            | DocumentSourceKind::Unknown => return Err(GeneratedArtifactError::invalid()),
        };
        Ok(Self {
            ordinal,
            source,
            declared_mime: image.media_type.as_ref().map(image_media_type),
        })
    }
}

#[derive(Debug)]
pub(crate) struct PreparedGeneratedImage {
    pub(crate) attachment: NewAttachment,
    pub(crate) final_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct PreparedGeneratedArtifacts {
    pub(crate) images: Vec<PreparedGeneratedImage>,
    cleanup_paths: Vec<PathBuf>,
    armed: bool,
}

impl PreparedGeneratedArtifacts {
    fn armed() -> Self {
        Self {
            images: Vec::new(),
            cleanup_paths: Vec::new(),
            armed: true,
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
        self.cleanup_paths.clear();
    }

    pub(crate) async fn rollback(&mut self) {
        for path in self.cleanup_paths.drain(..).rev() {
            let _ = fs::remove_file(path).await;
        }
        self.armed = false;
    }

    pub(crate) async fn preserve_only(&mut self, referenced: &std::collections::HashSet<String>) {
        for path in self.cleanup_paths.drain(..) {
            let keep = self
                .images
                .iter()
                .any(|image| image.final_path == path && referenced.contains(&image.attachment.id));
            if !keep {
                let _ = fs::remove_file(path).await;
            }
        }
        self.armed = false;
    }
}

impl Drop for PreparedGeneratedArtifacts {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for path in self.cleanup_paths.drain(..).rev() {
            let _ = std::fs::remove_file(path);
        }
    }
}

type ArtifactResult<T> = std::result::Result<T, GeneratedArtifactError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeneratedArtifactErrorKind {
    Invalid,
    Limit,
    Download,
    Storage,
}

#[derive(Debug, thiserror::Error)]
#[error("generated artifact processing failed")]
pub(crate) struct GeneratedArtifactError {
    kind: GeneratedArtifactErrorKind,
}

impl GeneratedArtifactError {
    fn invalid() -> Self {
        Self {
            kind: GeneratedArtifactErrorKind::Invalid,
        }
    }

    fn limit() -> Self {
        Self {
            kind: GeneratedArtifactErrorKind::Limit,
        }
    }

    fn download(_error: impl std::fmt::Display) -> Self {
        Self {
            kind: GeneratedArtifactErrorKind::Download,
        }
    }

    fn storage(_error: impl std::fmt::Display) -> Self {
        Self {
            kind: GeneratedArtifactErrorKind::Storage,
        }
    }

    pub(crate) fn run_error(&self) -> RunErrorPayload {
        let (code, message, retryable) = match self.kind {
            GeneratedArtifactErrorKind::Invalid => (
                "generated_artifact_invalid",
                "generated image response was invalid",
                false,
            ),
            GeneratedArtifactErrorKind::Limit => (
                "generated_artifact_limit_exceeded",
                "generated image exceeded a safety limit",
                false,
            ),
            GeneratedArtifactErrorKind::Download => (
                "generated_artifact_download_failed",
                "generated image download failed",
                true,
            ),
            GeneratedArtifactErrorKind::Storage => (
                "generated_artifact_storage_failed",
                "generated image storage failed",
                true,
            ),
        };
        RunErrorPayload {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
            provider: Some("openrouter".to_string()),
            raw: None,
        }
    }
}

fn cancellation_check(cancellation: &CancellationToken) -> ArtifactResult<()> {
    if cancellation.is_cancelled() {
        Err(GeneratedArtifactError::storage(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "artifact processing canceled",
        )))
    } else {
        Ok(())
    }
}

async fn ensure_directory(path: &Path) -> ArtifactResult<()> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(GeneratedArtifactError::storage(std::io::Error::other(
                "managed artifact directory is unsafe",
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path)
                .await
                .map_err(GeneratedArtifactError::storage)?;
            let metadata = fs::symlink_metadata(path)
                .await
                .map_err(GeneratedArtifactError::storage)?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(GeneratedArtifactError::storage(std::io::Error::other(
                    "managed artifact directory is unsafe",
                )));
            }
            Ok(())
        }
        Err(error) => Err(GeneratedArtifactError::storage(error)),
    }
}

async fn reject_existing_path(path: &Path) -> ArtifactResult<()> {
    match fs::symlink_metadata(path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(GeneratedArtifactError::storage(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "managed artifact target already exists",
        ))),
        Err(error) => Err(GeneratedArtifactError::storage(error)),
    }
}

async fn require_regular_file(path: &Path) -> ArtifactResult<()> {
    let metadata = fs::symlink_metadata(path)
        .await
        .map_err(GeneratedArtifactError::storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GeneratedArtifactError::storage(std::io::Error::other(
            "managed artifact is not a regular file",
        )));
    }
    Ok(())
}

async fn write_base64(payload: &str, path: &Path, max_bytes: u64) -> ArtifactResult<()> {
    let max_encoded = max_bytes
        .checked_add(2)
        .and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(GeneratedArtifactError::limit)?;
    if u64::try_from(payload.len()).map_err(|_| GeneratedArtifactError::limit())? > max_encoded {
        return Err(GeneratedArtifactError::limit());
    }
    let decoded = general_purpose::STANDARD
        .decode(payload)
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(payload))
        .map_err(|_| GeneratedArtifactError::invalid())?;
    if u64::try_from(decoded.len()).map_err(|_| GeneratedArtifactError::limit())? > max_bytes {
        return Err(GeneratedArtifactError::limit());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .map_err(GeneratedArtifactError::storage)?;
    file.write_all(&decoded)
        .await
        .map_err(GeneratedArtifactError::storage)?;
    file.flush()
        .await
        .map_err(GeneratedArtifactError::storage)?;
    file.sync_all()
        .await
        .map_err(GeneratedArtifactError::storage)
}

async fn download_url(
    locator: &str,
    path: &Path,
    max_bytes: u64,
    cancellation: &CancellationToken,
) -> ArtifactResult<Option<String>> {
    let mut url = validate_artifact_url(locator)?;
    for redirects in 0..=MAX_REDIRECTS {
        cancellation_check(cancellation)?;
        let hop = async {
            let host = url.host_str().ok_or_else(GeneratedArtifactError::invalid)?;
            let addresses = resolve_public_addresses(&url).await?;
            cancellation_check(cancellation)?;
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(CONNECT_TIMEOUT)
                .resolve_to_addrs(host, &addresses)
                .build()
                .map_err(GeneratedArtifactError::download)?;
            let mut response = client
                .get(url.clone())
                .header(header::ACCEPT_ENCODING, "identity")
                .send()
                .await
                .map_err(GeneratedArtifactError::download)?;
            if response.status().is_redirection() {
                if redirects == MAX_REDIRECTS {
                    return Err(GeneratedArtifactError::download("too many redirects"));
                }
                let location = response
                    .headers()
                    .get(header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| GeneratedArtifactError::download("invalid redirect"))?;
                let next = url
                    .join(location)
                    .map_err(GeneratedArtifactError::download)?;
                return Ok(DownloadHop::Redirect(validate_artifact_url(next.as_str())?));
            }
            if !response.status().is_success() {
                return Err(GeneratedArtifactError::download("unexpected HTTP status"));
            }
            if let Some(length) = response.content_length()
                && length > max_bytes
            {
                return Err(GeneratedArtifactError::limit());
            }
            let declared_mime = parse_content_type(response.headers().get(header::CONTENT_TYPE))?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .await
                .map_err(GeneratedArtifactError::storage)?;
            let mut written = 0_u64;
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(GeneratedArtifactError::download)?
            {
                cancellation_check(cancellation)?;
                written = written
                    .checked_add(
                        u64::try_from(chunk.len()).map_err(|_| GeneratedArtifactError::limit())?,
                    )
                    .ok_or_else(GeneratedArtifactError::limit)?;
                if written > max_bytes {
                    return Err(GeneratedArtifactError::limit());
                }
                file.write_all(&chunk)
                    .await
                    .map_err(GeneratedArtifactError::storage)?;
            }
            file.flush()
                .await
                .map_err(GeneratedArtifactError::storage)?;
            file.sync_all()
                .await
                .map_err(GeneratedArtifactError::storage)?;
            Ok(DownloadHop::Complete(declared_mime))
        };
        match tokio::time::timeout(DOWNLOAD_TIMEOUT, hop)
            .await
            .map_err(GeneratedArtifactError::download)??
        {
            DownloadHop::Redirect(next) => url = next,
            DownloadHop::Complete(mime) => return Ok(mime),
        }
    }
    Err(GeneratedArtifactError::download("too many redirects"))
}

enum DownloadHop {
    Redirect(Url),
    Complete(Option<String>),
}

fn validate_artifact_url(locator: &str) -> ArtifactResult<Url> {
    let url = Url::parse(locator).map_err(|_| GeneratedArtifactError::invalid())?;
    if url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(GeneratedArtifactError::invalid());
    }
    let host = url.host().ok_or_else(GeneratedArtifactError::invalid)?;
    match host {
        Host::Ipv4(address) if !is_public_artifact_ip(IpAddr::V4(address)) => {
            Err(GeneratedArtifactError::invalid())
        }
        Host::Ipv6(address) if !is_public_artifact_ip(IpAddr::V6(address)) => {
            Err(GeneratedArtifactError::invalid())
        }
        _ => Ok(url),
    }
}

async fn resolve_public_addresses(url: &Url) -> ArtifactResult<Vec<SocketAddr>> {
    let host = url.host_str().ok_or_else(GeneratedArtifactError::invalid)?;
    let addresses = tokio::net::lookup_host((host, 443))
        .await
        .map_err(GeneratedArtifactError::download)?
        .collect::<Vec<_>>();
    validate_resolved_addresses(&addresses)?;
    Ok(addresses)
}

fn validate_resolved_addresses(addresses: &[SocketAddr]) -> ArtifactResult<()> {
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !is_public_artifact_ip(address.ip()))
    {
        return Err(GeneratedArtifactError::invalid());
    }
    Ok(())
}

pub(crate) fn is_public_artifact_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => {
            if address.to_ipv4().is_some() {
                return false;
            }
            is_public_ipv6(address)
        }
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 88 && c == 99)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    let segments = address.segments();
    !(address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xffc0) == 0xfec0
        || (segments[0] == 0x0064 && segments[1] == 0xff9b)
        || (segments[0] == 0x0100 && segments[1..4] == [0, 0, 0])
        || (segments[0] == 0x2001 && segments[1] <= 0x01ff)
        || (segments[0] == 0x2001 && segments[1] == 0x0db8)
        || segments[0] == 0x2002
        || (segments[0] == 0x3fff && (segments[1] & 0xf000) == 0))
}

#[derive(Debug)]
struct InspectedImage {
    extension: &'static str,
    mime_type: &'static str,
    width: u32,
    height: u32,
    size_bytes: u64,
    sha256: String,
}

async fn inspect_image(
    path: PathBuf,
    declared_mimes: Vec<String>,
    limits: GeneratedImageLimits,
) -> ArtifactResult<InspectedImage> {
    tokio::task::spawn_blocking(move || inspect_image_blocking(&path, declared_mimes, limits))
        .await
        .map_err(GeneratedArtifactError::storage)?
}

fn inspect_image_blocking(
    path: &Path,
    declared_mimes: Vec<String>,
    limits: GeneratedImageLimits,
) -> ArtifactResult<InspectedImage> {
    let metadata = std::fs::symlink_metadata(path).map_err(GeneratedArtifactError::storage)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(GeneratedArtifactError::storage(std::io::Error::other(
            "managed artifact stage is not a regular file",
        )));
    }
    let size_bytes = metadata.len();
    if size_bytes > limits.max_image_bytes {
        return Err(GeneratedArtifactError::limit());
    }
    let reader = ImageReader::new(BufReader::new(
        std::fs::File::open(path).map_err(GeneratedArtifactError::storage)?,
    ))
    .with_guessed_format()
    .map_err(GeneratedArtifactError::storage)?;
    let format = reader
        .format()
        .ok_or_else(GeneratedArtifactError::invalid)?;
    let (extension, mime_type) = canonical_format(format)?;
    if declared_mimes
        .iter()
        .any(|declared_mime| normalize_mime(declared_mime) != mime_type)
    {
        return Err(GeneratedArtifactError::invalid());
    }
    let (width, height) = decode_first_frame(path, format, limits)?;
    let mut file = std::fs::File::open(path).map_err(GeneratedArtifactError::storage)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(GeneratedArtifactError::storage)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(InspectedImage {
        extension,
        mime_type,
        width,
        height,
        size_bytes,
        sha256: hex::encode(digest.finalize()),
    })
}

fn decode_first_frame(
    path: &Path,
    format: ImageFormat,
    limits: GeneratedImageLimits,
) -> ArtifactResult<(u32, u32)> {
    let mut dimensions_reader = ImageReader::new(BufReader::new(
        std::fs::File::open(path).map_err(GeneratedArtifactError::storage)?,
    ));
    dimensions_reader.set_format(format);
    dimensions_reader.limits(decoder_limits(limits));
    let dimensions = dimensions_reader
        .into_dimensions()
        .map_err(image_decode_error)?;
    validate_dimensions(dimensions, limits)?;

    let mut frame_reader = ImageReader::new(BufReader::new(
        std::fs::File::open(path).map_err(GeneratedArtifactError::storage)?,
    ));
    frame_reader.set_format(format);
    frame_reader.limits(decoder_limits(limits));
    let decoded_dimensions = frame_reader
        .decode()
        .map_err(image_decode_error)?
        .dimensions();
    if decoded_dimensions != dimensions {
        return Err(GeneratedArtifactError::invalid());
    }
    Ok(dimensions)
}

fn decoder_limits(limits: GeneratedImageLimits) -> Limits {
    let mut decoder_limits = Limits::default();
    decoder_limits.max_image_width = Some(limits.max_dimension);
    decoder_limits.max_image_height = Some(limits.max_dimension);
    decoder_limits.max_alloc = Some(limits.max_decoder_output_bytes);
    decoder_limits
}

fn image_decode_error(error: image::ImageError) -> GeneratedArtifactError {
    match error {
        image::ImageError::Limits(_) => GeneratedArtifactError::limit(),
        _ => GeneratedArtifactError::invalid(),
    }
}

fn validate_dimensions(
    (width, height): (u32, u32),
    limits: GeneratedImageLimits,
) -> ArtifactResult<()> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(GeneratedArtifactError::limit)?;
    if width > limits.max_dimension || height > limits.max_dimension || pixels > limits.max_pixels {
        return Err(GeneratedArtifactError::limit());
    }
    Ok(())
}

fn canonical_format(format: ImageFormat) -> ArtifactResult<(&'static str, &'static str)> {
    match format {
        ImageFormat::Png => Ok(("png", "image/png")),
        ImageFormat::Jpeg => Ok(("jpg", "image/jpeg")),
        ImageFormat::Gif => Ok(("gif", "image/gif")),
        ImageFormat::WebP => Ok(("webp", "image/webp")),
        _ => Err(GeneratedArtifactError::invalid()),
    }
}

fn image_media_type(media_type: &ImageMediaType) -> String {
    match media_type {
        ImageMediaType::JPEG => "image/jpeg",
        ImageMediaType::PNG => "image/png",
        ImageMediaType::GIF => "image/gif",
        ImageMediaType::WEBP => "image/webp",
        ImageMediaType::HEIC => "image/heic",
        ImageMediaType::HEIF => "image/heif",
        ImageMediaType::SVG => "image/svg+xml",
    }
    .to_string()
}

fn normalize_mime(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

fn parse_content_type(value: Option<&header::HeaderValue>) -> ArtifactResult<Option<String>> {
    match value {
        None => Ok(None),
        Some(value) => value
            .to_str()
            .map(normalize_mime)
            .map(Some)
            .map_err(|_| GeneratedArtifactError::invalid()),
    }
}

#[cfg(not(windows))]
async fn sync_directory(path: &Path) -> ArtifactResult<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(GeneratedArtifactError::storage)
    })
    .await
    .map_err(GeneratedArtifactError::storage)?
}

#[cfg(windows)]
async fn sync_directory(_path: &Path) -> ArtifactResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{AnimationDecoder as _, DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;
    use tempfile::tempdir;

    fn png_bytes() -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 3, Rgba([1, 2, 3, 4])));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    fn image_bytes(format: ImageFormat) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 3, Rgba([1, 2, 3, 255])));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        bytes.into_inner()
    }

    fn animated_gif_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            encoder
                .encode_frame(image::Frame::new(ImageBuffer::from_pixel(
                    2,
                    2,
                    Rgba([255, 0, 0, 255]),
                )))
                .unwrap();
            encoder
                .encode_frame(image::Frame::new(ImageBuffer::from_pixel(
                    2,
                    2,
                    Rgba([0, 255, 0, 255]),
                )))
                .unwrap();
        }
        bytes
    }

    fn animated_webp_bytes() -> Vec<u8> {
        // 2x2 lossless red/blue two-frame WebP generated with img2webp.
        general_purpose::STANDARD
            .decode(
                "UklGRoQAAABXRUJQVlA4WAoAAAACAAAAAQAAAQAAQU5JTQYAAAD/////AABBTk1GKAAAAAAAAAAAAAEAAAEAAGQAAAJWUDhMDwAAAC8BQAAABxDlj/4HIqL/AQBBTk1GKAAAAAAAAAAAAAEAAAEAAGQAAABWUDhMDwAAAC8BQAAABxDR/v4HIqL/AQA=",
            )
            .unwrap()
    }

    #[test]
    fn public_ip_policy_rejects_special_ranges() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "100.64.0.1",
            "169.254.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "203.0.113.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "3fff::1",
            "2002:0808:0808::1",
            "::8.8.8.8",
            "::ffff:127.0.0.1",
            "::ffff:8.8.8.8",
        ] {
            assert!(
                !is_public_artifact_ip(address.parse().unwrap()),
                "{address}"
            );
        }
        assert!(is_public_artifact_ip("8.8.8.8".parse().unwrap()));
        assert!(is_public_artifact_ip(
            "2606:4700:4700::1111".parse().unwrap()
        ));

        let mixed = [
            "8.8.8.8:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap(),
        ];
        assert!(validate_resolved_addresses(&mixed).is_err());
        assert!(validate_resolved_addresses(&[]).is_err());
    }

    #[test]
    fn url_policy_requires_safe_https() {
        for locator in [
            "http://example.com/image.png",
            "https://user@example.com/image.png",
            "https://example.com:444/image.png",
            "https://example.com/image.png#secret",
            "https://127.0.0.1/image.png",
        ] {
            assert!(validate_artifact_url(locator).is_err(), "{locator}");
        }
        assert!(validate_artifact_url("https://example.com/image.png").is_ok());
    }

    #[tokio::test]
    async fn base64_materialization_uses_reserved_filename_and_records_exact_metadata() {
        let directory = tempdir().unwrap();
        let conversation_dir = directory.path().join("attachments").join("conversation-1");
        let store = ManagedArtifactStore::new("conversation-1".to_string(), conversation_dir);
        let bytes = png_bytes();
        let expected_hash = hex::encode(Sha256::digest(&bytes));
        let mut prepared = store
            .prepare(
                vec![GeneratedImageCandidate {
                    ordinal: 1,
                    source: GeneratedImageSource::Base64(general_purpose::STANDARD.encode(&bytes)),
                    declared_mime: Some("image/png".to_string()),
                }],
                &"provider-1".to_string(),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(prepared.images.len(), 1);
        let final_path = prepared.images[0].final_path.clone();
        let attachment = &prepared.images[0].attachment;
        assert_eq!(attachment.mime_type.as_deref(), Some("image/png"));
        assert_eq!(attachment.sha256.as_deref(), Some(expected_hash.as_str()));
        assert_eq!(attachment.size_bytes, Some(bytes.len() as i64));
        assert_eq!(attachment.metadata.width, Some(2));
        assert_eq!(attachment.metadata.height, Some(3));
        assert!(final_path.is_file());
        assert_eq!(
            final_path.file_name().and_then(std::ffi::OsStr::to_str),
            Some(format!("{GENERATED_FILE_PREFIX}{}.png", attachment.id).as_str())
        );
        prepared.rollback().await;
        assert!(!final_path.is_file());
    }

    #[tokio::test]
    async fn base64_mime_mismatch_and_size_limit_remove_stage() {
        let directory = tempdir().unwrap();
        let conversation_dir = directory.path().join("attachments").join("conversation-1");
        let store =
            ManagedArtifactStore::new("conversation-1".to_string(), conversation_dir.clone())
                .with_limits(GeneratedImageLimits {
                    max_image_bytes: 8,
                    ..GeneratedImageLimits::production()
                });
        let error = store
            .prepare(
                vec![GeneratedImageCandidate {
                    ordinal: 1,
                    source: GeneratedImageSource::Base64(
                        general_purpose::STANDARD.encode(png_bytes()),
                    ),
                    declared_mime: Some("image/jpeg".to_string()),
                }],
                &"provider-1".to_string(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Limit);
        let pending = conversation_dir.join(".pending");
        assert_eq!(std::fs::read_dir(pending).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn base64_rejects_data_uri_whitespace_and_mime_mismatch() {
        let directory = tempdir().unwrap();
        for (index, payload) in [
            "data:image/png;base64,AAAA".to_string(),
            "AAAA\n".to_string(),
        ]
        .into_iter()
        .enumerate()
        {
            let path = directory.path().join(format!("invalid-{index}.part"));
            let error = write_base64(&payload, &path, MAX_IMAGE_BYTES)
                .await
                .unwrap_err();
            assert_eq!(error.kind, GeneratedArtifactErrorKind::Invalid);
            assert!(!path.exists());
        }

        let path = directory.path().join("mismatch.png");
        std::fs::write(&path, png_bytes()).unwrap();
        let error = inspect_image_blocking(
            &path,
            vec!["image/jpeg".to_string(), "image/png".to_string()],
            GeneratedImageLimits::production(),
        )
        .unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Invalid);
    }

    #[test]
    fn allowed_formats_decode_first_frame_and_apply_limits() {
        let directory = tempdir().unwrap();
        for (format, extension, mime) in [
            (ImageFormat::Png, "png", "image/png"),
            (ImageFormat::Jpeg, "jpg", "image/jpeg"),
            (ImageFormat::WebP, "webp", "image/webp"),
        ] {
            let path = directory.path().join(extension);
            std::fs::write(&path, image_bytes(format)).unwrap();
            let inspected = inspect_image_blocking(
                &path,
                vec![mime.to_string()],
                GeneratedImageLimits::production(),
            )
            .unwrap_or_else(|error| panic!("{extension}: {error:?}"));
            assert_eq!((inspected.width, inspected.height), (2, 3));
        }

        let gif_path = directory.path().join("animated.gif");
        std::fs::write(&gif_path, animated_gif_bytes()).unwrap();
        let inspected = inspect_image_blocking(
            &gif_path,
            vec!["image/gif".to_string()],
            GeneratedImageLimits::production(),
        )
        .unwrap();
        assert_eq!((inspected.width, inspected.height), (2, 2));

        let limited = GeneratedImageLimits {
            max_pixels: 3,
            ..GeneratedImageLimits::production()
        };
        let error = inspect_image_blocking(&gif_path, Vec::new(), limited).unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Limit);
    }

    #[test]
    fn animated_gif_corruption_after_first_frame_is_deferred() {
        let valid = animated_gif_bytes();
        let directory = tempdir().unwrap();
        let path = directory.path().join("truncated.gif");
        let mut found_late_corruption = None;
        for cut in (valid.len() / 2)..valid.len() {
            let truncated = &valid[..cut];
            let first_frame_decodes = ImageReader::new(Cursor::new(truncated))
                .with_guessed_format()
                .ok()
                .and_then(|reader| reader.decode().ok())
                .is_some();
            let all_frames_decode = image::codecs::gif::GifDecoder::new(Cursor::new(truncated))
                .ok()
                .map(|decoder| decoder.into_frames().all(|frame| frame.is_ok()))
                .unwrap_or(false);
            if first_frame_decodes && !all_frames_decode {
                found_late_corruption = Some(truncated.to_vec());
                break;
            }
        }
        let truncated = found_late_corruption.expect("fixture corrupts only a later GIF frame");
        std::fs::write(&path, truncated).unwrap();
        let inspected = inspect_image_blocking(
            &path,
            vec!["image/gif".to_string()],
            GeneratedImageLimits::production(),
        )
        .unwrap();
        assert_eq!((inspected.width, inspected.height), (2, 2));
    }

    #[test]
    fn animated_webp_decodes_first_frame_and_defers_late_corruption() {
        let valid = animated_webp_bytes();
        let directory = tempdir().unwrap();
        let path = directory.path().join("animated.webp");
        std::fs::write(&path, &valid).unwrap();
        let inspected = inspect_image_blocking(
            &path,
            vec!["image/webp".to_string()],
            GeneratedImageLimits::production(),
        )
        .unwrap();
        assert_eq!((inspected.width, inspected.height), (2, 2));

        let limited = GeneratedImageLimits {
            max_pixels: 3,
            ..GeneratedImageLimits::production()
        };
        let error = inspect_image_blocking(&path, Vec::new(), limited).unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Limit);

        let corrupted = (valid.len() / 2..valid.len())
            .find_map(|index| {
                let mut corrupted = valid.clone();
                corrupted[index] ^= 0xff;
                let first_frame_decodes = ImageReader::new(Cursor::new(&corrupted))
                    .with_guessed_format()
                    .ok()
                    .and_then(|reader| reader.decode().ok())
                    .is_some();
                let all_frames_decode =
                    image::codecs::webp::WebPDecoder::new(Cursor::new(&corrupted))
                        .ok()
                        .map(|decoder| decoder.into_frames().all(|frame| frame.is_ok()))
                        .unwrap_or(false);
                (first_frame_decodes && !all_frames_decode).then_some(corrupted)
            })
            .expect("fixture corrupts only a later WebP frame");
        std::fs::write(&path, corrupted).unwrap();
        let inspected = inspect_image_blocking(
            &path,
            vec!["image/webp".to_string()],
            GeneratedImageLimits::production(),
        )
        .unwrap();
        assert_eq!((inspected.width, inspected.height), (2, 2));
    }

    #[test]
    fn invalid_content_type_header_is_not_treated_as_absent() {
        let invalid = header::HeaderValue::from_bytes(&[0xff]).unwrap();
        let error = parse_content_type(Some(&invalid)).unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Invalid);
        assert_eq!(parse_content_type(None).unwrap(), None);
    }

    #[tokio::test]
    async fn count_and_response_aggregate_limits_fail_without_durable_files() {
        let directory = tempdir().unwrap();
        let conversation_dir = directory.path().join("attachments").join("conversation-1");
        let bytes = png_bytes();
        let candidate = || GeneratedImageCandidate {
            ordinal: 1,
            source: GeneratedImageSource::Base64(general_purpose::STANDARD.encode(&bytes)),
            declared_mime: Some("image/png".to_string()),
        };
        let store =
            ManagedArtifactStore::new("conversation-1".to_string(), conversation_dir.clone());
        let error = store
            .prepare(
                (0..=MAX_IMAGES).map(|_| candidate()).collect(),
                &"provider-1".to_string(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Limit);
        assert!(!conversation_dir.exists());

        let store = store.with_limits(GeneratedImageLimits {
            max_response_bytes: bytes.len() as u64,
            ..GeneratedImageLimits::production()
        });
        let error = store
            .prepare(
                vec![candidate(), candidate()],
                &"provider-1".to_string(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Limit);
        let durable = std::fs::read_dir(&conversation_dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name() != ".pending")
            .count();
        assert_eq!(durable, 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinked_pending_directory_is_rejected() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().unwrap();
        let conversation_dir = directory.path().join("attachments").join("conversation-1");
        std::fs::create_dir_all(&conversation_dir).unwrap();
        let target = directory.path().join("outside");
        std::fs::create_dir(&target).unwrap();
        symlink(&target, conversation_dir.join(".pending")).unwrap();
        let store = ManagedArtifactStore::new("conversation-1".to_string(), conversation_dir);
        let error = store
            .prepare(
                vec![GeneratedImageCandidate {
                    ordinal: 1,
                    source: GeneratedImageSource::Base64(
                        general_purpose::STANDARD.encode(png_bytes()),
                    ),
                    declared_mime: Some("image/png".to_string()),
                }],
                &"provider-1".to_string(),
                &CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, GeneratedArtifactErrorKind::Storage);
        assert_eq!(std::fs::read_dir(target).unwrap().count(), 0);
    }
}
