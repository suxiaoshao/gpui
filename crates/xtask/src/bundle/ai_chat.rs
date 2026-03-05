use image::ImageDecoder;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use tauri_bundler::{BundleBinary, PackageType, SettingsBuilder};
use tracing::{info, warn};

use crate::cli::BundleAiChatArgs;
use crate::cmd::run_cmd;
use crate::context::{ai_chat_dir, workspace_root};
use crate::error::{Result, XtaskError};
use crate::manifest::get_main_binary_name;

pub fn run(args: BundleAiChatArgs) -> Result<()> {
    if args.install || args.arch.is_some() || args.target.is_some() {
        warn!("--install/--arch/--target are only used on Windows and will be ignored");
    }

    let app_dir = ai_chat_dir()?;
    let workspace_dir = workspace_root()?;
    let bundle_dir = workspace_dir.join("target/release/bundle");
    let out_dir = workspace_dir.join("target/release");

    run_cmd(
        "cargo",
        &["build", "-p", "ai-chat", "--release"],
        Some(&workspace_dir),
    )?;

    prepare_bundle_icons(&app_dir)?;

    let manifest_path = app_dir.join("Cargo.toml");
    let main_bin_name = get_main_binary_name(&manifest_path)?;
    let (package_settings, bundle_settings) =
        crate::bundle::settings::read_bundle_settings(&manifest_path)?;

    let mut settings_builder = SettingsBuilder::new()
        .project_out_directory(&out_dir)
        .package_types(default_package_types())
        .package_settings(package_settings)
        .bundle_settings(bundle_settings)
        .binaries(vec![BundleBinary::new(main_bin_name, true)]);

    if let Ok(local_tools_dir) = env::var("TAURI_BUNDLER_TOOLS_DIR") {
        settings_builder = settings_builder.local_tools_directory(local_tools_dir);
        info!("using local tauri-bundler tools dir from TAURI_BUNDLER_TOOLS_DIR");
    }

    let settings = settings_builder
        .build()
        .map_err(|err| XtaskError::msg(format!("failed to build tauri bundle settings: {err}")))?;

    tauri_bundler::bundle_project(&settings)
        .map_err(|err| XtaskError::msg(format!("failed to bundle app with tauri-bundler: {err}")))?;

    #[cfg(target_os = "macos")]
    {
        let osx_dir = bundle_dir.join("osx");
        if let Some(app_path) = crate::bundle::macos::first_app_bundle(&osx_dir)? {
            crate::bundle::macos::inject_liquid_glass_icon(&app_dir, &app_path)?;
        } else {
            warn!("未找到 .app 包，跳过 Liquid Glass 图标注入");
        }
    }

    info!(bundle_dir = %bundle_dir.display(), "打包完成");
    Ok(())
}

fn default_package_types() -> Vec<PackageType> {
    #[cfg(target_os = "macos")]
    {
        return vec![PackageType::MacOsBundle];
    }

    #[cfg(target_os = "linux")]
    {
        return vec![PackageType::Deb];
    }

    vec![]
}

fn prepare_bundle_icons(app_dir: &Path) -> Result<()> {
    let mut src_png = app_dir.join("build-assets/icon/ChatGPT.icon/Assets/logo.png");
    if !src_png.exists() {
        src_png = app_dir.join("build-assets/icon/app-icon.png");
    }

    let iconset_dir = app_dir.join("build-assets/icon/app-icon.iconset");
    let required_icon = iconset_dir.join("icon_512x512@2x.png");
    let mut should_regenerate = false;

    if required_icon.exists() {
        if is_rgba16_png(&required_icon)? {
            should_regenerate = true;
        } else {
            return Ok(());
        }
    }

    if !src_png.exists() {
        warn!(icon = %src_png.display(), "未找到源图标，跳过 iconset 生成");
        return Ok(());
    }

    fs::create_dir_all(&iconset_dir).map_err(|err| {
        XtaskError::msg(format!(
            "failed to create iconset dir {}: {err}",
            iconset_dir.display()
        ))
    })?;

    if should_regenerate {
        for entry in fs::read_dir(&iconset_dir).map_err(|err| {
            XtaskError::msg(format!(
                "failed to read iconset dir {}: {err}",
                iconset_dir.display()
            ))
        })? {
            let path = entry
                .map_err(|err| XtaskError::msg(format!("failed to read iconset dir entry: {err}")))?
                .path();
            if path.extension().and_then(OsStr::to_str) == Some("png") {
                fs::remove_file(&path).map_err(|err| {
                    XtaskError::msg(format!("failed to remove {}: {err}", path.display()))
                })?;
            }
        }
    }

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

    Ok(())
}

fn is_rgba16_png(path: &Path) -> Result<bool> {
    let file = fs::File::open(path)
        .map_err(|err| XtaskError::msg(format!("failed to open {}: {err}", path.display())))?;
    let reader = std::io::BufReader::new(file);
    let decoder = image::codecs::png::PngDecoder::new(reader)
        .map_err(|err| XtaskError::msg(format!("failed to parse png {}: {err}", path.display())))?;

    Ok(decoder.original_color_type() == image::ExtendedColorType::Rgba16)
}
