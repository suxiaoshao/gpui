use crate::{Error, jsonl::Jsonl, protocol::*};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    process::{ExitStatus, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin},
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch},
    task::JoinHandle,
    time::{Instant, sleep_until, timeout},
};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Limits count encoded JSON bytes; decoded allocations also depend on JSON shape.
#[derive(Clone, Debug)]
pub struct Limits {
    pub frame_bytes: usize,
    pub event_bytes: usize,
    pub events: usize,
    pub requests: usize,
    pub writes: usize,
    pub stderr_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            frame_bytes: 32 * 1024 * 1024,
            event_bytes: 32 * 1024 * 1024,
            events: 128,
            requests: 128,
            writes: 32,
            stderr_bytes: 64 * 1024,
        }
    }
}
impl Limits {
    fn validate(&self) -> Result<(), Error> {
        if [
            self.frame_bytes,
            self.event_bytes,
            self.events,
            self.requests,
            self.writes,
            self.stderr_bytes,
        ]
        .iter()
        .any(|&n| n == 0 || n > u32::MAX as usize || n > Semaphore::MAX_PERMITS)
        {
            return Err(Error::Options(
                "limits must be positive and fit the channel/semaphore range",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct LaunchOptions {
    pub executable: PathBuf,
    pub cwd: PathBuf,
    /// Arguments in addition to --mode rpc. No shell command string is constructed.
    pub args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub clear_env: bool,
    pub startup_timeout: Duration,
    pub limits: Limits,
}
impl LaunchOptions {
    pub fn new(executable: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            cwd: cwd.into(),
            args: Vec::new(),
            env: Vec::new(),
            clear_env: false,
            startup_timeout: Duration::from_secs(15),
            limits: Limits::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CloseReport {
    /// Present only when the owner observed exit before releasing the Child.
    pub status: Option<ExitStatus>,
    /// Connection failure or failure to complete graceful shutdown.
    pub reason: Option<Error>,
    pub stderr: String,
}
#[derive(Clone, Debug)]
pub enum ConnectionState {
    Starting,
    Ready(Box<SessionState>),
    Closing,
    Closed(CloseReport),
}

struct Pending {
    command: String,
    reply: oneshot::Sender<Result<Response, Error>>,
}
struct Requests {
    accepting: bool,
    pending: HashMap<String, Pending>,
}
struct Shared {
    requests: Mutex<Requests>,
    next_id: AtomicU64,
}
struct Inner {
    shared: Arc<Shared>,
    writes: mpsc::Sender<Write>,
    write_budget: Arc<Semaphore>,
    close: watch::Sender<bool>,
    state: watch::Receiver<ConnectionState>,
    limits: Limits,
}
impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.close.send(true);
    }
}
/// Clones share the same single process. Dropping the last clone requests close.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}
struct Write {
    bytes: Vec<u8>,
    _permit: OwnedSemaphorePermit,
}
struct QueuedEvent {
    event: Event,
    _permit: OwnedSemaphorePermit,
}
pub struct EventStream {
    receiver: mpsc::Receiver<QueuedEvent>,
}
impl EventStream {
    pub async fn recv(&mut self) -> Option<Event> {
        self.receiver.recv().await.map(|item| item.event)
    }
}
struct Waiting {
    shared: Arc<Shared>,
    id: String,
}
impl Drop for Waiting {
    fn drop(&mut self) {
        self.shared
            .requests
            .lock()
            .unwrap()
            .pending
            .remove(&self.id);
    }
}

impl Client {
    /// Requires an entered Tokio runtime. Returns before the protocol is Ready.
    pub async fn spawn(options: LaunchOptions) -> Result<(Self, EventStream), Error> {
        options.limits.validate()?;
        if options.startup_timeout.is_zero() {
            return Err(Error::Options("startup timeout must be positive"));
        }
        let deadline = Instant::now() + options.startup_timeout;
        let mut child = launch(options.clone(), deadline).await?;
        let stdin = child
            .stdin
            .take()
            .ok_or(Error::Options("missing piped stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(Error::Options("missing piped stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(Error::Options("missing piped stderr"))?;
        let (writes, writer_rx) = mpsc::channel(options.limits.writes);
        let write_budget = Arc::new(Semaphore::new(options.limits.frame_bytes));
        let (close, close_rx) = watch::channel(false);
        let (state_tx, state) = watch::channel(ConnectionState::Starting);
        let (event_tx, receiver) = mpsc::channel(options.limits.events);
        let shared = Arc::new(Shared {
            requests: Mutex::new(Requests {
                accepting: true,
                pending: HashMap::new(),
            }),
            next_id: AtomicU64::new(1),
        });
        let client = Self {
            inner: Arc::new(Inner {
                shared: shared.clone(),
                writes,
                write_budget,
                close,
                state,
                limits: options.limits.clone(),
            }),
        };
        client.enqueue(serde_json::to_vec(&json!({"id":"0","type":"get_state"}))?)?;
        let (writer_control, control_rx) = watch::channel(WriterControl::Open);
        let writer_task = tokio::spawn(writer(stdin, writer_rx, control_rx));
        let tail = Arc::new(Mutex::new(Vec::new()));
        let stderr_task = tokio::spawn(drain_stderr(
            stderr,
            tail.clone(),
            options.limits.stderr_bytes,
        ));
        tokio::spawn(
            Owner {
                child,
                stdout: Jsonl::new(stdout, options.limits.frame_bytes),
                shared,
                state: state_tx,
                events: event_tx,
                event_budget: Arc::new(Semaphore::new(options.limits.event_bytes)),
                close: close_rx,
                writer_control,
                writer_task,
                stderr_task,
                tail,
                deadline,
            }
            .run(),
        );
        Ok((client, EventStream { receiver }))
    }
    pub fn state(&self) -> ConnectionState {
        self.inner.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<ConnectionState> {
        self.inner.state.clone()
    }
    pub async fn ready(&self) -> Result<SessionState, Error> {
        let mut state = self.subscribe();
        loop {
            match state.borrow_and_update().clone() {
                ConnectionState::Ready(value) => return Ok(*value),
                ConnectionState::Closed(report) => {
                    return Err(report.reason.unwrap_or(Error::NotReady));
                }
                ConnectionState::Closing => {}
                ConnectionState::Starting => {}
            }
            state.changed().await.map_err(|_| Error::Closed)?;
        }
    }
    pub async fn request(&self, command: Command) -> Result<Response, Error> {
        self.request_raw(serde_json::to_value(command)?).await
    }
    /// Escape hatch for commands not yet typed by this crate. The client owns id.
    pub async fn request_raw(&self, mut value: Value) -> Result<Response, Error> {
        let name = value
            .get("type")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty() && *name != "extension_ui_response")
            .ok_or_else(|| {
                Error::Protocol("command must have a string type; UI replies use reply()".into())
            })?
            .to_owned();
        let id = self
            .inner
            .shared
            .next_id
            .fetch_add(1, Ordering::Relaxed)
            .to_string();
        value
            .as_object_mut()
            .expect("tagged command")
            .insert("id".into(), Value::String(id.clone()));
        let bytes = encode(&value, self.inner.limits.frame_bytes)?;
        let (reply, receive) = oneshot::channel();
        let waiting = Waiting {
            shared: self.inner.shared.clone(),
            id: id.clone(),
        };
        {
            let mut requests = self.inner.shared.requests.lock().unwrap();
            if !requests.accepting {
                return Err(Error::Closed);
            }
            if requests.pending.len() >= self.inner.limits.requests {
                return Err(Error::Capacity("pending requests"));
            }
            requests.pending.insert(
                id,
                Pending {
                    command: name,
                    reply,
                },
            );
            self.enqueue(bytes)?;
        }
        let result = receive.await.map_err(|_| Error::Closed)?;
        drop(waiting);
        result
    }
    pub async fn get_state(&self) -> Result<SessionState, Error> {
        self.data(Command::GetState).await
    }
    pub async fn get_commands(&self) -> Result<Commands, Error> {
        self.data(Command::GetCommands).await
    }
    pub async fn get_entries(&self) -> Result<Entries, Error> {
        self.data(Command::GetEntries).await
    }
    pub async fn get_fork_messages(&self) -> Result<ForkMessages, Error> {
        self.data(Command::GetForkMessages).await
    }
    pub async fn fork(&self, entry_id: String) -> Result<ForkResult, Error> {
        self.data(Command::Fork { entry_id }).await
    }
    pub async fn set_session_name(&self, name: String) -> Result<Response, Error> {
        self.request(Command::SetSessionName { name }).await
    }
    pub async fn get_available_models(&self) -> Result<Models, Error> {
        self.data(Command::GetAvailableModels).await
    }
    pub async fn set_model(&self, provider: String, model_id: String) -> Result<Model, Error> {
        self.data(Command::SetModel { provider, model_id }).await
    }
    pub async fn get_available_thinking_levels(&self) -> Result<ThinkingLevels, Error> {
        self.data(Command::GetAvailableThinkingLevels).await
    }
    pub async fn set_thinking_level(&self, level: String) -> Result<Response, Error> {
        self.request(Command::SetThinkingLevel { level }).await
    }
    pub async fn get_session_stats(&self) -> Result<SessionStats, Error> {
        self.data(Command::GetSessionStats).await
    }
    pub async fn clear_queue(&self) -> Result<ClearedQueue, Error> {
        self.data(Command::ClearQueue).await
    }
    pub async fn prompt(&self, prompt: Prompt) -> Result<Response, Error> {
        self.request(Command::Prompt(prompt)).await
    }
    pub async fn abort(&self) -> Result<Response, Error> {
        self.request(Command::Abort).await
    }
    async fn data<T: DeserializeOwned>(&self, command: Command) -> Result<T, Error> {
        Ok(serde_json::from_value(self.request(command).await?.data)?)
    }
    /// Queues a reply, without treating its extension ID as a command ID.
    pub fn reply(&self, id: &str, reply: UiReply) -> Result<(), Error> {
        let mut value = serde_json::to_value(reply)?;
        let object = value.as_object_mut().expect("reply object");
        object.insert("type".into(), "extension_ui_response".into());
        object.insert("id".into(), id.into());
        let bytes = encode(&value, self.inner.limits.frame_bytes)?;
        let requests = self.inner.shared.requests.lock().unwrap();
        if !requests.accepting {
            return Err(Error::Closed);
        }
        self.enqueue(bytes)
    }
    fn enqueue(&self, bytes: Vec<u8>) -> Result<(), Error> {
        let permit = self
            .inner
            .write_budget
            .clone()
            .try_acquire_many_owned(bytes.len() as u32)
            .map_err(|_| Error::Capacity("queued write bytes"))?;
        self.inner
            .writes
            .try_send(Write {
                bytes,
                _permit: permit,
            })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => Error::Capacity("write queue"),
                mpsc::error::TrySendError::Closed(_) => Error::Closed,
            })
    }
    /// Idempotent; cancellation of this wait does not cancel the owner's cleanup.
    pub fn close(&self) -> impl std::future::Future<Output = CloseReport> + Send + 'static {
        self.inner.shared.requests.lock().unwrap().accepting = false;
        let _ = self.inner.close.send(true);
        let mut state = self.subscribe();
        async move {
            loop {
                if let ConnectionState::Closed(report) = state.borrow_and_update().clone() {
                    return report;
                }
                if state.changed().await.is_err() {
                    return CloseReport {
                        status: None,
                        reason: Some(Error::Closed),
                        stderr: String::new(),
                    };
                }
            }
        }
    }
}

fn encode(value: &Value, limit: usize) -> Result<Vec<u8>, Error> {
    struct Bounded {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl std::io::Write for Bounded {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len().saturating_add(buf.len()) > self.limit {
                return Err(std::io::Error::other("frame capacity exceeded"));
            }
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Bounded {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| Error::Capacity("outgoing frame"))?;
    Ok(writer.bytes)
}

async fn launch(options: LaunchOptions, deadline: Instant) -> Result<Child, Error> {
    static SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();
    tokio::time::timeout_at(deadline, async move {
        let slot = SLOTS
            .get_or_init(|| Arc::new(Semaphore::new(2)))
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| Error::Closed)?;
        // A cancelled JoinHandle drops the late output; kill_on_drop owns that Child.
        tokio::task::spawn_blocking(move || {
            let _slot = slot;
            if Instant::now() >= deadline {
                return Err(Error::StartupTimeout);
            }
            let mut command = tokio::process::Command::new(&options.executable);
            command
                .arg("--mode")
                .arg("rpc")
                .args(options.args)
                .current_dir(options.cwd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            if options.clear_env {
                command.env_clear();
            }
            command.envs(options.env);
            #[cfg(windows)]
            command.creation_flags(0x08000000);
            let child = command.spawn()?;
            if Instant::now() >= deadline {
                return Err(Error::StartupTimeout);
            }
            Ok(child)
        })
        .await
        .map_err(|error| Error::Io(error.to_string()))?
    })
    .await
    .map_err(|_| Error::StartupTimeout)?
}

#[derive(Clone, Copy, PartialEq)]
enum WriterControl {
    Open,
    Abort,
    Closed,
}
async fn writer(
    mut stdin: ChildStdin,
    mut writes: mpsc::Receiver<Write>,
    mut control: watch::Receiver<WriterControl>,
) -> Result<(), Error> {
    loop {
        let mode = *control.borrow_and_update();
        match mode {
            WriterControl::Closed => return Ok(()),
            WriterControl::Abort => {
                // Cancels any blocked normal write, and never waits for an RPC response.
                tokio::select! {
                    biased;
                    _ = control.changed() => {},
                    result = stdin.write_all(b"{\"id\":\"close\",\"type\":\"abort\"}\n") => { result?; }
                }
                while *control.borrow_and_update() != WriterControl::Closed {
                    if control.changed().await.is_err() {
                        return Ok(());
                    }
                }
                return Ok(());
            }
            WriterControl::Open => {}
        }
        tokio::select! {
            biased;
            _ = control.changed() => {},
            item = writes.recv() => {
                let Some(item) = item else { return Ok(()); };
                tokio::select! {
                    biased;
                    _ = control.changed() => {},
                    result = async {
                        stdin.write_all(&item.bytes).await?;
                        stdin.write_all(b"\n").await?;
                        stdin.flush().await
                    } => { result?; }
                }
            }
        }
    }
}
async fn drain_stderr(
    mut stderr: tokio::process::ChildStderr,
    tail: Arc<Mutex<Vec<u8>>>,
    limit: usize,
) {
    let mut chunk = [0; 8192];
    while let Ok(n) = stderr.read(&mut chunk).await {
        if n == 0 {
            break;
        }
        let mut tail = tail.lock().unwrap();
        if n >= limit {
            tail.clear();
            tail.extend_from_slice(&chunk[n - limit..n]);
        } else {
            let remove = tail.len().saturating_add(n).saturating_sub(limit);
            tail.drain(..remove);
            tail.extend_from_slice(&chunk[..n]);
        }
    }
}

struct Owner {
    child: Child,
    stdout: Jsonl<tokio::process::ChildStdout>,
    shared: Arc<Shared>,
    state: watch::Sender<ConnectionState>,
    events: mpsc::Sender<QueuedEvent>,
    event_budget: Arc<Semaphore>,
    close: watch::Receiver<bool>,
    writer_control: watch::Sender<WriterControl>,
    writer_task: JoinHandle<Result<(), Error>>,
    stderr_task: JoinHandle<()>,
    tail: Arc<Mutex<Vec<u8>>>,
    deadline: Instant,
}
impl Owner {
    async fn run(mut self) {
        let mut ready = false;
        let mut status = None;
        let mut drain_deadline = None;
        let mut writer_finished = false;
        let reason = loop {
            tokio::select! {
                biased;
                _ = self.close.changed() => break Some(Error::Closed),
                _ = self.events.closed() => break Some(Error::Closed),
                _ = sleep_until(self.deadline), if !ready => break Some(Error::StartupTimeout),
                _ = async { sleep_until(drain_deadline.unwrap_or(self.deadline)).await }, if drain_deadline.is_some() => break Some(Error::Closed),
                line = self.stdout.next() => {
                    match line {
                        Ok(Some(line)) => match self.dispatch(&line, &mut ready) {
                            Ok(()) => {}, Err(error) => break Some(error),
                        },
                        Ok(None) => break if ready { None } else { Some(Error::NotReady) },
                        Err(error) => break Some(error),
                    }
                }
                result = self.child.wait(), if status.is_none() => {
                    match result {
                        Ok(value) => {
                            status = Some(value);
                            // Drain bytes already written before exit, but do not wait forever
                            // for an extension descendant holding stdout open.
                            drain_deadline = Some(Instant::now() + SHUTDOWN_TIMEOUT);
                        }
                        Err(error) => break Some(error.into()),
                    }
                }
                result = &mut self.writer_task, if !writer_finished => {
                    writer_finished = true;
                    break Some(match result {
                        Ok(Err(error)) => error,
                        Ok(Ok(())) => Error::Closed,
                        Err(error) => Error::Io(error.to_string()),
                    });
                }
            }
        };
        let requested = *self.close.borrow();
        self.settle(reason.clone().unwrap_or(Error::Closed));
        self.state.send_replace(ConnectionState::Closing);
        let mut reason = match (reason, requested) {
            (Some(Error::Closed), true) => None,
            (reason, _) => reason,
        };
        if status.is_none() && reason.is_none() {
            let result = timeout(SHUTDOWN_TIMEOUT, async {
                if requested {
                    self.shutdown(&mut status, &mut writer_finished).await
                } else {
                    // stdout EOF on an otherwise healthy connection: observe exit.
                    status = Some(self.child.wait().await?);
                    Ok(())
                }
            })
            .await;
            reason = match result {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error),
                Err(_) => Some(Error::ShutdownTimeout),
            };
        }
        // Tokio JoinHandle drop detaches tasks. Cancel them explicitly so their
        // pipe handles are released; Child drop handles process termination/reaping.
        if !writer_finished {
            self.writer_task.abort();
            let _ = (&mut self.writer_task).await;
        }
        self.stderr_task.abort();
        let _ = (&mut self.stderr_task).await;
        drop(self.child);
        drop(self.stdout);
        drop(self.events);
        let stderr = String::from_utf8_lossy(&self.tail.lock().unwrap()).into_owned();
        self.state
            .send_replace(ConnectionState::Closed(CloseReport {
                status,
                reason,
                stderr,
            }));
    }
    async fn shutdown(
        &mut self,
        status: &mut Option<ExitStatus>,
        writer_finished: &mut bool,
    ) -> Result<(), Error> {
        let _ = self.writer_control.send(WriterControl::Abort);
        let mut stdout_open = true;
        // Read final events while abort settles. The reserved response confirms
        // that Pi is idle; only then close stdin to run its session_shutdown hooks.
        while stdout_open || status.is_none() {
            tokio::select! {
                line = self.stdout.next(), if stdout_open => {
                    if let Some(line) = line? {
                        let raw: Value = serde_json::from_slice(&line)?;
                        if raw.get("type").and_then(Value::as_str) == Some("response")
                            && raw.get("id").and_then(Value::as_str) == Some("close")
                        {
                            let response: Response = serde_json::from_value(raw)?;
                            if response.command != "abort" {
                                return Err(Error::Protocol("shutdown command mismatch".into()));
                            }
                            if !response.success {
                                return Err(rejection(&response));
                            }
                            let _ = self.writer_control.send(WriterControl::Closed);
                        } else {
                            // A late startup response must not restore Ready while closing.
                            self.dispatch(&line, &mut true)?;
                        }
                    } else {
                        stdout_open = false;
                    }
                }
                result = self.child.wait(), if status.is_none() => {
                    *status = Some(result?);
                }
                result = &mut self.writer_task, if !*writer_finished => {
                    *writer_finished = true;
                    result.map_err(|error| Error::Io(error.to_string()))??;
                }
            }
        }
        Ok(())
    }
    fn settle(&self, error: Error) {
        let mut requests = self.shared.requests.lock().unwrap();
        requests.accepting = false;
        for (_, pending) in requests.pending.drain() {
            let _ = pending.reply.send(Err(error.clone()));
        }
    }
    fn dispatch(&self, line: &[u8], ready: &mut bool) -> Result<(), Error> {
        let raw: Value = serde_json::from_slice(line)?;
        let kind = raw
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Protocol("missing string type".into()))?;
        if kind == "response" {
            let response: Response = serde_json::from_value(raw)?;
            if !response.success && response.error.is_none() {
                return Err(Error::Protocol("failed response has no error".into()));
            }
            if response.id.as_deref() == Some("0") && !*ready {
                if response.command != "get_state" {
                    return Err(Error::Protocol("startup command mismatch".into()));
                }
                if !response.success {
                    return Err(rejection(&response));
                }
                let state = serde_json::from_value(response.data)?;
                *ready = true;
                self.state
                    .send_replace(ConnectionState::Ready(Box::new(state)));
            } else if let Some(id) = &response.id {
                let pending = self.shared.requests.lock().unwrap().pending.remove(id);
                if let Some(pending) = pending {
                    if pending.command != response.command {
                        let error = Error::Protocol("response command mismatch".into());
                        let _ = pending.reply.send(Err(error.clone()));
                        return Err(error);
                    }
                    let result = if response.success {
                        Ok(response)
                    } else {
                        Err(rejection(&response))
                    };
                    let _ = pending.reply.send(result);
                }
            }
            return Ok(());
        }
        let event = if kind == "extension_ui_request" {
            Event::ExtensionUi {
                request: serde_json::from_value(raw.clone())?,
                raw,
            }
        } else {
            Event::Agent {
                kind: kind.to_owned(),
                raw,
            }
        };
        let permit = self
            .event_budget
            .clone()
            .try_acquire_many_owned(line.len() as u32)
            .map_err(|_| Error::Capacity("queued event bytes"))?;
        self.events
            .try_send(QueuedEvent {
                event,
                _permit: permit,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => Error::Capacity("event queue"),
                mpsc::error::TrySendError::Closed(_) => Error::Closed,
            })
    }
}
fn rejection(response: &Response) -> Error {
    Error::Rejected {
        command: response.command.clone(),
        message: response.error.clone().unwrap_or_default(),
    }
}
