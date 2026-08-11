//! `gpui-video-player` adapter for one response-owned video preview.
//!
//! Preparation is asynchronous and leaves the pipeline paused. The adapter
//! owns the only event-forwarding task and aborts it before shutdown. The
//! response asset is moved into the driver and is released by a terminal
//! cleanup thread only after the fork's [`VideoStop`] confirms shutdown.

use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel},
    },
    thread::JoinHandle,
    time::Duration,
};

use async_channel::{Receiver as EventReceiver, Sender as EventSender};
use gpui_video_player::{
    DecoderPolicy as ForkDecoderPolicy, Error as VideoError, PreparedVideo, Video, VideoEvent,
    VideoEventReceiver, VideoOptions, VideoRuntimeProblem, VideoStop,
};

use super::{
    DecoderPolicy, MediaCommand, MediaDriver, MediaDriverEvent, MediaDriverEvents, MediaMetadata,
    MediaPosition, MediaProblem, MediaProblemKind, ResponseAssetLease,
};

const EVENT_CHANNEL_CAPACITY: usize = 4;
const CRITICAL_EVENT_CHANNEL_CAPACITY: usize = 4;
const PRESENTATION_QUEUE_CAPACITY: usize = 2;
const COMMAND_CHANNEL_CAPACITY: usize = 16;
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(crate) type VideoHandle = Video;

/// A prepared, paused video pipeline and its response-pane event endpoint.
pub(crate) struct VideoPrepared {
    driver: VideoDriver,
    events: MediaDriverEvents,
    metadata: MediaMetadata,
}

impl VideoPrepared {
    pub(crate) fn into_parts(self) -> (VideoDriver, MediaDriverEvents, MediaMetadata) {
        (self.driver, self.events, self.metadata)
    }
}

/// Per-preview controller for a fork-owned GStreamer pipeline.
///
/// `asset` intentionally lives here instead of beside the driver in
/// `MediaActive`: the fork stops on a background thread, so dropping a sibling
/// asset immediately after issuing `Stop` could unlink the file while
/// GStreamer still has it open. `begin_cleanup` transfers both `VideoStop` and
/// the asset into the driver worker, which obtains and waits for `VideoStop`.
pub(crate) struct VideoDriver {
    video: Video,
    asset: Option<ResponseAssetLease>,
    commands: SyncSender<MediaCommand>,
    stop: SyncSender<ResponseAssetLease>,
    command_worker: Option<JoinHandle<()>>,
    event_task: Option<tokio::task::JoinHandle<()>>,
}

impl VideoDriver {
    /// Prepares one video pipeline without blocking the caller's executor.
    ///
    /// The fork performs GStreamer initialization and preroll on its dedicated
    /// preparation thread. Dropping this future cancels that preparation. The
    /// returned pipeline is paused and never starts playback automatically.
    pub(crate) async fn prepare(
        asset: ResponseAssetLease,
        decoder_policy: DecoderPolicy,
    ) -> Result<VideoPrepared, MediaProblem> {
        let uri = asset.uri().clone();
        let options = VideoOptions {
            frame_buffer_capacity: Some(PRESENTATION_QUEUE_CAPACITY),
            looping: Some(false),
            speed: Some(1.0),
            decoder_policy: map_decoder_policy(decoder_policy),
        };
        let preparation = start_preparation_owner(asset, uri, options)?;
        let PreparationOwnerResult { asset, result } = preparation
            .wait()
            .await
            .map_err(|_| MediaProblem::new(MediaProblemKind::Internal))?;
        let PreparedVideo { video, events } = match result {
            Ok(prepared) => prepared,
            Err(problem) => {
                // The owner receives a failure only after the fork's
                // preparation guard has returned the pipeline to Null.
                return Err(map_prepare_problem(problem));
            }
        };

        // `PreparedVideo` is contractually preroll-complete and paused. Do not
        // re-enter GStreamer on the awaiting executor merely to reassert it.
        video.set_frame_buffer_capacity(PRESENTATION_QUEUE_CAPACITY);

        let metadata = MediaMetadata::new(optional_duration(video.duration()));
        let (critical_sender, critical_receiver) =
            async_channel::bounded::<MediaDriverEvent>(CRITICAL_EVENT_CHANNEL_CAPACITY);
        let (telemetry_sender, telemetry_receiver) =
            async_channel::bounded::<MediaDriverEvent>(EVENT_CHANNEL_CAPACITY);
        let CommandWorkerHandle {
            commands,
            stop,
            worker: command_worker,
        } = match spawn_command_worker(
            video.clone(),
            critical_sender.clone(),
            telemetry_sender.clone(),
        ) {
            Ok(worker) => worker,
            Err(()) => {
                begin_direct_cleanup(video, asset);
                return Err(MediaProblem::new(MediaProblemKind::Internal));
            }
        };
        let event_task = tokio::spawn(forward_events(
            events,
            critical_sender,
            telemetry_sender,
            video.clone(),
        ));
        let driver = Self {
            video,
            asset: Some(asset),
            commands,
            stop,
            command_worker: Some(command_worker),
            event_task: Some(event_task),
        };

        Ok(VideoPrepared {
            driver,
            events: MediaDriverEvents::from_lanes(critical_receiver, telemetry_receiver),
            metadata,
        })
    }

