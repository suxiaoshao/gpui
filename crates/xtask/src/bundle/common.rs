use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use crate::error::{Result, XtaskError};
use image::codecs::ico::{IcoEncoder, IcoFrame};
use tauri_bundler::BundleSettings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BundleIconOutputKind {
    WindowsIco,
    MacOsIconset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BundleIconOutput {
    relative_path: &'static str,
    kind: BundleIconOutputKind,
}

const BUNDLE_ICON_OUTPUTS: [BundleIconOutput; 8] = [
    BundleIconOutput {
        relative_path: "app-icon.ico",
        kind: BundleIconOutputKind::WindowsIco,
    },
    BundleIconOutput {
        relative_path: "app-icon.iconset/icon_32x32.png",
        kind: BundleIconOutputKind::MacOsIconset,
    },
    BundleIconOutput {
        relative_path: "app-icon.iconset/icon_128x128.png",
        kind: BundleIconOutputKind::MacOsIconset,
    },
    BundleIconOutput {
        relative_path: "app-icon.iconset/icon_128x128@2x.png",
        kind: BundleIconOutputKind::MacOsIconset,
    },
    BundleIconOutput {
        relative_path: "app-icon.iconset/icon_256x256.png",
        kind: BundleIconOutputKind::MacOsIconset,
    },
    BundleIconOutput {
        relative_path: "app-icon.iconset/icon_256x256@2x.png",
        kind: BundleIconOutputKind::MacOsIconset,
    },
    BundleIconOutput {
        relative_path: "app-icon.iconset/icon_512x512.png",
        kind: BundleIconOutputKind::MacOsIconset,
    },
    BundleIconOutput {
        relative_path: "app-icon.iconset/icon_512x512@2x.png",
        kind: BundleIconOutputKind::MacOsIconset,
    },
];
const ICO_FRAME_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

pub(crate) struct BundleIconAssets {
    _temp_dir: tempfile::TempDir,
    #[cfg(target_os = "macos")]
    source_icon_dir: PathBuf,
    staged_icon_dir: PathBuf,
}

impl BundleIconAssets {
    #[cfg(target_os = "macos")]
    pub(crate) fn source_base_icon(&self) -> PathBuf {
        self.source_icon_dir.join("app-icon.png")
    }

    pub(crate) fn apply_to_bundle_settings(&self, bundle_settings: &mut BundleSettings) {
        bundle_settings.icon = Some(
            self.bundle_icon_paths()
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        );

        #[allow(deprecated)]
        {
            bundle_settings.windows.icon_path = self.windows_icon_path();
        }
    }

    fn windows_icon_path(&self) -> PathBuf {
        BUNDLE_ICON_OUTPUTS
            .into_iter()
            .find(|output| output.kind == BundleIconOutputKind::WindowsIco)
            .map(|output| self.staged_icon_dir.join(output.relative_path))
            .expect("bundle icon outputs should include a Windows ICO")
    }

    fn bundle_icon_paths(&self) -> Vec<PathBuf> {
        BUNDLE_ICON_OUTPUTS
            .into_iter()
            .map(|output| self.staged_icon_dir.join(output.relative_path))
            .collect()
    }
}

pub(crate) fn prepare_bundle_icons(app_dir: &Path) -> Result<BundleIconAssets> {
    let source_icon_dir = app_dir.join("build-assets/icon");
    let src_png = source_icon_dir.join("app-icon.png");

    if !src_png.exists() {
        return Err(XtaskError::msg(format!(
            "missing bundle base icon {}",
            src_png.display()
        )));
    }

    let temp_dir = tempfile::Builder::new()
        .prefix("xtask-bundle-icons-")
        .tempdir()
        .map_err(|err| {
            XtaskError::msg(format!("failed to create icon staging directory: {err}"))
        })?;
    let assets = BundleIconAssets {
        staged_icon_dir: temp_dir.path().join("build-assets/icon"),
        _temp_dir: temp_dir,
        #[cfg(target_os = "macos")]
        source_icon_dir: source_icon_dir.clone(),
    };

    let iconset_dir = assets.staged_icon_dir.join("app-icon.iconset");
    fs::create_dir_all(&iconset_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create iconset dir {}: {err}",
            iconset_dir.display()
        ))
    })?;
    fs::copy(&src_png, assets.staged_icon_dir.join("app-icon.png")).map_err(|err| {
        XtaskError::msg(format!(
            "failed to copy {} to staged bundle icon dir {}: {err}",
            src_png.display(),
            assets.staged_icon_dir.display()
        ))
    })?;

    let source_image = image::ImageReader::open(&src_png)
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to open source icon {}: {err}",
                src_png.display()
            ))
        })?
        .decode()
        .map_err(|err| {
            XtaskError::msg(format!(
                "failed to decode source icon {}: {err}",
                src_png.display()
            ))
        })?;

    for size in [16_u32, 32, 128, 256, 512] {
        let base = format!("icon_{size}x{size}.png");
        let retina = format!("icon_{size}x{size}@2x.png");

        let base_image =
            source_image.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
        base_image
            .save(iconset_dir.join(base))
            .map_err(|err| XtaskError::msg(format!("failed to save iconset image: {err}")))?;

        let doubled = size * 2;
        let retina_image =
            source_image.resize_exact(doubled, doubled, image::imageops::FilterType::Lanczos3);
        retina_image
            .save(iconset_dir.join(retina))
            .map_err(|err| XtaskError::msg(format!("failed to save iconset image: {err}")))?;
    }

    save_windows_ico(&source_image, &assets.windows_icon_path())
        .map_err(|err| XtaskError::msg(format!("failed to save app icon ico: {err}")))?;

    Ok(assets)
}

fn save_windows_ico(source_image: &image::DynamicImage, path: &Path) -> Result<()> {
    let mut frames = Vec::with_capacity(ICO_FRAME_SIZES.len());
    for size in ICO_FRAME_SIZES {
        let image = source_image
            .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        frames.push(IcoFrame::as_png(
            image.as_raw(),
            size,
            size,
            image::ExtendedColorType::Rgba8,
        )?);
    }

    let file = File::create(path)?;
    IcoEncoder::new(BufWriter::new(file)).encode_images(&frames)?;
    Ok(())
}
