use std::{
    error::Error,
    fmt,
    future::{Future, poll_fn},
    mem,
    pin::pin,
    task::Poll,
    time::Duration,
};

use async_channel::{Receiver, RecvError};
use gpui::Task;
use gpui_operation::Transition;

use super::PreviewToken;

#[cfg(test)]
use super::ViewerMode;

pub(crate) type MediaTask = Task<()>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MediaMetadata {
    duration: Option<Duration>,
}

impl MediaMetadata {
    pub(crate) const fn new(duration: Option<Duration>) -> Self {
        Self { duration }
    }

    pub(crate) const fn duration(&self) -> Option<Duration> {
        self.duration
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MediaPosition {
    position: Duration,
    duration: Option<Duration>,
}

impl MediaPosition {
    pub(crate) const fn new(position: Duration, duration: Option<Duration>) -> Self {
        Self { position, duration }
    }

    pub(crate) const fn position(&self) -> Duration {
        self.position
    }

    pub(crate) const fn duration(&self) -> Option<Duration> {
        self.duration
    }

    fn is_valid(self) -> bool {
        self.duration
            .is_none_or(|duration| self.position <= duration)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaProblemKind {
    AssetRead,
    TemporaryAsset,
    RuntimeUnavailable,
    Decode,
    Control,
    Internal,
}

/// A deliberately redacted viewer-local media problem.
///
/// Detailed backend errors, response metadata, and temporary paths remain at
/// the adapter boundary; UI/i18n only receives this stable problem kind.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct MediaProblem {
    kind: MediaProblemKind,
}

impl MediaProblem {
    pub(crate) const fn new(kind: MediaProblemKind) -> Self {
        Self { kind }
    }

    pub(crate) const fn kind(self) -> MediaProblemKind {
        self.kind
    }
}

impl fmt::Debug for MediaProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaProblem")
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for MediaProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.kind {
            MediaProblemKind::AssetRead => "response body could not be copied for media preview",
            MediaProblemKind::TemporaryAsset => "media temporary asset could not be prepared",
            MediaProblemKind::RuntimeUnavailable => "media runtime is unavailable",
            MediaProblemKind::Decode => "media could not be decoded",
            MediaProblemKind::Control => "media control failed",
            MediaProblemKind::Internal => "media preview encountered an internal error",
        })
    }
}

impl Error for MediaProblem {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MediaCommand {
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    /// Requests a best-effort position update without changing playback.
    ///
    /// The response pane owns the timer that sends this message, so drivers do
    /// not create a detached polling task.
    PollPosition,
    Stop,
}

/// Adapter boundary for an already prepared, per-instance media pipeline.
///
/// Implementations must be abort-on-drop, must not block the UI thread in
/// `Drop`, and must report asynchronous pipeline failures through
/// [`MediaMessage::PlaybackFailed`]. No implementation may change global
/// process-wide decoder or output-device policy.
pub(crate) trait MediaDriver: Send {
    fn command(&mut self, command: MediaCommand) -> Result<(), MediaProblem>;
}

impl<T> MediaDriver for Box<T>
where
    T: MediaDriver + ?Sized,
{
    fn command(&mut self, command: MediaCommand) -> Result<(), MediaProblem> {
        (**self).command(command)
    }
}

/// A redacted asynchronous event emitted by a prepared media driver.
///
/// The response pane is the sole receiver owner. It maps each event to a
/// token-checked [`MediaMessage`] in an owner-bound GPUI task; drivers never
/// update GPUI entities directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaDriverEvent {
    Metadata(MediaMetadata),
    Position(MediaPosition),
    Ended,
    PlaybackFailed(MediaProblem),
}

/// The single-consumer event endpoint returned by an adapter's prepare facade.
///
/// This wrapper intentionally does not implement `Clone`: the response pane
/// moves it into its retained event-bridge task, which is dropped before the
/// driver and asset during preview teardown.
pub(crate) struct MediaDriverEvents {
    critical: Receiver<MediaDriverEvent>,
    telemetry: Receiver<MediaDriverEvent>,
}