    fn begin_cleanup(&mut self) -> Result<(), MediaProblem> {
        if let Some(task) = self.event_task.take() {
            task.abort();
        }
        let Some(_) = self.asset.as_ref() else {
            return Ok(());
        };

        let asset = self
            .asset
            .take()
            .expect("the asset was checked before starting video cleanup");
        match self.stop.try_send(asset) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(asset) | TrySendError::Disconnected(asset)) => {
                self.asset = Some(asset);
                Err(MediaProblem::new(MediaProblemKind::Control))
            }
        }
    }
}

struct PreparationOwnerResult {
    asset: ResponseAssetLease,
    result: Result<PreparedVideo, VideoError>,
}

struct PreparationOwner {
    result: Option<EventReceiver<PreparationOwnerResult>>,
    worker: Option<JoinHandle<()>>,
}

impl PreparationOwner {
    async fn wait(mut self) -> Result<PreparationOwnerResult, async_channel::RecvError> {
        let result = self
            .result
            .as_ref()
            .expect("preparation result receiver is present while waiting")
            .recv()
            .await;
        self.result.take();
        if let Some(worker) = self.worker.take() {
            spawn_terminal_join(worker);
        }
        result
    }
}

impl Drop for PreparationOwner {
    fn drop(&mut self) {
        // Closing the result route first makes the worker clean any prepared
        // pipeline and its asset instead of leaving an unread success value.
        self.result.take();
        if let Some(worker) = self.worker.take() {
            spawn_terminal_join(worker);
        }
    }
}

fn start_preparation_owner(
    asset: ResponseAssetLease,
    uri: url::Url,
    options: VideoOptions,
) -> Result<PreparationOwner, MediaProblem> {
    let (completion, result) = async_channel::bounded(1);
    let worker = std::thread::Builder::new()
        .name("http-client-video-prepare".into())
        .spawn(move || {
            let prepared = Video::prepare_with_options(&uri, options)
                .and_then(|preparation| preparation.wait_blocking());
            let completion_value = PreparationOwnerResult {
                asset,
                result: prepared,
            };
            if let Err(unsent) = completion.send_blocking(completion_value) {
                let PreparationOwnerResult { asset, result } = unsent.0;
                if let Ok(prepared) = result {
                    stop_video(prepared.video, asset);
                }
                // On preparation failure the fork has already returned its
                // guarded pipeline to Null, so the remaining asset drops here.
            }
        })
        .map_err(|_| MediaProblem::new(MediaProblemKind::Internal))?;
    Ok(PreparationOwner {
        result: Some(result),
        worker: Some(worker),
    })
}

impl MediaDriver for VideoDriver {
    fn command(&mut self, command: MediaCommand) -> Result<(), MediaProblem> {
        match command {
            MediaCommand::Stop => self.begin_cleanup(),
            command => enqueue_command(&self.commands, command),
        }
    }

