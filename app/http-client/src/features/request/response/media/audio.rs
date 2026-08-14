//! Rodio-backed, audio-only media adapter.
//!
//! Construction and decoder probing run on the response pane's background
//! preparation task. After preparation, the GPUI owner only enqueues typed
//! commands; a bounded, driver-owned worker performs every Rodio control and
//! position query. The worker transitively owns the output stream and private
//! response asset, and releases the asset only after playback has stopped.

use std::{
    io::BufReader,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source as _};

use super::{
    MediaCommand, MediaDriver, MediaDriverEvent, MediaDriverEvents, MediaMetadata, MediaPosition,
    MediaProblem, MediaProblemKind, ResponseAssetLease,
};

type AudioDecoder = Decoder<BufReader<std::fs::File>>;

/// A prepared audio player and its single-consumer event endpoint.
///
/// The backend worker owns the `ResponseAssetLease`; no URI or path crosses
/// back into the response pane.
pub(crate) struct AudioPrepared {
    driver: AudioDriver,
    events: MediaDriverEvents,
    metadata: MediaMetadata,
}

impl AudioPrepared {
    pub(crate) fn into_parts(self) -> (AudioDriver, MediaDriverEvents, MediaMetadata) {
        (self.driver, self.events, self.metadata)
    }
}

/// Per-preview Rodio audio controller.
pub(crate) struct AudioDriver {
    commands: Sender<MediaCommand>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AudioDriver {
    /// Opens the private response asset, validates its decoder, initializes the
    /// default output device, and leaves the resulting player paused.
    ///
    /// The caller must invoke this from the pane's background preparation task.
    pub(crate) fn prepare(asset: ResponseAssetLease) -> Result<AudioPrepared, MediaProblem> {
        let (decoder, metadata) = decode_asset(&asset)?;
        let (command_sender, command_receiver) = async_channel::bounded(16);
        let (critical_sender, critical_receiver) = async_channel::bounded(4);
        let (telemetry_sender, telemetry_receiver) = async_channel::bounded(1);
        let stop = Arc::new(AtomicBool::new(false));
        let device_failed = Arc::new(AtomicBool::new(false));
        let device_failed_callback = Arc::clone(&device_failed);
        let device_critical = critical_sender.clone();

        let builder = DeviceSinkBuilder::from_default_device()
            .map_err(|_| MediaProblem::new(MediaProblemKind::RuntimeUnavailable))?
            .with_error_callback(move |_| {
                device_failed_callback.store(true, Ordering::Release);
                // A CPAL device callback must never block the audio thread.
                let _ = device_critical.try_send(MediaDriverEvent::PlaybackFailed(
                    MediaProblem::new(MediaProblemKind::Control),
                ));
            });
        let mut output = builder
            .open_sink_or_fallback()
            .map_err(|_| MediaProblem::new(MediaProblemKind::RuntimeUnavailable))?;
        output.log_on_drop(false);

        let player = Player::connect_new(output.mixer());
        // Pause before appending the decoded source so preparation can never
        // produce an audible sample.
        player.pause();
        player.append(decoder);

        let backend_stop = Arc::clone(&stop);
        let backend = AudioBackend {
            player,
            _output: output,
            gain: AudioGain::default(),
            metadata,
            ended: false,
            asset,
        };
        let worker = std::thread::Builder::new()
            .name("http-client-audio-preview".into())
            .spawn(move || {
                run_audio_backend(
                    backend,
                    command_receiver,
                    backend_stop,
                    device_failed,
                    critical_sender,
                    telemetry_sender,
                );
            })
            .map_err(|_| MediaProblem::new(MediaProblemKind::Internal))?;

        Ok(AudioPrepared {
            driver: Self {
                commands: command_sender,
                stop,
                worker: Some(worker),
            },
            events: MediaDriverEvents::from_lanes(critical_receiver, telemetry_receiver),
            metadata,
        })
    }
}

impl MediaDriver for AudioDriver {
    fn command(&mut self, command: MediaCommand) -> Result<(), MediaProblem> {
        if command == MediaCommand::Stop {
            self.stop.store(true, Ordering::Release);
            return Ok(());
        }
        match self.commands.try_send(command) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) | Err(TrySendError::Closed(_)) => {
                Err(MediaProblem::new(MediaProblemKind::Control))
            }
        }
    }
}

