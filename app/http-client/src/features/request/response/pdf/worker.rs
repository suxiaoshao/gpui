use std::{
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use bytes::Bytes;
use gpui::RenderImage;

use super::super::PreviewToken;
use super::{
    MAX_PDF_IMAGE_BYTES, MAX_PDF_PAGE_COUNT, MAX_PDF_PAGE_DIMENSION, PdfProblem, PdfViewport,
};

pub(crate) struct PdfWorkerHandle {
    shared: Arc<WorkerShared>,
    events: Option<async_channel::Receiver<PdfWorkerEvent>>,
}

impl PdfWorkerHandle {
    /// Builds the mailbox and worker without parsing the document.  Parsing
    /// begins only after [`Self::load`] is sent by the runtime transition.
    pub(crate) fn new(bytes: Bytes) -> Result<Self, PdfProblem> {
        let shared = Arc::new(WorkerShared::new());
        let (event_sender, event_receiver) = async_channel::bounded(1);
        let worker_shared = Arc::clone(&shared);
        let worker_events = event_receiver.clone();
        thread::Builder::new()
            .name("http-client-pdf-preview".into())
            .spawn(move || worker_loop(bytes, worker_shared, event_sender, worker_events))
            .map_err(|_| PdfProblem::internal())?;
        Ok(Self {
            shared,
            events: Some(event_receiver),
        })
    }

    pub(crate) fn take_event_receiver(
        &mut self,
    ) -> Result<async_channel::Receiver<PdfWorkerEvent>, PdfProblem> {
        self.events.take().ok_or_else(PdfProblem::internal)
    }

    #[cfg(test)]
    pub(crate) fn stop_probe(&self) -> Arc<AtomicBool> {
        self.shared.stop_probe()
    }

    pub(super) fn load(
        &self,
        token: PreviewToken,
        page_generation: u64,
        viewport: PdfViewport,
    ) -> Result<(), PdfProblem> {
        self.shared.push(PdfWorkerCommand::Load {
            token,
            page_generation,
            viewport,
        })
    }

    pub(super) fn render(
        &self,
        token: PreviewToken,
        page_generation: u64,
        page: usize,
        viewport: PdfViewport,
    ) -> Result<(), PdfProblem> {
        self.shared.push(PdfWorkerCommand::Render {
            token,
            page_generation,
            page,
            viewport,
        })
    }
}

impl Drop for PdfWorkerHandle {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

pub(crate) enum PdfWorkerEvent {
    Loaded {
        token: PreviewToken,
        page_generation: u64,
        page_count: usize,
        page: usize,
        image: Arc<RenderImage>,
    },
    Rendered {
        token: PreviewToken,
        page_generation: u64,
        page: usize,
        image: Arc<RenderImage>,
    },
    Failed {
        token: PreviewToken,
        page_generation: u64,
        problem: PdfProblem,
    },
}

impl PdfWorkerEvent {
    pub(crate) fn token(&self) -> &PreviewToken {
        match self {
            Self::Loaded { token, .. }
            | Self::Rendered { token, .. }
            | Self::Failed { token, .. } => token,
        }
    }

    pub(super) fn into_message(self) -> super::PdfMessage {
        match self {
            Self::Loaded {
                token,
                page_generation,
                page_count,
                page,
                image,
            } => super::PdfMessage::Loaded {
                token,
                page_generation,
                page_count,
                page,
                image,
            },
            Self::Rendered {
                token,
                page_generation,
                page,
                image,
            } => super::PdfMessage::Rendered {
                token,
                page_generation,
                page,
                image,
            },
            Self::Failed {
                token,
                page_generation,
                problem,
            } => super::PdfMessage::Failed {
                token,
                page_generation,
                problem,
            },
        }
    }
}

enum PdfWorkerCommand {
    Load {
        token: PreviewToken,
        page_generation: u64,
        viewport: PdfViewport,
    },
    Render {
        token: PreviewToken,
        page_generation: u64,
        page: usize,
        viewport: PdfViewport,
    },
}

struct WorkerShared {
    stopped: AtomicBool,
    mailbox: Mutex<Option<PdfWorkerCommand>>,
    wake: Condvar,
    #[cfg(test)]
    stop_probe: Mutex<Option<Arc<AtomicBool>>>,
}

impl WorkerShared {
    fn new() -> Self {
        Self {
            stopped: AtomicBool::new(false),
            mailbox: Mutex::new(None),
            wake: Condvar::new(),
            #[cfg(test)]
            stop_probe: Mutex::new(None),
        }
    }

    #[cfg(test)]
    fn stop_probe(&self) -> Arc<AtomicBool> {
        let stopped = Arc::new(AtomicBool::new(false));
        *self.stop_probe.lock().unwrap() = Some(Arc::clone(&stopped));
        stopped
    }

    fn push(&self, command: PdfWorkerCommand) -> Result<(), PdfProblem> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(PdfProblem::internal());
        }
        let mut mailbox = self.mailbox.lock().map_err(|_| PdfProblem::internal())?;
        if self.stopped.load(Ordering::Acquire) {
            return Err(PdfProblem::internal());
        }
        // A capacity-one, latest-only mailbox: an unstarted page target is
        // intentionally replaced rather than queued behind obsolete input.
        *mailbox = Some(command);
        self.wake.notify_one();
        Ok(())
    }

    fn next(&self) -> Option<PdfWorkerCommand> {
        let mut mailbox = self.mailbox.lock().ok()?;
        loop {
            if self.stopped.load(Ordering::Acquire) {
                return None;
            }
            if let Some(command) = mailbox.take() {
                return Some(command);
            }
            mailbox = self.wake.wait(mailbox).ok()?;
        }
    }

    fn stop(&self) {
        #[cfg(test)]
        if let Ok(probe) = self.stop_probe.lock()
            && let Some(probe) = probe.as_ref()
        {
            probe.store(true, Ordering::Release);
        }
        self.stopped.store(true, Ordering::Release);
        self.wake.notify_all();
    }
}