    fn video_handle(&self) -> Option<VideoHandle> {
        Some(self.video.clone())
    }
}

fn enqueue_command(
    commands: &SyncSender<MediaCommand>,
    command: MediaCommand,
) -> Result<(), MediaProblem> {
    commands.try_send(command).map_err(|_| {
        // A full or disconnected bounded queue means the controller cannot
        // honor this user action; never block the GPUI thread.
        MediaProblem::new(MediaProblemKind::Control)
    })
}

impl Drop for VideoDriver {
    fn drop(&mut self) {
        if self.begin_cleanup().is_err()
            && let Some(asset) = self.asset.take()
        {
            // A failure to construct the fork's stop handle leaves shutdown
            // unconfirmed. Leaking this bounded, private temporary asset is
            // safer than unlinking a file a native pipeline may still read.
            std::mem::forget(asset);
        }
        if let Some(worker) = self.command_worker.take() {
            spawn_terminal_join(worker);
        }
    }
}

async fn forward_events(
    events: VideoEventReceiver,
    critical: EventSender<MediaDriverEvent>,
    telemetry: EventSender<MediaDriverEvent>,
    video: Video,
) {
    while let Ok(event) = events.recv().await {
        match event {
            VideoEvent::Position { position, duration } => {
                let _ = telemetry.try_send(MediaDriverEvent::Position(MediaPosition::new(
                    position,
                    optional_duration(duration),
                )));
            }
            // Frame availability must wake the response pane, but it is not a
            // second state authority. A coalescible position event causes the
            // same owner-bound notify while the Video element consumes the
            // fork's latest bounded frame.
            VideoEvent::FrameAvailable => {
                let _ = telemetry.try_send(MediaDriverEvent::Position(current_position(&video)));
            }
            VideoEvent::Ended => {
                if critical.send(MediaDriverEvent::Ended).await.is_err() {
                    break;
                }
            }
            VideoEvent::Error(problem) => {
                if critical
                    .send(MediaDriverEvent::PlaybackFailed(map_runtime_problem(
                        problem,
                    )))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            VideoEvent::Stopped => break,
        }
    }
}

struct CommandWorkerHandle {
    commands: SyncSender<MediaCommand>,
    stop: SyncSender<ResponseAssetLease>,
    worker: JoinHandle<()>,
}

fn spawn_command_worker(
    video: Video,
    critical: EventSender<MediaDriverEvent>,
    telemetry: EventSender<MediaDriverEvent>,
) -> Result<CommandWorkerHandle, ()> {
    let (command_sender, commands) = sync_channel(COMMAND_CHANNEL_CAPACITY);
    let (stop_sender, stop) = sync_channel(1);
    let worker = std::thread::Builder::new()
        .name("http-client-video-driver".into())
        .spawn(move || run_command_worker(video, critical, telemetry, commands, stop))
        .map_err(|_| ())?;
    Ok(CommandWorkerHandle {
        commands: command_sender,
        stop: stop_sender,
        worker,
    })
}

fn run_command_worker(
    video: Video,
    critical: EventSender<MediaDriverEvent>,
    telemetry: EventSender<MediaDriverEvent>,
    commands: Receiver<MediaCommand>,
    stop: Receiver<ResponseAssetLease>,
) {
    loop {
        if let Ok(asset) = stop.try_recv() {
            stop_video(video, asset);
            return;
        }

        let command = match commands.recv_timeout(COMMAND_POLL_INTERVAL) {
            Ok(command) => command,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                // Driver Drop normally delivers the asset through `stop`
                // before this sender closes. If construction is unwinding,
                // still stop the native pipeline; the caller retains/leaks
                // the asset according to the same conservative rule.
                stop_video_without_asset(video);
                return;
            }
        };
        let result = execute_command(&video, command, &telemetry);
        if let Err(problem) = result
            && critical
                .send_blocking(MediaDriverEvent::PlaybackFailed(problem))
                .is_err()
        {
            return;
        }
    }
}

