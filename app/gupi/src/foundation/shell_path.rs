//! Capture the PATH from the user's login shell for GUI launches on macOS.
//!
//! The shell starts this executable again with `--gupi-print-shell-path`. The
//! helper writes a NUL-framed byte string so startup-file output cannot be
//! mistaken for PATH, and non-UTF-8 paths survive the round trip.

use std::{
    ffi::{CStr, OsStr, OsString},
    io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::{io::AsyncRead, process::Command, time::timeout};

#[cfg(unix)]
use std::os::unix::{
    ffi::{OsStrExt, OsStringExt},
    process::CommandExt,
};

const FRAME_MARKER: &[u8] = b"\0GUPI-SHELL-PATH-v1\0";
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const HELPER_ENV: &str = "GUPI_SHELL_PATH_HELPER";

/// Capture only PATH from the current user's default login shell.
pub(crate) async fn capture() -> Result<OsString, String> {
    let (shell, home) = login_shell_and_home()?;
    let helper =
        std::env::current_exe().map_err(|error| format!("resolve Gupi executable: {error}"))?;
    capture_with(&shell, &home, helper.as_os_str(), CAPTURE_TIMEOUT).await
}

/// Entry point used by `main` before GPUI starts in shell PATH helper mode.
pub(crate) fn print_path() -> io::Result<()> {
    let path = std::env::var_os("PATH")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PATH is not set"))?;
    let frame = path_frame(&path);
    use io::Write;
    let mut stdout = io::stdout().lock();
    stdout.write_all(&frame)?;
    stdout.flush()
}

fn path_frame(path: &OsStr) -> Vec<u8> {
    let mut frame = Vec::with_capacity(FRAME_MARKER.len() + path.as_bytes().len() + 1);
    frame.extend_from_slice(FRAME_MARKER);
    frame.extend_from_slice(path.as_bytes());
    frame.push(0);
    frame
}

fn parse_path_frame(output: &[u8]) -> Result<OsString, String> {
    let marker_start = output
        .windows(FRAME_MARKER.len())
        .position(|window| window == FRAME_MARKER)
        .ok_or_else(|| "login shell did not return a PATH frame".to_owned())?;
    let value_start = marker_start + FRAME_MARKER.len();
    let value_end = output[value_start..]
        .iter()
        .position(|byte| *byte == 0)
        .map(|offset| value_start + offset)
        .ok_or_else(|| "login shell returned an incomplete PATH frame".to_owned())?;
    Ok(OsString::from_vec(output[value_start..value_end].to_vec()))
}

fn capture_command(shell: &OsStr, home: &Path, helper: &OsStr) -> Result<Command, String> {
    const EXEC_HELPER: &str = "exec \"$GUPI_SHELL_PATH_HELPER\" --gupi-print-shell-path";
    let mut command = Command::new(shell);
    match Path::new(shell).file_name().and_then(OsStr::to_str) {
        Some("sh" | "bash" | "zsh" | "dash" | "ksh") => {
            command.args(["-l", "-i", "-c", EXEC_HELPER]);
        }
        Some("fish") => {
            // Some version managers update PATH from fish_prompt hooks.
            command.args([
                "-l", "-i", "-c",
                "emit fish_prompt >/dev/null 2>/dev/null; exec \"$GUPI_SHELL_PATH_HELPER\" --gupi-print-shell-path",
            ]);
        }
        Some("csh" | "tcsh") => {
            // These shells cannot combine -l with command arguments. A leading
            // '-' in argv[0] enables login startup files, including ~/.login.
            command.as_std_mut().arg0("-");
            command.args(["-i", "-c", EXEC_HELPER]);
        }
        Some("nu") => {
            // -e loads interactive/login configuration before executing. Nu
            // reads environment variables through $env, not POSIX expansion.
            // exec replaces the shell so it never enters the REPL afterwards.
            command.args([
                "-l",
                "-e",
                "exec $env.GUPI_SHELL_PATH_HELPER --gupi-print-shell-path",
            ]);
        }
        _ => {
            return Err(format!(
                "unsupported login shell: {}",
                Path::new(shell).display()
            ));
        }
    }
    command
        .env(HELPER_ENV, helper)
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Keep an interactive shell or a version-manager hook from inheriting the
    // app's controlling terminal or leaving descendants holding our pipes.
    command.as_std_mut().process_group(0);
    Ok(command)
}

async fn capture_with(
    shell: &OsStr,
    home: &Path,
    helper: &OsStr,
    deadline: Duration,
) -> Result<OsString, String> {
    let mut child = capture_command(shell, home, helper)?
        .spawn()
        .map_err(|error| format!("start login shell: {error}"))?;
    let pid = child
        .id()
        .ok_or_else(|| "login shell did not provide a process id".to_owned())?;
    let mut group = ProcessGroup::new(pid as i32);
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "login shell stdout was unavailable".to_owned())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "login shell stderr was unavailable".to_owned())?;
    let total = Arc::new(AtomicUsize::new(0));

    let result = timeout(deadline, async {
        let status = async {
            child
                .wait()
                .await
                .map_err(|error| format!("wait for login shell: {error}"))
        };
        let stdout = read_capped(stdout, total.clone(), true);
        let stderr = read_capped(stderr, total, false);
        let (status, stdout, _) = tokio::try_join!(status, stdout, stderr)?;
        if !status.success() {
            return Err("login shell exited unsuccessfully".to_owned());
        }
        parse_path_frame(&stdout)
    })
    .await;

    match result {
        Ok(Ok(path)) => {
            group.disarm();
            Ok(path)
        }
        result => {
            group.kill();
            let _ = child.start_kill();
            let _ = child.wait().await;
            match result {
                Ok(Err(error)) => Err(error),
                _ => Err("login shell PATH capture timed out".to_owned()),
            }
        }
    }
}

