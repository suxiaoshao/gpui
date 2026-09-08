use std::{env, io, path::PathBuf};

fn directory(key: &str, fallback: Option<PathBuf>) -> io::Result<PathBuf> {
    let path = env::var_os(key)
        .filter(|v| !v.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
        .or(fallback)
        .ok_or_else(|| io::Error::other("system directory unavailable"))?;
    std::path::absolute(path)
}
pub(crate) fn config_dir() -> io::Result<PathBuf> {
    directory(
        "GUPI_CONFIG_DIR",
        dirs_next::config_dir().map(|p| p.join("gupi")),
    )
}
pub(crate) fn log_dir() -> io::Result<PathBuf> {
    #[cfg(target_os = "macos")]
    let fallback = dirs_next::home_dir().map(|p| p.join("Library/Logs/gupi"));
    #[cfg(not(target_os = "macos"))]
    let fallback = dirs_next::data_local_dir().map(|p| p.join("gupi/logs"));
    directory("GUPI_LOG_DIR", fallback)
}