impl MediaDriverEvents {
    #[cfg(test)]
    pub(crate) fn new(receiver: Receiver<MediaDriverEvent>) -> Self {
        let (telemetry_sender, telemetry) = async_channel::bounded(1);
        drop(telemetry_sender);
        Self {
            critical: receiver,
            telemetry,
        }
    }

    pub(crate) fn from_lanes(
        critical: Receiver<MediaDriverEvent>,
        telemetry: Receiver<MediaDriverEvent>,
    ) -> Self {
        Self {
            critical,
            telemetry,
        }
    }

    pub(crate) async fn recv(&self) -> Result<MediaDriverEvent, RecvError> {
        let critical = self.critical.recv();
        let telemetry = self.telemetry.recv();
        let mut critical = pin!(critical);
        let mut telemetry = pin!(telemetry);
        poll_fn(|cx| {
            let critical_result = critical.as_mut().poll(cx);
            if let Poll::Ready(Ok(event)) = critical_result {
                return Poll::Ready(Ok(event));
            }
            let telemetry_result = telemetry.as_mut().poll(cx);
            if let Poll::Ready(Ok(event)) = telemetry_result {
                return Poll::Ready(Ok(event));
            }
            if matches!(critical_result, Poll::Ready(Err(_)))
                && matches!(telemetry_result, Poll::Ready(Err(_)))
            {
                Poll::Ready(Err(RecvError))
            } else {
                Poll::Pending
            }
        })
        .await
    }
}

impl fmt::Debug for MediaDriverEvents {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MediaDriverEvents(..)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MediaPhase {
    Idle,
    Preparing,
    Paused,
    Playing,
    Ended,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlayIntent {
    Paused,
    Playing,
}

pub(crate) struct MediaPreparing<T = MediaTask> {
    token: PreviewToken,
    resume_position: Duration,
    intent: PlayIntent,
    volume: f32,
    muted: bool,
    _task: T,
}

pub(crate) struct MediaActive<T = MediaTask, D = Box<dyn MediaDriver>> {
    token: PreviewToken,
    // Fields are dropped in declaration order. Stop the event route before
    // releasing the driver. Concrete drivers transitively own their private
    // response asset until their pipeline has stopped.
    _task: T,
    driver: D,
    metadata: MediaMetadata,
    position: MediaPosition,
    volume: f32,
    muted: bool,
}

impl<T, D> MediaActive<T, D> {
    pub(crate) const fn metadata(&self) -> MediaMetadata {
        self.metadata
    }

    pub(crate) const fn position(&self) -> MediaPosition {
        self.position
    }

    pub(crate) const fn volume(&self) -> f32 {
        self.volume
    }

    pub(crate) const fn muted(&self) -> bool {
        self.muted
    }
}

pub(crate) struct MediaFailed {
    token: PreviewToken,
    problem: MediaProblem,
}

impl MediaFailed {
    fn from_preparing<T>(preparing: &MediaPreparing<T>, problem: MediaProblem) -> Self {
        Self {
            token: preparing.token.clone(),
            problem,
        }
    }

    fn from_active<T, D>(active: &MediaActive<T, D>, problem: MediaProblem) -> Self {
        Self {
            token: active.token.clone(),
            problem,
        }
    }
}

/// Complete media owner state. The active variant is the only owner of its
/// pipeline command bridge, completion task, and session-private asset.
#[derive(Default)]
pub(crate) enum MediaRuntime<T = MediaTask, D = Box<dyn MediaDriver>> {
    #[default]
    Idle,
    Preparing(MediaPreparing<T>),
    Paused(MediaActive<T, D>),
    Playing(MediaActive<T, D>),
    Ended(MediaActive<T, D>),
    Failed(MediaFailed),
}

impl<T, D> MediaRuntime<T, D> {
    pub(crate) const fn phase(&self) -> MediaPhase {
        match self {
            Self::Idle => MediaPhase::Idle,
            Self::Preparing(_) => MediaPhase::Preparing,
            Self::Paused(_) => MediaPhase::Paused,
            Self::Playing(_) => MediaPhase::Playing,
            Self::Ended(_) => MediaPhase::Ended,
            Self::Failed(_) => MediaPhase::Failed,
        }
    }

