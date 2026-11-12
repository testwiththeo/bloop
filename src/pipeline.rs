// GStreamer pipewiresrc pipeline: pulls BGRA frames out of the portal
// PipeWire remote and hands them to callers as timestamped surfaces.
// Remove this allow once ui.rs calls into this module.
#![allow(dead_code)]
//
// Element setup mirrors Kooha src/pipeline.rs: make_pipewiresrc sets
// fd (raw fd int), path (the portal stream node id as a string),
// do-timestamp so buffers carry running-time PTS, provide-clock=false,
// keepalive-time=1000ms and resend-last=true; make_videoflip follows
// with video-direction=auto because portals can deliver flipped
// buffers. Our chain stops at appsink instead of Kooha's encoder:
// pipewiresrc -> videoflip -> videoconvert -> video/x-raw,format=BGRA
// -> appsink. BGRA maps 1:1 onto cairo Format::ARgb32 byte order.
// gst docs: docs.rs/gstreamer (Sample::buffer/caps, BufferRef::pts/
// map_readable, ClockTime::mseconds) and docs.rs/gstreamer-app
// (AppSink::builder().caps, AppSinkCallbacks::new_sample, pull_sample).

use std::collections::VecDeque;
use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::{Arc, Mutex};

use cairo::{Format, ImageSurface};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;

#[derive(Debug)]
pub enum PipelineError {
    Gst(gst::glib::Error),
    Bool(gst::glib::BoolError),
    StateChange(gst::StateChangeError),
    MissingBuffer,
    MissingCaps,
    MapFailed,
    BadStride { expected: usize, got: usize },
    Cairo(cairo::Error),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gst(e) => write!(f, "gstreamer error: {e}"),
            Self::Bool(e) => write!(f, "gstreamer call failed: {e}"),
            Self::StateChange(e) => write!(f, "pipeline state change failed: {e}"),
            Self::MissingBuffer => write!(f, "sample carried no buffer"),
            Self::MissingCaps => write!(f, "sample caps missing width/height"),
            Self::MapFailed => write!(f, "buffer map_readable failed"),
            Self::BadStride { expected, got } => {
                write!(
                    f,
                    "buffer stride mismatch: expected {expected} bytes, got {got}"
                )
            }
            Self::Cairo(e) => write!(f, "cairo error: {e}"),
        }
    }
}

impl std::error::Error for PipelineError {}

impl From<gst::glib::Error> for PipelineError {
    fn from(e: gst::glib::Error) -> Self {
        Self::Gst(e)
    }
}

impl From<gst::glib::BoolError> for PipelineError {
    fn from(e: gst::glib::BoolError) -> Self {
        Self::Bool(e)
    }
}

impl From<gst::StateChangeError> for PipelineError {
    fn from(e: gst::StateChangeError) -> Self {
        Self::StateChange(e)
    }
}

impl From<cairo::Error> for PipelineError {
    fn from(e: cairo::Error) -> Self {
        Self::Cairo(e)
    }
}

/// A decoded BGRA buffer pulled from appsink, before surface upload.
pub struct RawSample {
    /// Buffer PTS in milliseconds (gst BufferRef::pts, nanosecond
    /// ClockTime converted via ClockTime::mseconds).
    pub timestamp_ms: u64,
    pub width: i32,
    pub height: i32,
    /// Tightly packed B,G,R,A bytes, stride = width * 4.
    pub data: Vec<u8>,
}

/// The appsink callback pushes samples here; callers pop timestamped
/// surfaces oldest-first. Popping and conversion is shared between the
/// live pipeline and the mock used in tests.
#[derive(Default)]
pub struct FrameQueue {
    inner: Mutex<VecDeque<RawSample>>,
}

impl FrameQueue {
    pub fn push(&self, sample: RawSample) {
        self.inner
            .lock()
            .expect("frame queue poisoned")
            .push_back(sample);
    }

    /// Pops the oldest frame as a (timestamp_ms, surface) pair.
    pub fn next_frame(&self) -> Option<Result<(u64, ImageSurface), PipelineError>> {
        self.inner
            .lock()
            .expect("frame queue poisoned")
            .pop_front()
            .map(sample_to_frame)
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("frame queue poisoned").len()
    }
}

fn sample_to_frame(sample: RawSample) -> Result<(u64, ImageSurface), PipelineError> {
    // ARgb32 stride is always width * 4; reject padded buffers instead
    // of guessing (a padded pipewire buffer would need gstreamer-video's
    // VideoInfo, kept out of scope).
    let stride = sample.width * 4;
    let expected = (sample.height * stride) as usize;
    if sample.data.len() != expected {
        return Err(PipelineError::BadStride {
            expected,
            got: sample.data.len(),
        });
    }
    let surface = ImageSurface::create_for_data(
        sample.data,
        Format::ARgb32,
        sample.width,
        sample.height,
        stride,
    )?;
    Ok((sample.timestamp_ms, surface))
}

