use std::{path::PathBuf, sync::OnceLock};

pub fn fixture() -> PathBuf {
    static FIXTURE: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("process.rs");
            std::fs::write(&source, include_str!("../fixtures/process.rs")).unwrap();
            let binary = dir
                .path()
                .join(format!("pi-rpc-fixture{}", std::env::consts::EXE_SUFFIX));
            let result = std::process::Command::new(
                std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
            )
            .arg("--edition=2024")
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
            assert!(
                result.status.success(),
                "fixture compile failed: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            (dir, binary)
        })
        .1
        .clone()
}

pub fn options(dir: &std::path::Path, mode: &str) -> pi_rpc::LaunchOptions {
    let mut options = pi_rpc::LaunchOptions::new(fixture(), dir);
    options.env.push(("FIXTURE_MODE".into(), mode.into()));
    options
        .env
        .push(("FIXTURE_LOG".into(), dir.join("process.log").into()));
    options.startup_timeout = std::time::Duration::from_secs(3);
    options
}
