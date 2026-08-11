//! GStreamer-backed, audio-only media adapter.
//!
//! Construction and preroll run on the response pane's background preparation
//! task. After preparation, the GPUI owner only enqueues typed commands; a
//! bounded, driver-owned worker performs every GStreamer query, seek, and state
//! change. The worker transitively owns the private response asset and releases
//! it only after the pipeline has entered `Null`.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};
use gstreamer::{self as gst, message::MessageView, prelude::*};

use super::{
    DecoderPolicy, MediaCommand, MediaDriver, MediaDriverEvent, MediaDriverEvents, MediaMetadata,
    MediaPosition, MediaProblem, MediaProblemKind, ResponseAssetLease,
};

/// A prepared audio pipeline and its single-consumer event endpoint.
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

/// Per-preview GStreamer audio controller.
///
/// Each instance builds its own `uridecodebin`. `SoftwareOnly` is set on that
/// instance only; this adapter never changes registry ranks or process-wide
/// plugin environment variables.
pub(crate) struct AudioDriver {
    commands: Sender<MediaCommand>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl AudioDriver {
    /// Builds and confirms preroll of an audio-only pipeline in `Paused` state.
    /// The caller must invoke this from the pane's background preparation task.
    pub(crate) fn prepare(
        asset: ResponseAssetLease,
        decoder_policy: DecoderPolicy,
    ) -> Result<AudioPrepared, MediaProblem> {
        Self::prepare_with_sink(asset, decoder_policy, "autoaudiosink")
    }

    fn prepare_with_sink(
        asset: ResponseAssetLease,
        decoder_policy: DecoderPolicy,
        sink_factory: &str,
    ) -> Result<AudioPrepared, MediaProblem> {
        gst::init().map_err(|_| MediaProblem::new(MediaProblemKind::RuntimeUnavailable))?;

        let pipeline = gst::Pipeline::new();
        let decoder = make("uridecodebin", "audio decoder")?;
        decoder.set_property("uri", asset.uri().as_str());
        configure_decoder_policy(&decoder, decoder_policy)?;

        let convert = make("audioconvert", "audio converter")?;
        let resample = make("audioresample", "audio resampler")?;
        let volume = make("volume", "audio volume")?;
        let sink = make(sink_factory, "audio output")?;

        pipeline
            .add_many([&decoder, &convert, &resample, &volume, &sink])
            .map_err(|_| MediaProblem::plugin("audio pipeline"))?;
        gst::Element::link_many([&convert, &resample, &volume, &sink])
            .map_err(|_| MediaProblem::plugin("audio pipeline"))?;
        link_audio_pad_when_available(&decoder, &convert);

        let bus = pipeline
            .bus()
            .ok_or_else(|| MediaProblem::new(MediaProblemKind::Internal))?;

        // PAUSED performs preroll but never starts audible playback. Waiting
        // here is intentional: this method only runs on the preparation
        // worker, and the UI must not expose a playable state before decoder
        // and sink failures are known.
        if pipeline.set_state(gst::State::Paused).is_err() {
            return Err(MediaProblem::new(MediaProblemKind::Decode));
        }
        let (state_result, current, _) = pipeline.state(gst::ClockTime::from_seconds(5));
        if state_result.is_err() || current != gst::State::Paused {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(MediaProblem::new(MediaProblemKind::Decode));
        }

        let metadata = MediaMetadata::new(query_duration(&pipeline));
        let (command_sender, command_receiver) = async_channel::bounded(16);
        let (critical_sender, critical_receiver) = async_channel::bounded(4);
        let (telemetry_sender, telemetry_receiver) = async_channel::bounded(1);
        let stop = Arc::new(AtomicBool::new(false));
        let backend_stop = Arc::clone(&stop);
        let backend = AudioBackend {
            pipeline,
            bus,
            volume,
            _asset: asset,
        };
        let worker = std::thread::Builder::new()
            .name("http-client-audio-preview".into())
            .spawn(move || {
                run_audio_backend(
                    backend,
                    command_receiver,
                    backend_stop,
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
        // Never join on the GPUI thread. The bounded worker owns the pipeline
        // and asset, observes `stop`, enters Null, then exits. Dropping this
        // handle only releases the OS join capability; it does not abandon a
        // producer capable of routing back into GPUI.
        self.worker.take();
    }
}

struct AudioBackend {
    pipeline: gst::Pipeline,
    bus: gst::Bus,
    volume: gst::Element,
    _asset: ResponseAssetLease,
}

impl Drop for AudioBackend {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

fn run_audio_backend(
    backend: AudioBackend,
    commands: Receiver<MediaCommand>,
    stop: Arc<AtomicBool>,
    critical: Sender<MediaDriverEvent>,
    telemetry: Sender<MediaDriverEvent>,
) {
    while !stop.load(Ordering::Acquire) {
        loop {
            match commands.try_recv() {
                Ok(command) => {
                    if let Err(problem) = execute_audio_command(&backend, command, &telemetry) {
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
        while let Some(message) = backend.bus.timed_pop(gst::ClockTime::ZERO) {
            match message.view() {
                MessageView::Eos(..) => {
                    send_critical(&critical, MediaDriverEvent::Ended);
                }
                MessageView::Error(..) => {
                    send_critical(
                        &critical,
                        MediaDriverEvent::PlaybackFailed(MediaProblem::new(
                            MediaProblemKind::Decode,
                        )),
                    );
                    stop.store(true, Ordering::Release);
                }
                MessageView::AsyncDone(..) | MessageView::DurationChanged(..) => {
                    send_telemetry(
                        &telemetry,
                        MediaDriverEvent::Metadata(MediaMetadata::new(query_duration(
                            &backend.pipeline,
                        ))),
                    );
                }
                _ => {}
            }
        }
        if !stop.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn execute_audio_command(
    backend: &AudioBackend,
    command: MediaCommand,
    telemetry: &Sender<MediaDriverEvent>,
) -> Result<(), MediaProblem> {
    match command {
        MediaCommand::Play => backend
            .pipeline
            .set_state(gst::State::Playing)
            .map(|_| ())
            .map_err(|_| MediaProblem::new(MediaProblemKind::Control)),
        MediaCommand::Pause => backend
            .pipeline
            .set_state(gst::State::Paused)
            .map(|_| ())
            .map_err(|_| MediaProblem::new(MediaProblemKind::Control)),
        MediaCommand::Seek(position) => {
            backend
                .pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                    duration_to_clock_time(position),
                )
                .map_err(|_| MediaProblem::new(MediaProblemKind::Control))?;
            emit_position(&backend.pipeline, telemetry);
            Ok(())
        }
        MediaCommand::SetVolume(volume) => {
            backend.volume.set_property("volume", f64::from(volume));
            Ok(())
        }
        MediaCommand::SetMuted(muted) => {
            backend.volume.set_property("mute", muted);
            Ok(())
        }
        MediaCommand::PollPosition => {
            emit_position(&backend.pipeline, telemetry);
            Ok(())
        }
        MediaCommand::Stop => Ok(()),
    }
}

fn emit_position(pipeline: &gst::Pipeline, telemetry: &Sender<MediaDriverEvent>) {
    let position = pipeline
        .query_position::<gst::ClockTime>()
        .map(Duration::from)
        .unwrap_or(Duration::ZERO);
    send_telemetry(
        telemetry,
        MediaDriverEvent::Position(MediaPosition::new(position, query_duration(pipeline))),
    );
}

fn send_telemetry(sender: &Sender<MediaDriverEvent>, event: MediaDriverEvent) {
    let _ = sender.try_send(event);
}

fn send_critical(sender: &Sender<MediaDriverEvent>, event: MediaDriverEvent) {
    // This runs only on the driver worker. Backpressure is preferable to
    // losing EOS or a terminal decoder failure, and dropping the pane closes
    // the receiver so shutdown never waits for a vanished UI owner.
    let _ = sender.send_blocking(event);
}

fn make(factory: &str, family: &'static str) -> Result<gst::Element, MediaProblem> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| MediaProblem::plugin(family))
}

fn configure_decoder_policy(
    decoder: &gst::Element,
    decoder_policy: DecoderPolicy,
) -> Result<(), MediaProblem> {
    if decoder_policy == DecoderPolicy::SoftwareOnly {
        if decoder.find_property("force-sw-decoders").is_none() {
            return Err(MediaProblem::plugin("software decoder policy"));
        }
        decoder.set_property("force-sw-decoders", true);
    }
    Ok(())
}

fn link_audio_pad_when_available(decoder: &gst::Element, convert: &gst::Element) {
    let sink_pad = convert
        .static_pad("sink")
        .expect("audioconvert always has a static sink pad");
    decoder.connect_pad_added(move |_, source_pad| {
        if sink_pad.is_linked() || !pad_is_raw_audio(source_pad) {
            return;
        }
        let _ = source_pad.link(&sink_pad);
    });
}

fn pad_is_raw_audio(pad: &gst::Pad) -> bool {
    pad.current_caps()
        .or_else(|| Some(pad.query_caps(None)))
        .and_then(|caps| {
            caps.structure(0)
                .map(|structure| structure.name().as_str() == "audio/x-raw")
        })
        .unwrap_or(false)
}

fn query_duration(pipeline: &gst::Pipeline) -> Option<Duration> {
    pipeline
        .query_duration::<gst::ClockTime>()
        .map(Duration::from)
}

fn duration_to_clock_time(duration: Duration) -> gst::ClockTime {
    // `GST_CLOCK_TIME_NONE` is encoded as `u64::MAX`, which is not a valid
    // concrete seek destination.
    gst::ClockTime::from_nseconds(duration.as_nanos().min(u128::from(u64::MAX - 1)) as u64)
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

    #[test]
    fn duration_conversion_saturates_without_panicking() {
        assert_eq!(
            duration_to_clock_time(Duration::MAX).nseconds(),
            u64::MAX - 1,
        );
    }

    #[tokio::test]
    async fn wav_fixture_prerolls_paused_and_accepts_controls_without_an_audio_device() {
        let bytes = wav_fixture();
        let len = bytes.len() as u64;
        let response = Arc::new(ResponseData::new(
            ResponseHead::new(
                StatusCode::OK,
                Version::HTTP_11,
                Url::parse("https://example.test/private.wav").unwrap(),
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
        ));
        let asset = response
            .read_lease()
            .materialize_media_asset()
            .await
            .unwrap();
        let prepared =
            AudioDriver::prepare_with_sink(asset, DecoderPolicy::Auto, "fakesink").unwrap();
        let (mut driver, events, _) = prepared.into_parts();

        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, MediaDriverEvent::Metadata(_)));
        driver.command(MediaCommand::Play).unwrap();
        driver.command(MediaCommand::Pause).unwrap();
        driver.command(MediaCommand::SetVolume(0.5)).unwrap();
        driver.command(MediaCommand::SetMuted(true)).unwrap();
        driver.command(MediaCommand::PollPosition).unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if matches!(events.recv().await.unwrap(), MediaDriverEvent::Position(_)) {
                    break;
                }
            }
        })
        .await
        .unwrap();
        driver.command(MediaCommand::Stop).unwrap();
    }

    #[test]
    fn missing_element_is_a_redacted_plugin_problem() {
        gst::init().unwrap();
        let problem = make(
            "http-client-element-that-does-not-exist",
            "audio test plugin",
        )
        .unwrap_err();
        assert_eq!(problem.kind(), MediaProblemKind::PluginMissing);
        assert!(!format!("{problem:?}").contains("http-client-element"));
        assert!(format!("{problem:?}").contains("audio test plugin"));
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
