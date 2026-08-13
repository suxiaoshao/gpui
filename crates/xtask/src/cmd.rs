use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, ExitStatus};

use crate::error::{Result, XtaskError};

#[cfg(target_os = "macos")]
pub fn command_exists(command: &str) -> bool {
    which::which(command).is_ok()
}

pub fn run_cmd_with_env(
    command: &str,
    args: &[&str],
    current_dir: Option<&Path>,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<()> {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    run_cmd_program_os_with_env(OsStr::new(command), &args, current_dir, environment)
}

pub fn run_cmd_os(command: &str, args: &[&OsStr], current_dir: Option<&Path>) -> Result<()> {
    run_cmd_program_os(OsStr::new(command), args, current_dir)
}

pub fn run_cmd_program_os(
    command: &OsStr,
    args: &[&OsStr],
    current_dir: Option<&Path>,
) -> Result<()> {
    run_cmd_program_os_with_env(command, args, current_dir, &[])
}

fn run_cmd_program_os_with_env(
    command: &OsStr,
    args: &[&OsStr],
    current_dir: Option<&Path>,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<()> {
    let rendered_cmd = format!(
        "{} {}",
        shell_escape_os(command),
        args.iter()
            .map(|arg| shell_escape_os(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let mut cmd = Command::new(command);
    cmd.args(args);
    cmd.envs(environment.iter().map(|(key, value)| (key, value)));

    if let Some(current_dir) = current_dir {
        cmd.current_dir(current_dir);
    }

    let status = cmd.status().map_err(|source| XtaskError::CommandExecute {
        command: rendered_cmd.clone(),
        source,
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(XtaskError::CommandFailed {
            command: rendered_cmd,
            status: status_to_string(status),
        })
    }
}

fn shell_escape_os(value: &OsStr) -> String {
    let value = value.to_string_lossy();
    if value.chars().all(|c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' || c == ':'
    }) {
        value.into_owned()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

fn status_to_string(status: ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "terminated by signal".to_string())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn command_exists_handles_bare_absolute_and_missing_commands() {
        assert!(command_exists("sh"));

        let current_exe = std::env::current_exe().unwrap();
        assert!(command_exists(current_exe.to_str().unwrap()));

        assert!(!command_exists(
            "jaco-xtask-command-that-must-not-exist-5b5657b7"
        ));
    }
}