fn sample_to_raw(sample: &gst::Sample) -> Result<RawSample, PipelineError> {
    let buffer = sample.buffer().ok_or(PipelineError::MissingBuffer)?;
    let caps = sample.caps().ok_or(PipelineError::MissingCaps)?;
    let structure = caps.structure(0).ok_or(PipelineError::MissingCaps)?;
    let width = structure
        .get::<i32>("width")
        .map_err(|_| PipelineError::MissingCaps)?;
    let height = structure
        .get::<i32>("height")
        .map_err(|_| PipelineError::MissingCaps)?;
    let timestamp_ms = buffer.pts().map(|pts| pts.mseconds()).unwrap_or(0);
    let map = buffer
        .map_readable()
        .map_err(|_| PipelineError::MapFailed)?;
    Ok(RawSample {
        timestamp_ms,
        width,
        height,
        data: map.as_slice().to_vec(),
    })
}

/// A running GStreamer pipeline; frames accumulate until popped.
/// stop() sets the pipeline to Null so PipeWire disconnects.
pub struct Pipeline {
    pipeline: gst::Pipeline,
    queue: Arc<FrameQueue>,
}

impl Pipeline {
    pub fn next_frame(&self) -> Option<Result<(u64, ImageSurface), PipelineError>> {
        self.queue.next_frame()
    }

    pub fn frames_buffered(&self) -> usize {
        self.queue.len()
    }

    pub fn stop(&self) -> Result<(), PipelineError> {
        self.pipeline.set_state(gst::State::Null)?;
        Ok(())
    }
}

/// Starts the screencast pipeline. The portal fd and stream node id come
/// from capture::PortalSession; the fd stays owned by the caller while
/// pipewiresrc connects through it (Kooha passes it the same way).
pub fn start_pipeline(pipewire_fd: &OwnedFd, node_id: u32) -> Result<Pipeline, PipelineError> {
    gst::init()?;

    let src = gst::ElementFactory::make("pipewiresrc")
        .property("fd", pipewire_fd.as_raw_fd())
        .property("path", node_id.to_string())
        .property("do-timestamp", true)
        .property("provide-clock", false)
        .property("keepalive-time", 1000)
        .property("resend-last", true)
        .build()?;
    let videoflip = gst::ElementFactory::make("videoflip")
        .property_from_str("video-direction", "auto")
        .build()?;
    let convert = gst::ElementFactory::make("videoconvert").build()?;
    let appsink = gst_app::AppSink::builder()
        .caps(
            &gst::Caps::builder("video/x-raw")
                .field("format", "BGRA")
                .build(),
        )
        .build();

    let queue = Arc::new(FrameQueue::default());
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample({
                let queue = queue.clone();
                move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    match sample_to_raw(&sample) {
                        Ok(raw) => {
                            queue.push(raw);
                            Ok(gst::FlowSuccess::Ok)
                        }
                        Err(_) => Err(gst::FlowError::Error),
                    }
                }
            })
            .build(),
    );

    let elements = [&src, &videoflip, &convert, appsink.upcast_ref()];
    let pipeline = gst::Pipeline::new();
    pipeline.add_many(elements)?;
    gst::Element::link_many(elements)?;
    pipeline.set_state(gst::State::Playing)?;
    Ok(Pipeline { pipeline, queue })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(timestamp_ms: u64) -> RawSample {
        RawSample {
            timestamp_ms,
            width: 4,
            height: 2,
            data: vec![0; 4 * 2 * 4],
        }
    }

    #[test]
    fn frames_come_out_timestamped_in_order() {
        // Mocked producer: what the appsink callback would have pushed.
        let queue = FrameQueue::default();
        queue.push(raw(0));
        queue.push(raw(33));
        queue.push(raw(66));

        let mut stamps = Vec::new();
        while let Some(frame) = queue.next_frame() {
            let (ts, surface) = frame.unwrap();
            stamps.push(ts);
            assert_eq!((surface.width(), surface.height()), (4, 2));
        }
        assert_eq!(stamps, [0, 33, 66]);
        assert!(queue.next_frame().is_none());
    }

    #[test]
    fn padded_buffer_is_rejected_not_misread() {
        let queue = FrameQueue::default();
        queue.push(RawSample {
            timestamp_ms: 5,
            width: 4,
            height: 2,
            data: vec![0; 4 * 2 * 4 + 8],
        });
        assert!(matches!(
            queue.next_frame(),
            Some(Err(PipelineError::BadStride { .. }))
        ));
    }
}