async fn read_capped(
    mut reader: impl AsyncRead + Unpin,
    total: Arc<AtomicUsize>,
    keep_bytes: bool,
) -> Result<Vec<u8>, String> {
    use tokio::io::AsyncReadExt;

    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let count = reader
            .read(&mut chunk)
            .await
            .map_err(|error| format!("read login shell output: {error}"))?;
        if count == 0 {
            break;
        }
        let previous = total.fetch_add(count, Ordering::Relaxed);
        if keep_bytes && previous < MAX_OUTPUT_BYTES {
            let keep = count.min(MAX_OUTPUT_BYTES - previous);
            bytes.extend_from_slice(&chunk[..keep]);
        }
        if previous.saturating_add(count) > MAX_OUTPUT_BYTES {
            return Err("login shell output exceeded 1 MiB".to_owned());
        }
    }
    Ok(bytes)
}

struct ProcessGroup {
    id: i32,
    armed: bool,
}

impl ProcessGroup {
    fn new(id: i32) -> Self {
        Self { id, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn kill(&mut self) {
        if self.armed {
            // SAFETY: the child was started in a fresh process group whose id is
            // its pid. A negative pid addresses only that group.
            unsafe {
                libc::kill(-self.id, libc::SIGKILL);
            }
            self.armed = false;
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.kill();
    }
}

fn login_shell_and_home() -> Result<(OsString, PathBuf), String> {
    match account_shell_and_home() {
        Ok(Some(value)) => Ok(value),
        Ok(None) | Err(_) => {
            let shell = std::env::var_os("SHELL")
                .filter(|shell| !shell.is_empty())
                .ok_or_else(|| "could not determine the user's login shell".to_owned())?;
            let home = dirs_next::home_dir()
                .ok_or_else(|| "could not determine the user's home directory".to_owned())?;
            Ok((shell, home))
        }
    }
}

#[cfg(target_os = "macos")]
fn account_shell_and_home() -> Result<Option<(OsString, PathBuf)>, String> {
    use std::mem::MaybeUninit;

    let uid = unsafe { libc::getuid() };
    let configured = unsafe { libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) };
    let capacity = if configured > 0 {
        (configured as usize).clamp(1024, 1024 * 1024)
    } else {
        16 * 1024
    };
    let mut buffer = vec![0u8; capacity];
    let mut entry = MaybeUninit::<libc::passwd>::zeroed();
    let mut result = std::ptr::null_mut();
    // SAFETY: `entry` is writable, `buffer` remains alive for the call, and
    // libc initializes `entry` and `result` on success.
    let code = unsafe {
        libc::getpwuid_r(
            uid,
            entry.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    if code != 0 {
        return Err(format!("read the user's account record: {code}"));
    }
    if result.is_null() {
        return Ok(None);
    }
    // SAFETY: a non-null result from successful getpwuid_r points at `entry`.
    let entry = unsafe { entry.assume_init() };
    if entry.pw_shell.is_null() || entry.pw_dir.is_null() {
        return Ok(None);
    }
    // SAFETY: both pointers refer to NUL-terminated strings in `buffer`.
    let shell = unsafe { CStr::from_ptr(entry.pw_shell) }.to_bytes();
    let home = unsafe { CStr::from_ptr(entry.pw_dir) }.to_bytes();
    if shell.is_empty() || home.is_empty() {
        return Ok(None);
    }
    Ok(Some((
        OsString::from_vec(shell.to_vec()),
        PathBuf::from(OsString::from_vec(home.to_vec())),
    )))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn path_frame_preserves_non_utf8_bytes_and_ignores_shell_noise() {
        let path = OsString::from_vec(b"/usr/bin:/tmp/\xff-bin".to_vec());
        let frame = path_frame(&path);
        let mut noisy = b"zsh startup notice\n".to_vec();
        noisy.extend_from_slice(&frame);
        noisy.extend_from_slice(b"\ntrailing shell output");
        assert_eq!(
            parse_path_frame(&noisy).unwrap().as_bytes(),
            path.as_bytes()
        );
        assert!(
            noisy
                .windows(FRAME_MARKER.len())
                .any(|window| window == FRAME_MARKER)
        );
    }
}