impl Drop for AudioDriver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Never join on the GPUI thread. The worker observes `stop`, stops the
        // player, then releases the output stream and private asset.
        self.worker.take();
    }
}

struct AudioBackend {
    player: Player,
    _output: MixerDeviceSink,
    gain: AudioGain,
    metadata: MediaMetadata,
    ended: bool,
    asset: ResponseAssetLease,
}

impl AudioBackend {
    fn reload_source(&mut self) -> Result<(), MediaProblem> {
        let (decoder, metadata) = decode_asset(&self.asset)?;
        self.player.append(decoder);
        self.metadata = metadata;
        self.ended = false;
        Ok(())
    }
}

impl Drop for AudioBackend {
    fn drop(&mut self) {
        self.player.stop();
    }
}

#[derive(Clone, Copy, Debug)]
struct AudioGain {
    volume: f32,
    muted: bool,
}

impl Default for AudioGain {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
        }
    }
}

impl AudioGain {
    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    fn effective(self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }
}

fn decode_asset(asset: &ResponseAssetLease) -> Result<(AudioDecoder, MediaMetadata), MediaProblem> {
    let file = asset
        .open()
        .map_err(|_| MediaProblem::new(MediaProblemKind::Decode))?;
    let decoder =
        Decoder::try_from(file).map_err(|_| MediaProblem::new(MediaProblemKind::Decode))?;
    let metadata = MediaMetadata::new(decoder.total_duration());
    Ok((decoder, metadata))
}

