#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]

fn main() {
    #[cfg(target_os = "windows")]
    windows::main();
}

#[cfg(target_os = "windows")]
mod windows {
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    use std::fs::File;
    use std::io::BufWriter;
    use std::path::{Path, PathBuf};

    const ICO_FRAME_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

    pub(super) fn main() {
        let mut res = winresource::WindowsResource::new();
        if let Ok(toolkit_path) = std::env::var("AI_CHAT2_RC_TOOLKIT_PATH") {
            res.set_toolkit_path(toolkit_path.as_str());
        }

        if let Some(icon) = windows_icon_path() {
            if let Some(icon) = icon.to_str() {
                res.set_icon(icon);
            } else {
                println!(
                    "cargo:warning=AI Chat 2 icon path is not valid UTF-8: {}; building without app icon",
                    icon.display()
                );
            }
        }

        res.set("FileDescription", "AI Chat 2");
        res.set("ProductName", "AI Chat 2");

        if let Err(err) = res.compile() {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }

    fn windows_icon_path() -> Option<PathBuf> {
        println!("cargo:rerun-if-env-changed=AI_CHAT2_ICON_PATH");

        if let Ok(icon_path) = std::env::var("AI_CHAT2_ICON_PATH") {
            let icon = PathBuf::from(icon_path);
            println!("cargo:rerun-if-changed={}", icon.display());
            return validate_ico_icon(icon);
        }

        let source_icon = Path::new("build-assets/icon/app-icon.png");
        println!("cargo:rerun-if-changed={}", source_icon.display());

        let out_dir = match std::env::var_os("OUT_DIR") {
            Some(out_dir) => PathBuf::from(out_dir),
            None => {
                println!("cargo:warning=OUT_DIR is not set; building without app icon");
                return None;
            }
        };
        let derived_icon = out_dir.join("app-icon.ico");
        match derive_ico_icon(source_icon, &derived_icon) {
            Ok(()) => Some(derived_icon),
            Err(err) => {
                println!(
                    "cargo:warning=failed to derive AI Chat 2 Windows icon from '{}': {err}; building without app icon",
                    source_icon.display()
                );
                None
            }
        }
    }

    fn validate_ico_icon(icon: PathBuf) -> Option<PathBuf> {
        if !icon.exists() {
            println!(
                "cargo:warning=AI Chat 2 icon not found at '{}'; building without app icon",
                icon.display()
            );
            return None;
        }

        if !icon
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ico"))
        {
            println!(
                "cargo:warning=AI Chat 2 icon must be .ico for Windows resources (got '{}'); building without app icon",
                icon.display()
            );
            return None;
        }

        Some(icon)
    }

    fn derive_ico_icon(
        source_icon: &Path,
        derived_icon: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_image = image::open(source_icon)?;
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

        let file = File::create(derived_icon)?;
        IcoEncoder::new(BufWriter::new(file)).encode_images(&frames)?;
        Ok(())
    }
}