    pub(crate) fn token(&self) -> Option<&PreviewToken> {
        match self {
            Self::Idle => None,
            Self::Preparing(preparing) => Some(&preparing.token),
            Self::Paused(active) | Self::Playing(active) | Self::Ended(active) => {
                Some(&active.token)
            }
            Self::Failed(failed) => Some(&failed.token),
        }
    }

    pub(crate) fn active(&self) -> Option<&MediaActive<T, D>> {
        match self {
            Self::Paused(active) | Self::Playing(active) | Self::Ended(active) => Some(active),
            Self::Idle | Self::Preparing(_) | Self::Failed(_) => None,
        }
    }

    pub(crate) fn problem(&self) -> Option<MediaProblem> {
        match self {
            Self::Failed(failed) => Some(failed.problem),
            Self::Idle
            | Self::Preparing(_)
            | Self::Paused(_)
            | Self::Playing(_)
            | Self::Ended(_) => None,
        }
    }
}

fn active_runtime<T, D>(phase: MediaPhase, active: MediaActive<T, D>) -> MediaRuntime<T, D> {
    match phase {
        MediaPhase::Paused => MediaRuntime::Paused(active),
        MediaPhase::Playing => MediaRuntime::Playing(active),
        MediaPhase::Ended => MediaRuntime::Ended(active),
        MediaPhase::Idle | MediaPhase::Preparing | MediaPhase::Failed => {
            unreachable!("only active media phases can retain a MediaActive")
        }
    }
}

impl<T, D> fmt::Debug for MediaRuntime<T, D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("MediaRuntime");
        debug.field("phase", &self.phase());
        if let Some(token) = self.token() {
            debug.field("token", token);
        }
        if let Self::Failed(failed) = self {
            debug.field("problem", &failed.problem);
        }
        debug.finish()
    }
}

pub(crate) enum MediaMessage<T = MediaTask, D = Box<dyn MediaDriver>> {
    Start {
        token: PreviewToken,
        resume_position: Duration,
        resume_playing: bool,
        task: T,
    },
    Prepared {
        token: PreviewToken,
        driver: D,
        metadata: MediaMetadata,
        task: T,
    },
    PrepareFailed {
        token: PreviewToken,
        problem: MediaProblem,
    },
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    /// Internal owner-bound tick for a driver's best-effort position query.
    PollPosition,
    Metadata {
        token: PreviewToken,
        metadata: MediaMetadata,
    },
    Position {
        token: PreviewToken,
        position: MediaPosition,
    },
    Ended {
        token: PreviewToken,
    },
    PlaybackFailed {
        token: PreviewToken,
        problem: MediaProblem,
    },
    Stop,
}

