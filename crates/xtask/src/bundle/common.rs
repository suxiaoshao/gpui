use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
    temp_dir: PathBuf,
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

impl Drop for BundleIconAssets {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
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

    let temp_dir = std::env::temp_dir().join(format!(
        "xtask-bundle-icons-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| XtaskError::msg(format!("failed to read system time: {err}")))?
            .as_millis()
    ));
    let assets = BundleIconAssets {
        staged_icon_dir: temp_dir.join("build-assets/icon"),
        temp_dir,
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Result<Self> {
            let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "xtask-bundle-common-{suffix}-{}",
                std::process::id()
            ));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[allow(deprecated)]
    #[test]
    fn prepare_bundle_icons_stages_generated_files_outside_app_dir() -> Result<()> {
        let temp_dir = TestDir::new()?;
        let app_dir = temp_dir.path.join("app/demo");
        let source_icon_dir = app_dir.join("build-assets/icon");
        fs::create_dir_all(&source_icon_dir)?;

        let icon = RgbaImage::from_pixel(32, 32, Rgba([20, 40, 80, 255]));
        icon.save(source_icon_dir.join("app-icon.png"))?;

        let assets = prepare_bundle_icons(&app_dir)?;

        assert!(!source_icon_dir.join("app-icon.ico").exists());
        assert!(!source_icon_dir.join("app-icon.iconset").exists());
        assert!(assets.staged_icon_dir.join("app-icon.ico").exists());
        assert_eq!(
            ico_frame_count(&assets.staged_icon_dir.join("app-icon.ico"))?,
            ICO_FRAME_SIZES.len()
        );
        assert!(
            assets
                .staged_icon_dir
                .join("app-icon.iconset/icon_32x32.png")
                .exists()
        );

        let mut bundle_settings = BundleSettings::default();

        assets.apply_to_bundle_settings(&mut bundle_settings);

        let icon_paths = bundle_settings.icon.expect("icon paths");
        assert_eq!(icon_paths.len(), 8);
        assert!(Path::new(&icon_paths[0]).starts_with(&assets.staged_icon_dir));
        assert!(Path::new(&icon_paths[1]).starts_with(&assets.staged_icon_dir));
        assert!(
            bundle_settings
                .windows
                .icon_path
                .starts_with(&assets.staged_icon_dir)
        );

        Ok(())
    }

    fn ico_frame_count(path: &Path) -> Result<usize> {
        let bytes = fs::read(path)?;
        Ok(u16::from_le_bytes([bytes[4], bytes[5]]) as usize)
    }
}
