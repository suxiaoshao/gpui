use std::fs;
use std::path::Path;

use crate::error::{Result, XtaskError};

pub(crate) fn prepare_bundle_icons(app_dir: &Path) -> Result<()> {
    let icon_dir = app_dir.join("build-assets/icon");
    let src_png = icon_dir.join("app-icon.png");
    let iconset_dir = app_dir.join("build-assets/icon/app-icon.iconset");

    if !src_png.exists() {
        return Err(XtaskError::msg(format!(
            "missing bundle base icon {}",
            src_png.display()
        )));
    }

    fs::create_dir_all(&iconset_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create iconset dir {}: {err}",
            iconset_dir.display()
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

    let ico_image = source_image
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    image::DynamicImage::ImageRgba8(ico_image)
        .save(icon_dir.join("app-icon.ico"))
        .map_err(|err| XtaskError::msg(format!("failed to save app icon ico: {err}")))?;

    Ok(())
}
