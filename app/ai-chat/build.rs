#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]

fn main() {
    #[cfg(target_os = "windows")]
    {
        // Allow overriding icon path from environment for CI/release pipelines.
        let icon_path = std::env::var("AI_CHAT_ICON_PATH")
            .unwrap_or_else(|_| "assets/icon/app-icon.ico".into());
        let icon = std::path::Path::new(&icon_path);

        println!("cargo:rerun-if-env-changed=AI_CHAT_ICON_PATH");
        println!("cargo:rerun-if-changed={}", icon.display());

        let mut res = winresource::WindowsResource::new();
        if let Ok(toolkit_path) = std::env::var("AI_CHAT_RC_TOOLKIT_PATH") {
            res.set_toolkit_path(toolkit_path.as_str());
        }
        if icon.exists()
            && icon
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ico"))
        {
            res.set_icon(icon.to_str().unwrap());
        } else if !icon.exists() {
            println!(
                "cargo:warning=AI Chat icon not found at '{}'; building without app icon",
                icon.display()
            );
        } else {
            println!(
                "cargo:warning=AI Chat icon must be .ico for Windows resources (got '{}'); building without app icon",
                icon.display()
            );
        }
        res.set("FileDescription", "AI Chat");
        res.set("ProductName", "AI Chat");

        if let Err(err) = res.compile() {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