fn worker_loop(
    bytes: Bytes,
    shared: Arc<WorkerShared>,
    events: async_channel::Sender<PdfWorkerEvent>,
    pending_events: async_channel::Receiver<PdfWorkerEvent>,
) {
    let Some(PdfWorkerCommand::Load {
        token,
        page_generation,
        viewport,
    }) = shared.next()
    else {
        return;
    };

    // The potentially 50 MiB Bytes -> Vec conversion stays on this dedicated
    // worker thread. The GPUI continuation only transfers the ref-counted
    // buffer into the worker.
    let pdf = match hayro::hayro_syntax::Pdf::new(bytes.to_vec()) {
        Ok(pdf) => pdf,
        Err(hayro::hayro_syntax::LoadPdfError::Decryption(_)) => {
            let _ = send_latest(
                &events,
                &pending_events,
                PdfWorkerEvent::Failed {
                    token,
                    page_generation,
                    problem: PdfProblem::encrypted(),
                },
            );
            return;
        }
        Err(hayro::hayro_syntax::LoadPdfError::Invalid) => {
            let _ = send_latest(
                &events,
                &pending_events,
                PdfWorkerEvent::Failed {
                    token,
                    page_generation,
                    problem: PdfProblem::parse(),
                },
            );
            return;
        }
    };
    let page_count = pdf.pages().len();
    if validate_page_count(page_count).is_err() {
        let _ = send_latest(
            &events,
            &pending_events,
            PdfWorkerEvent::Failed {
                token,
                page_generation,
                problem: PdfProblem::budget(),
            },
        );
        return;
    }

    let cache = hayro::RenderCache::new();
    let settings = hayro::hayro_interpret::InterpreterSettings::default();
    if !send_initial_page(
        &pdf,
        &cache,
        &settings,
        &events,
        &pending_events,
        &shared,
        token,
        page_generation,
        page_count,
        viewport,
    ) {
        return;
    }

    while let Some(command) = shared.next() {
        let PdfWorkerCommand::Render {
            token,
            page_generation,
            page,
            viewport,
        } = command
        else {
            continue;
        };
        if page >= page_count {
            let _ = send_latest(
                &events,
                &pending_events,
                PdfWorkerEvent::Failed {
                    token,
                    page_generation,
                    problem: PdfProblem::budget(),
                },
            );
            continue;
        }
        let result = render_page(&pdf, &cache, &settings, page, viewport);
        let event = match result {
            Ok(image) => PdfWorkerEvent::Rendered {
                token,
                page_generation,
                page,
                image,
            },
            Err(problem) => PdfWorkerEvent::Failed {
                token,
                page_generation,
                problem,
            },
        };
        if !send_latest(&events, &pending_events, event) {
            return;
        }
    }
}