fn execute_command(
    video: &Video,
    command: MediaCommand,
    telemetry: &EventSender<MediaDriverEvent>,
) -> Result<(), MediaProblem> {
    match command {
        MediaCommand::Play => video
            .set_paused(false)
            .map_err(|_| MediaProblem::new(MediaProblemKind::Control)),
        MediaCommand::Pause => video
            .set_paused(true)
            .map_err(|_| MediaProblem::new(MediaProblemKind::Control)),
        MediaCommand::Seek(position) => {
            video
                .seek(position, false)
                .map_err(|_| MediaProblem::new(MediaProblemKind::Control))?;
            send_position(video, telemetry);
            Ok(())
        }
        MediaCommand::SetVolume(volume) => {
            video.set_volume(f64::from(volume));
            Ok(())
        }
        MediaCommand::SetMuted(muted) => {
            video.set_muted(muted);
            Ok(())
        }
        MediaCommand::PollPosition => {
            send_position(video, telemetry);
            Ok(())
        }
        MediaCommand::Stop => Err(MediaProblem::new(MediaProblemKind::Internal)),
    }
}

fn send_position(video: &Video, telemetry: &EventSender<MediaDriverEvent>) {
    let _ = telemetry.try_send(MediaDriverEvent::Position(current_position(video)));
}

fn stop_video(video: Video, asset: ResponseAssetLease) {
    let stopped = video.stop().and_then(VideoStop::wait_blocking).is_ok();
    if stopped {
        drop(asset);
    } else {
        // Shutdown could not be proven. Preserve the private file rather than
        // racing a native reader.
        std::mem::forget(asset);
    }
}

fn stop_video_without_asset(video: Video) {
    if let Ok(stop) = video.stop() {
        let _ = stop.wait_blocking();
    }
}

fn begin_direct_cleanup(video: Video, asset: ResponseAssetLease) {
    let stop = match video.stop() {
        Ok(stop) => stop,
        Err(_) => {
            // Without a stop handle native shutdown cannot be proven.
            std::mem::forget(asset);
            return;
        }
    };
    struct Cleanup {
        stop: VideoStop,
        asset: ResponseAssetLease,
    }

    let cleanup = Arc::new(Mutex::new(Some(Cleanup { stop, asset })));
    let cleanup_for_worker = Arc::clone(&cleanup);
    let spawned = std::thread::Builder::new()
        .name("http-client-video-direct-cleanup".into())
        .spawn(move || {
            let cleanup = cleanup_for_worker
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            let Some(Cleanup { stop, asset }) = cleanup else {
                return;
            };
            if stop.wait_blocking().is_ok() {
                drop(asset);
            } else {
                std::mem::forget(asset);
            }
        });
    if spawned.is_err() {
        // The outer Arc recovers the payload when Builder drops its closure.
        // Preserve the asset because the fork's own stop thread is still
        // running without an app-side waiter.
        if let Some(Cleanup { stop: _, asset }) = cleanup
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            std::mem::forget(asset);
        }
    }
}

fn spawn_terminal_join(worker: JoinHandle<()>) {
    let worker = Arc::new(Mutex::new(Some(worker)));
    let worker_for_join = Arc::clone(&worker);
    let spawned = std::thread::Builder::new()
        .name("http-client-video-cleanup".into())
        .spawn(move || {
            if let Some(worker) = worker_for_join
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take()
            {
                let _ = worker.join();
            }
        });
    if spawned.is_err() {
        // A thread creation failure leaves the command worker detached, but it
        // still owns the stop/asset sequence. The owner-bound event receiver
        // was dropped first, so the worker has no live GPUI completion route.
        drop(
            worker
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take(),
        );
    }
    // This is a terminal disposer, not a detached producer: it has no sender,
    // cannot update GPUI, and exits after joining the already-stopping command
    // worker. The command worker releases its asset only after VideoStop.
}

const fn map_decoder_policy(policy: DecoderPolicy) -> ForkDecoderPolicy {
    match policy {
        DecoderPolicy::Auto => ForkDecoderPolicy::Auto,
        DecoderPolicy::SoftwareOnly => ForkDecoderPolicy::SoftwareOnly,
    }
}