impl<T, D> Transition<MediaMessage<T, D>> for &mut MediaRuntime<T, D>
where
    D: MediaDriver,
{
    type Output = ();

    fn transition(self, message: MediaMessage<T, D>) {
        let current_phase = self.phase();
        let current = mem::replace(self, MediaRuntime::Idle);

        match (current, message) {
            (
                MediaRuntime::Idle | MediaRuntime::Failed(_),
                MediaMessage::Start {
                    token,
                    resume_position,
                    resume_playing,
                    task,
                },
            ) => {
                *self = MediaRuntime::Preparing(MediaPreparing {
                    token,
                    resume_position,
                    intent: if resume_playing {
                        PlayIntent::Playing
                    } else {
                        PlayIntent::Paused
                    },
                    volume: 1.0,
                    muted: false,
                    _task: task,
                });
            }
            (
                MediaRuntime::Preparing(preparing),
                MediaMessage::Prepared {
                    token,
                    driver,
                    metadata,
                    task,
                },
            ) if preparing.token.matches(&token) => {
                let position = MediaPosition::new(preparing.resume_position, metadata.duration());
                let mut active = MediaActive {
                    token: preparing.token.clone(),
                    driver,
                    _task: task,
                    metadata,
                    position,
                    volume: preparing.volume,
                    muted: preparing.muted,
                };
                let initialized = if active.position.is_valid() {
                    initialize_active(&mut active, preparing.intent)
                } else {
                    Err(MediaProblem::new(MediaProblemKind::Control))
                };
                if initialized.is_ok() {
                    *self = match preparing.intent {
                        PlayIntent::Paused => MediaRuntime::Paused(active),
                        PlayIntent::Playing => MediaRuntime::Playing(active),
                    };
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
                drop(preparing);
            }
            (
                MediaRuntime::Preparing(preparing),
                MediaMessage::PrepareFailed { token, problem },
            ) if preparing.token.matches(&token) => {
                let failed = MediaFailed::from_preparing(&preparing, problem);
                *self = MediaRuntime::Failed(failed);
                drop(preparing);
            }
            (MediaRuntime::Paused(mut active), MediaMessage::Play) => {
                if active.driver.command(MediaCommand::Play).is_ok() {
                    *self = MediaRuntime::Playing(active);
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
            }
            (MediaRuntime::Playing(mut active), MediaMessage::Pause) => {
                if active.driver.command(MediaCommand::Pause).is_ok() {
                    *self = MediaRuntime::Paused(active);
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
            }
            (MediaRuntime::Ended(mut active), MediaMessage::Play) => {
                let seek = active.driver.command(MediaCommand::Seek(Duration::ZERO));
                let play = seek.and_then(|()| active.driver.command(MediaCommand::Play));
                if play.is_ok() {
                    active.position =
                        MediaPosition::new(Duration::ZERO, active.metadata.duration());
                    *self = MediaRuntime::Playing(active);
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::Seek(position),
            ) if active
                .metadata
                .duration()
                .is_some_and(|duration| position <= duration) =>
            {
                if active.driver.command(MediaCommand::Seek(position)).is_ok() {
                    active.position = MediaPosition::new(position, active.metadata.duration());
                    *self = active_runtime(current_phase, active);
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::SetVolume(volume),
            ) if volume.is_finite() && (0.0..=1.0).contains(&volume) => {
                if active
                    .driver
                    .command(MediaCommand::SetVolume(volume))
                    .is_ok()
                {
                    active.volume = volume;
                    *self = active_runtime(current_phase, active);
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::SetMuted(muted),
            ) => {
                if active.driver.command(MediaCommand::SetMuted(muted)).is_ok() {
                    active.muted = muted;
                    *self = active_runtime(current_phase, active);
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::PollPosition,
            ) => {
                if active.driver.command(MediaCommand::PollPosition).is_ok() {
                    *self = active_runtime(current_phase, active);
                } else {
                    let failed = MediaFailed::from_active(
                        &active,
                        MediaProblem::new(MediaProblemKind::Control),
                    );
                    *self = MediaRuntime::Failed(failed);
                    stop_active(&mut active, current_phase);
                    drop(active);
                }
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::Metadata { token, metadata },
            ) if active.token.matches(&token) => {
                let position = MediaPosition::new(active.position.position(), metadata.duration());
                if position.is_valid() {
                    active.metadata = metadata;
                    active.position = position;
                    *self = active_runtime(current_phase, active);
                } else {
                    *self = active_runtime(current_phase, active);
                }
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::Position { token, position },
            ) if active.token.matches(&token) && position.is_valid() => {
                active.position = position;
                *self = active_runtime(current_phase, active);
            }
            (
                MediaRuntime::Paused(active) | MediaRuntime::Playing(active),
                MediaMessage::Ended { token },
            ) if active.token.matches(&token) => {
                *self = MediaRuntime::Ended(active);
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::PlaybackFailed { token, problem },
            ) if active.token.matches(&token) => {
                let failed = MediaFailed::from_active(&active, problem);
                *self = MediaRuntime::Failed(failed);
                stop_active(&mut active, current_phase);
                drop(active);
            }
            (
                MediaRuntime::Paused(mut active)
                | MediaRuntime::Playing(mut active)
                | MediaRuntime::Ended(mut active),
                MediaMessage::Stop,
            ) => {
                *self = MediaRuntime::Idle;
                stop_active(&mut active, current_phase);
                drop(active);
            }
            (current, MediaMessage::Stop) => {
                *self = MediaRuntime::Idle;
                drop(current);
            }
            (current, message) => {
                *self = current;
                drop(message);
            }
        }
    }
}

fn initialize_active<T, D>(
    active: &mut MediaActive<T, D>,
    intent: PlayIntent,
) -> Result<(), MediaProblem>
where
    D: MediaDriver,
{
    active
        .driver
        .command(MediaCommand::SetVolume(active.volume))?;
    active
        .driver
        .command(MediaCommand::SetMuted(active.muted))?;
    if !active.position.position().is_zero() {
        active
            .driver
            .command(MediaCommand::Seek(active.position.position()))?;
    }
    if intent == PlayIntent::Playing {
        active.driver.command(MediaCommand::Play)?;
    }
    Ok(())
}

fn stop_active<T, D>(active: &mut MediaActive<T, D>, phase: MediaPhase)
where
    D: MediaDriver,
{
    if let Err(problem) = active.driver.command(MediaCommand::Stop) {
        tracing::debug!(?phase, kind = ?problem.kind(), "media driver stop command failed");
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
        time::Duration,
    };

    use bytes::Bytes;
    use gpui::{App, TestAppContext};
    use http::{HeaderMap, StatusCode, Version};
    use url::Url;

    use super::*;
    use crate::features::request::response::{
        BodyDecoding, CompletedBody, ResponseData, ResponseHead, ResponseSizes, ResponseTiming,
        StoredBody,
    };

    #[derive(Clone)]
    struct DropLog(Arc<Mutex<Vec<&'static str>>>);

    struct FakeTask(DropLog);

    impl Drop for FakeTask {
        fn drop(&mut self) {
            self.0.0.lock().unwrap().push("task");
        }
    }

    #[derive(Default)]
    struct PendingTaskControl {
        started: Cell<bool>,
        dropped: Cell<bool>,
        waker: RefCell<Option<Waker>>,
    }

    struct PendingTask {
        control: std::rc::Rc<PendingTaskControl>,
    }

    impl Future for PendingTask {
        type Output = ();

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.control.started.set(true);
            *self.control.waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    impl Drop for PendingTask {
        fn drop(&mut self) {
            self.control.dropped.set(true);
            self.control.waker.borrow_mut().take();
        }
    }

    struct FakeDriver {
        commands: Arc<Mutex<Vec<MediaCommand>>>,
        fail: bool,
        dropped: DropLog,
    }

    impl Drop for FakeDriver {
        fn drop(&mut self) {
            self.dropped.0.lock().unwrap().push("driver");
        }
    }

    impl MediaDriver for FakeDriver {
        fn command(&mut self, command: MediaCommand) -> Result<(), MediaProblem> {
            self.commands.lock().unwrap().push(command);
            if self.fail {
                Err(MediaProblem::new(MediaProblemKind::Control))
            } else {
                Ok(())
            }
        }
    }

    fn token(generation: u64) -> PreviewToken {
        let response = Arc::new(ResponseData::new(
            ResponseHead::new(
                StatusCode::OK,
                Version::HTTP_11,
                Url::parse("https://example.test/private?secret=value").unwrap(),
                HeaderMap::new(),
            ),
            ResponseTiming {
                head_after: Duration::from_millis(1),
                completed_after: Duration::from_millis(2),
            },
            CompletedBody {
                body: StoredBody::Memory(Bytes::from_static(b"media")),
                body_decoding: BodyDecoding::Identity,
                sizes: ResponseSizes {
                    declared_encoded_bytes: Some(5),
                    received_encoded_bytes: 5,
                    stored_body_bytes: 5,
                },
            },
        ));
        PreviewToken::new(response, ViewerMode::Audio, ViewerMode::Audio, generation)
    }

    fn task(log: &DropLog) -> FakeTask {
        FakeTask(log.clone())
    }

    #[test]
    fn preview_token_rejects_a_mode_change_for_the_same_response_generation() {
        let preview = token(1);
        let changed_mode = PreviewToken::new(
            Arc::clone(preview.response()),
            ViewerMode::Pdf,
            ViewerMode::Pdf,
            1,
        );

        assert!(!preview.matches(&changed_mode));
    }

    #[tokio::test]
    async fn prepared_media_starts_paused_and_accepts_transport_controls() {
        let log = DropLog(Arc::default());
        let commands = Arc::default();
        let preview = token(1);
        let mut runtime = MediaRuntime::<FakeTask, FakeDriver>::default();

        runtime.transition(MediaMessage::Start {
            token: preview.clone(),
            resume_position: Duration::ZERO,
            resume_playing: false,
            task: task(&log),
        });
        runtime.transition(MediaMessage::Prepared {
            token: preview.clone(),
            driver: FakeDriver {
                commands: Arc::clone(&commands),
                fail: false,
                dropped: log.clone(),
            },
            metadata: MediaMetadata::new(None),
            task: task(&log),
        });
        assert_eq!(runtime.phase(), MediaPhase::Paused);

        runtime.transition(MediaMessage::Metadata {
            token: preview.clone(),
            metadata: MediaMetadata::new(Some(Duration::from_secs(10))),
        });

        runtime.transition(MediaMessage::Play);
        runtime.transition(MediaMessage::Seek(Duration::from_secs(3)));
        runtime.transition(MediaMessage::Pause);
        assert_eq!(runtime.phase(), MediaPhase::Paused);
        assert_eq!(
            runtime.active().unwrap().position().position(),
            Duration::from_secs(3)
        );
        assert_eq!(
            *commands.lock().unwrap(),
            vec![
                MediaCommand::SetVolume(1.0),
                MediaCommand::SetMuted(false),
                MediaCommand::Play,
                MediaCommand::Seek(Duration::from_secs(3)),
                MediaCommand::Pause,
            ]
        );
    }

    #[tokio::test]
    async fn playing_after_end_seeks_to_start_before_resuming() {
        let log = DropLog(Arc::default());
        let commands = Arc::default();
        let preview = token(1);
        let mut runtime = MediaRuntime::<FakeTask, FakeDriver>::default();

        runtime.transition(MediaMessage::Start {
            token: preview.clone(),
            resume_position: Duration::ZERO,
            resume_playing: false,
            task: task(&log),
        });
        runtime.transition(MediaMessage::Prepared {
            token: preview.clone(),
            driver: FakeDriver {
                commands: Arc::clone(&commands),
                fail: false,
                dropped: log.clone(),
            },
            metadata: MediaMetadata::new(Some(Duration::from_secs(10))),
            task: task(&log),
        });
        runtime.transition(MediaMessage::Ended { token: preview });
        assert_eq!(runtime.phase(), MediaPhase::Ended);

        runtime.transition(MediaMessage::Play);

        assert_eq!(runtime.phase(), MediaPhase::Playing);
        assert_eq!(
            *commands.lock().unwrap(),
            vec![
                MediaCommand::SetVolume(1.0),
                MediaCommand::SetMuted(false),
                MediaCommand::Seek(Duration::ZERO),
                MediaCommand::Play,
            ]
        );
    }

    #[tokio::test]
    async fn driver_events_have_one_typed_receiver_owner() {
        let (sender, receiver) = async_channel::unbounded();
        let events = MediaDriverEvents::new(receiver);
        sender
            .send(MediaDriverEvent::PlaybackFailed(MediaProblem::new(
                MediaProblemKind::Decode,
            )))
            .await
            .unwrap();

        assert_eq!(
            events.recv().await.unwrap(),
            MediaDriverEvent::PlaybackFailed(MediaProblem::new(MediaProblemKind::Decode)),
        );
    }

    #[tokio::test]
    async fn stale_completion_keeps_the_current_preparation() {
        let log = DropLog(Arc::default());
        let preview = token(1);
        let stale = token(2);
        let mut runtime = MediaRuntime::<FakeTask, FakeDriver>::default();
        runtime.transition(MediaMessage::Start {
            token: preview.clone(),
            resume_position: Duration::ZERO,
            resume_playing: false,
            task: task(&log),
        });
        runtime.transition(MediaMessage::Prepared {
            token: stale,
            driver: FakeDriver {
                commands: Arc::default(),
                fail: false,
                dropped: log.clone(),
            },
            metadata: MediaMetadata::new(None),
            task: task(&log),
        });

        assert_eq!(runtime.phase(), MediaPhase::Preparing);
        assert!(runtime.token().unwrap().matches(&preview));
    }

    #[tokio::test]
    async fn playback_failure_is_terminal_and_stops_the_driver() {
        let log = DropLog(Arc::default());
        let commands = Arc::default();
        let preview = token(1);
        let mut runtime = MediaRuntime::<FakeTask, FakeDriver>::default();
        runtime.transition(MediaMessage::Start {
            token: preview.clone(),
            resume_position: Duration::ZERO,
            resume_playing: false,
            task: task(&log),
        });
        runtime.transition(MediaMessage::Prepared {
            token: preview.clone(),
            driver: FakeDriver {
                commands: Arc::clone(&commands),
                fail: false,
                dropped: log.clone(),
            },
            metadata: MediaMetadata::new(Some(Duration::from_secs(10))),
            task: task(&log),
        });
        runtime.transition(MediaMessage::PlaybackFailed {
            token: preview,
            problem: MediaProblem::new(MediaProblemKind::Decode),
        });

        assert_eq!(runtime.phase(), MediaPhase::Failed);
        assert_eq!(
            runtime.problem(),
            Some(MediaProblem::new(MediaProblemKind::Decode))
        );
        assert_eq!(
            *commands.lock().unwrap(),
            vec![
                MediaCommand::SetVolume(1.0),
                MediaCommand::SetMuted(false),
                MediaCommand::Stop,
            ]
        );
    }

    #[gpui::test]
    fn stopping_preparation_cancels_its_owner_task_and_becomes_idle(cx: &mut TestAppContext) {
        let control = std::rc::Rc::new(PendingTaskControl::default());
        let task = cx.update(|cx: &mut App| {
            let pending = PendingTask {
                control: control.clone(),
            };
            cx.spawn(async move |_cx| pending.await)
        });
        let mut runtime = MediaRuntime::<MediaTask, FakeDriver>::default();

        runtime.transition(MediaMessage::Start {
            token: token(1),
            resume_position: Duration::ZERO,
            resume_playing: false,
            task,
        });
        cx.run_until_parked();
        assert!(control.started.get(), "preparation task must reach Pending");
        assert!(!control.dropped.get());

        runtime.transition(MediaMessage::Stop);
        cx.run_until_parked();

        assert_eq!(runtime.phase(), MediaPhase::Idle);
        assert!(
            control.dropped.get(),
            "stopping preparation must drop and cancel its sole owner task"
        );
    }

    #[tokio::test]
    async fn stop_drops_session_resources_after_installing_idle() {
        let log = DropLog(Arc::default());
        let commands = Arc::default();
        let preview = token(1);
        let mut runtime = MediaRuntime::<FakeTask, FakeDriver>::default();
        runtime.transition(MediaMessage::Start {
            token: preview.clone(),
            resume_position: Duration::ZERO,
            resume_playing: false,
            task: task(&log),
        });
        runtime.transition(MediaMessage::Prepared {
            token: preview,
            driver: FakeDriver {
                commands: Arc::clone(&commands),
                fail: false,
                dropped: log.clone(),
            },
            metadata: MediaMetadata::new(None),
            task: task(&log),
        });

        runtime.transition(MediaMessage::Stop);
        assert_eq!(runtime.phase(), MediaPhase::Idle);
        assert_eq!(
            *commands.lock().unwrap(),
            vec![
                MediaCommand::SetVolume(1.0),
                MediaCommand::SetMuted(false),
                MediaCommand::Stop,
            ]
        );
        assert!(log.0.lock().unwrap().contains(&"driver"));
        assert!(
            log.0
                .lock()
                .unwrap()
                .iter()
                .filter(|entry| **entry == "task")
                .count()
                >= 2
        );
    }
}