pub(super) fn validate_page_count(page_count: usize) -> Result<(), PdfProblem> {
    if page_count == 0 || page_count > MAX_PDF_PAGE_COUNT {
        Err(PdfProblem::budget())
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn send_initial_page<'a>(
    pdf: &'a hayro::hayro_syntax::Pdf,
    cache: &hayro::RenderCache<'a>,
    settings: &hayro::hayro_interpret::InterpreterSettings,
    events: &async_channel::Sender<PdfWorkerEvent>,
    pending_events: &async_channel::Receiver<PdfWorkerEvent>,
    shared: &WorkerShared,
    token: PreviewToken,
    page_generation: u64,
    page_count: usize,
    viewport: PdfViewport,
) -> bool {
    let event = match render_page(pdf, cache, settings, 0, viewport) {
        Ok(image) => PdfWorkerEvent::Loaded {
            token,
            page_generation,
            page_count,
            page: 0,
            image,
        },
        Err(problem) => PdfWorkerEvent::Failed {
            token,
            page_generation,
            problem,
        },
    };
    !shared.stopped.load(Ordering::Acquire) && send_latest(events, pending_events, event)
}

fn send_latest(
    sender: &async_channel::Sender<PdfWorkerEvent>,
    pending: &async_channel::Receiver<PdfWorkerEvent>,
    mut event: PdfWorkerEvent,
) -> bool {
    loop {
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(async_channel::TrySendError::Closed(_)) => return false,
            Err(async_channel::TrySendError::Full(returned)) => {
                event = returned;
                // The worker is the only producer. Any queued event is older
                // than the one being installed, so evict it from this
                // capacity-one completion mailbox.
                let _ = pending.try_recv();
            }
        }
    }
}

fn render_page<'a>(
    pdf: &'a hayro::hayro_syntax::Pdf,
    cache: &hayro::RenderCache<'a>,
    settings: &hayro::hayro_interpret::InterpreterSettings,
    page: usize,
    viewport: PdfViewport,
) -> Result<Arc<RenderImage>, PdfProblem> {
    let page = pdf.pages().get(page).ok_or_else(PdfProblem::budget)?;
    let (source_width, source_height) = page.render_dimensions();
    let (width, height) = contain_dimensions(source_width, source_height, viewport)?;
    reserve_image_budget(width, height)?;

    let scale_x = width as f32 / source_width;
    let scale_y = height as f32 / source_height;
    let pixmap = hayro::render(
        page,
        cache,
        settings,
        &hayro::RenderSettings {
            x_scale: scale_x,
            y_scale: scale_y,
            width: Some(width as u16),
            height: Some(height as u16),
            bg_color: hayro::vello_cpu::color::palette::css::WHITE,
        },
    );
    let actual_width = u32::from(pixmap.width());
    let actual_height = u32::from(pixmap.height());
    if actual_width != width || actual_height != height {
        return Err(PdfProblem::render());
    }
    reserve_image_budget(actual_width, actual_height)?;

    // Hayro yields premultiplied RGBA.  GPUI's RenderImage stores
    // premultiplied BGRA, so swap only the red/blue bytes in place and then
    // transfer the one pixmap allocation into the single image frame.
    let mut pixels = pixmap.take();
    let bytes = bytemuck::cast_slice_mut(&mut pixels);
    rgba_to_bgra(bytes);
    let bytes = bytemuck::allocation::try_cast_vec(pixels).map_err(|_| PdfProblem::internal())?;
    let buffer = image::RgbaImage::from_raw(actual_width, actual_height, bytes)
        .ok_or_else(PdfProblem::internal)?;
    Ok(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

pub(super) fn contain_dimensions(
    source_width: f32,
    source_height: f32,
    viewport: PdfViewport,
) -> Result<(u32, u32), PdfProblem> {
    if !source_width.is_finite()
        || !source_height.is_finite()
        || source_width <= 0.0
        || source_height <= 0.0
        || viewport.width() == 0
        || viewport.height() == 0
    {
        return Err(PdfProblem::budget());
    }
    let max_width = viewport.width().min(MAX_PDF_PAGE_DIMENSION) as f32;
    let max_height = viewport.height().min(MAX_PDF_PAGE_DIMENSION) as f32;
    let scale = (max_width / source_width).min(max_height / source_height);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(PdfProblem::budget());
    }
    let width = (source_width * scale).floor().max(1.0);
    let height = (source_height * scale).floor().max(1.0);
    if !width.is_finite()
        || !height.is_finite()
        || width > MAX_PDF_PAGE_DIMENSION as f32
        || height > MAX_PDF_PAGE_DIMENSION as f32
    {
        return Err(PdfProblem::budget());
    }
    Ok((width as u32, height as u32))
}

pub(super) fn reserve_image_budget(width: u32, height: u32) -> Result<(), PdfProblem> {
    if width == 0
        || height == 0
        || width > MAX_PDF_PAGE_DIMENSION
        || height > MAX_PDF_PAGE_DIMENSION
    {
        return Err(PdfProblem::budget());
    }
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(PdfProblem::budget)?;
    if bytes > MAX_PDF_IMAGE_BYTES {
        Err(PdfProblem::budget())
    } else {
        Ok(())
    }
}

pub(super) fn rgba_to_bgra(bytes: &mut [u8]) {
    for pixel in bytes.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
}