fn current_position(video: &Video) -> MediaPosition {
    MediaPosition::new(video.position(), optional_duration(video.duration()))
}

const fn optional_duration(duration: Duration) -> Option<Duration> {
    if duration.is_zero() {
        None
    } else {
        Some(duration)
    }
}

const fn map_runtime_problem(problem: VideoRuntimeProblem) -> MediaProblem {
    match problem {
        VideoRuntimeProblem::Pipeline
        | VideoRuntimeProblem::Frame
        | VideoRuntimeProblem::LoopRestart => MediaProblem::new(MediaProblemKind::Decode),
    }
}

fn map_prepare_problem(problem: VideoError) -> MediaProblem {
    match problem {
        VideoError::Glib => MediaProblem::new(MediaProblemKind::RuntimeUnavailable),
        VideoError::AppSink(_) => MediaProblem::plugin("appsink"),
        VideoError::Bus | VideoError::Bool | VideoError::Cast => {
            MediaProblem::new(MediaProblemKind::Internal)
        }
        VideoError::Caps | VideoError::Framerate(_) | VideoError::Duration => {
            MediaProblem::new(MediaProblemKind::UnsupportedMedia)
        }
        VideoError::ResolutionUnsupported { width, height } => {
            MediaProblem::resolution(width, height)
        }
        VideoError::StateChange | VideoError::Sync | VideoError::PreparationTimeout => {
            MediaProblem::new(MediaProblemKind::Decode)
        }
        VideoError::Io
        | VideoError::Uri
        | VideoError::Cancelled
        | VideoError::PreparationChannelClosed
        | VideoError::StopChannelClosed
        | VideoError::WorkerPanicked
        | VideoError::Lock => MediaProblem::new(MediaProblemKind::Internal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_policy_is_mapped_without_process_global_state() {
        assert_eq!(
            map_decoder_policy(DecoderPolicy::Auto),
            ForkDecoderPolicy::Auto
        );
        assert_eq!(
            map_decoder_policy(DecoderPolicy::SoftwareOnly),
            ForkDecoderPolicy::SoftwareOnly
        );
    }

    #[test]
    fn preparation_errors_are_redacted_and_keep_resolution_kind() {
        let resolution = map_prepare_problem(VideoError::ResolutionUnsupported {
            width: 3_840,
            height: 2_160,
        });
        assert_eq!(resolution.kind(), MediaProblemKind::ResolutionUnsupported);
        assert_eq!(
            resolution.detail(),
            Some(super::super::MediaProblemDetail::Resolution {
                width: 3_840,
                height: 2_160,
            })
        );
        assert_eq!(
            map_prepare_problem(VideoError::Glib).kind(),
            MediaProblemKind::RuntimeUnavailable
        );
        let plugin = map_prepare_problem(VideoError::AppSink("private-pipeline-name".into()));
        assert_eq!(
            plugin.detail(),
            Some(super::super::MediaProblemDetail::Plugin("appsink"))
        );
        assert!(!format!("{plugin:?}").contains("private-pipeline-name"));
    }

    #[test]
    fn zero_duration_is_treated_as_unknown() {
        assert_eq!(optional_duration(Duration::ZERO), None);
        assert_eq!(
            optional_duration(Duration::from_secs(1)),
            Some(Duration::from_secs(1))
        );
    }

    #[test]
    fn adapter_lifecycle_handles_are_send() {
        fn assert_send<T: Send>() {}

        assert_send::<VideoDriver>();
        assert_send::<VideoPrepared>();
        assert_send::<VideoHandle>();
    }

    #[test]
    fn command_enqueue_is_bounded_and_never_waits_for_capacity() {
        let (sender, receiver) = sync_channel(1);
        enqueue_command(&sender, MediaCommand::Play).unwrap();

        let problem = enqueue_command(&sender, MediaCommand::Pause).unwrap_err();
        assert_eq!(problem.kind(), MediaProblemKind::Control);
        assert_eq!(receiver.recv().unwrap(), MediaCommand::Play);
    }
}