fn run_audio_backend(
    mut backend: AudioBackend,
    commands: Receiver<MediaCommand>,
    stop: Arc<AtomicBool>,
    device_failed: Arc<AtomicBool>,
    critical: Sender<MediaDriverEvent>,
    telemetry: Sender<MediaDriverEvent>,
) {
    send_telemetry(&telemetry, MediaDriverEvent::Metadata(backend.metadata));

    while !stop.load(Ordering::Acquire) && !device_failed.load(Ordering::Acquire) {
        loop {
            match commands.try_recv() {
                Ok(command) => {
                    if let Err(problem) = execute_audio_command(&mut backend, command, &telemetry) {
                        send_critical(&critical, MediaDriverEvent::PlaybackFailed(problem));
                        stop.store(true, Ordering::Release);
                        break;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => {
                    stop.store(true, Ordering::Release);
                    break;
                }
            }
        }

        if !stop.load(Ordering::Acquire)
            && !device_failed.load(Ordering::Acquire)
            && !backend.ended
            && backend.player.empty()
        {
            // Keep the worker alive so the Ended -> Seek(0) -> Play state
            // transition can reopen the worker-owned asset and replay it.
            backend.player.pause();
            backend.ended = true;
            send_critical(&critical, MediaDriverEvent::Ended);
        }

        if !stop.load(Ordering::Acquire) && !device_failed.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn execute_audio_command(
    backend: &mut AudioBackend,
    command: MediaCommand,
    telemetry: &Sender<MediaDriverEvent>,
) -> Result<(), MediaProblem> {
    match command {
        MediaCommand::Play => {
            backend.player.play();
            Ok(())
        }
        MediaCommand::Pause => {
            backend.player.pause();
            Ok(())
        }
        MediaCommand::Seek(position) => {
            if backend.player.empty() {
                backend.reload_source()?;
            }
            backend
                .player
                .try_seek(position)
                .map_err(|_| MediaProblem::new(MediaProblemKind::Control))?;
            emit_position(backend, telemetry);
            Ok(())
        }
        MediaCommand::SetVolume(volume) => {
            backend.gain.set_volume(volume);
            backend.player.set_volume(backend.gain.effective());
            Ok(())
        }
        MediaCommand::SetMuted(muted) => {
            backend.gain.set_muted(muted);
            backend.player.set_volume(backend.gain.effective());
            Ok(())
        }
        MediaCommand::PollPosition => {
            emit_position(backend, telemetry);
            Ok(())
        }
        MediaCommand::Stop => Ok(()),
    }
}

fn emit_position(backend: &AudioBackend, telemetry: &Sender<MediaDriverEvent>) {
    send_telemetry(
        telemetry,
        MediaDriverEvent::Position(MediaPosition::new(
            backend.player.get_pos(),
            backend.metadata.duration(),
        )),
    );
}

fn send_telemetry(sender: &Sender<MediaDriverEvent>, event: MediaDriverEvent) {
    let _ = sender.try_send(event);
}

fn send_critical(sender: &Sender<MediaDriverEvent>, event: MediaDriverEvent) {
    // This runs only on the driver worker. Backpressure is preferable to
    // losing EOS or a terminal control failure, and dropping the pane closes
    // the receiver so shutdown never waits for a vanished UI owner.
    let _ = sender.send_blocking(event);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bytes::Bytes;
    use http::{HeaderMap, StatusCode, Version};
    use url::Url;

    use super::*;
    use crate::features::request::response::{
        BodyDecoding, CompletedBody, ResponseData, ResponseHead, ResponseSizes, ResponseTiming,
        StoredBody,
    };

    #[tokio::test]
    async fn wav_fixture_is_decoded_without_opening_an_output_device() {
        let response = response(wav_fixture());
        let asset = response
            .read_lease()
            .materialize_media_asset()
            .await
            .unwrap();
        let (_decoder, metadata) = decode_asset(&asset).unwrap();

        assert_eq!(metadata.duration(), Some(Duration::from_millis(100)));
    }

    #[tokio::test]
    async fn unsupported_fixture_is_a_redacted_decode_problem() {
        let response = response(b"not an audio stream".to_vec());
        let asset = response
            .read_lease()
            .materialize_media_asset()
            .await
            .unwrap();
        let problem = match decode_asset(&asset) {
            Ok(_) => panic!("invalid audio fixture must not decode"),
            Err(problem) => problem,
        };

        assert_eq!(problem.kind(), MediaProblemKind::Decode);
        assert!(!format!("{problem:?}").contains("not an audio stream"));
    }

    #[test]
    fn muting_preserves_the_selected_volume() {
        let mut gain = AudioGain::default();
        gain.set_volume(0.35);
        gain.set_muted(true);
        assert_eq!(gain.effective(), 0.0);

        gain.set_muted(false);
        assert_eq!(gain.effective(), 0.35);
    }

    fn response(bytes: Vec<u8>) -> Arc<ResponseData> {
        let len = bytes.len() as u64;
        Arc::new(ResponseData::new(
            ResponseHead::new(
                StatusCode::OK,
                Version::HTTP_11,
                Url::parse("https://example.test/private-audio").unwrap(),
                HeaderMap::new(),
            ),
            ResponseTiming {
                head_after: Duration::ZERO,
                completed_after: Duration::ZERO,
            },
            CompletedBody {
                body: StoredBody::Memory(Bytes::from(bytes)),
                body_decoding: BodyDecoding::Identity,
                sizes: ResponseSizes {
                    declared_encoded_bytes: Some(len),
                    received_encoded_bytes: len,
                    stored_body_bytes: len,
                },
            },
        ))
    }

    fn wav_fixture() -> Vec<u8> {
        let sample_rate = 8_000_u32;
        let sample_count = 800_u32;
        let data_bytes = sample_count * 2;
        let mut bytes = Vec::with_capacity((44 + data_bytes) as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_bytes.to_le_bytes());
        bytes.resize((44 + data_bytes) as usize, 0);
        bytes
    }
}
