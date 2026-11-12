// Remove this allow once main.rs or ui.rs calls into this module.
#![allow(dead_code)]

use crate::annotation::{composite_annotations, Annotation, DrawError};
use cairo::{BorrowError, Context, Format, ImageSurface};
use gifski::collector::{ImgVec, RGBA8};
use gifski::{progress, Repeat, Settings};
use std::fmt;

pub const SIZE_WARN_BYTES: u64 = 10 * 1024 * 1024;

// Heuristic bytes per source pixel for a screen recording GIF: one byte per
// pixel for the palette index, compressed well by LZW and deduplication.
const BYTES_PER_PIXEL_ESTIMATE: f64 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeEstimate {
    pub bytes: u64,
    pub over_limit: bool,
}

/// Returns the frames whose timestamps fall inside
/// trim_start_ms <= t <= trim_end_ms. Both bounds are inclusive.
/// `frames` must be sorted by timestamp, capture produces them in order.
pub fn trim_frames<T>(frames: &[(u64, T)], trim_start_ms: u64, trim_end_ms: u64) -> &[(u64, T)] {
    let start = frames.partition_point(|(t, _)| *t < trim_start_ms);
    let end = frames.partition_point(|(t, _)| *t <= trim_end_ms);
    if end <= start {
        return &[];
    }
    &frames[start..end]
}

/// Removes consecutive frames with identical payloads, keeping the first
/// frame of each run. Returns borrowed frames so no copying is needed,
/// timestamps are preserved untouched. Non-consecutive identical frames
/// are kept, they may be re-shown later.
pub fn dedup_frames<T: PartialEq>(frames: &[(u64, T)]) -> Vec<&(u64, T)> {
    let mut kept = Vec::with_capacity(frames.len());
    let mut prev: Option<&T> = None;
    for frame in frames {
        if prev != Some(&frame.1) {
            kept.push(frame);
            prev = Some(&frame.1);
        }
    }
    kept
}

/// Rough GIF size estimate in bytes. Real size depends on content entropy,
/// this is a heuristic, not a promise.
pub fn estimate_size(
    width: u32,
    height: u32,
    fps: u32,
    duration_ms: u64,
    scale_percent: u32,
) -> SizeEstimate {
    let scale = f64::from(scale_percent) / 100.0;
    let pixels = f64::from(width) * f64::from(height) * scale * scale;
    let frames = f64::from(fps) * duration_ms as f64 / 1000.0;
    let bytes = (pixels * frames * BYTES_PER_PIXEL_ESTIMATE) as u64;
    SizeEstimate {
        bytes,
        over_limit: bytes > SIZE_WARN_BYTES,
    }
}

#[derive(Debug)]
pub enum ExportError {
    NoFrames,
    MixedFrameSizes,
    Cairo(cairo::Error),
    Borrow(BorrowError),
    Draw(DrawError),
    Gifski(gifski::Error),
    FeedThreadPanicked,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrames => write!(f, "no frames inside the trim range"),
            Self::MixedFrameSizes => write!(f, "frames have different dimensions"),
            Self::Cairo(e) => write!(f, "cairo error: {e}"),
            Self::Borrow(e) => write!(f, "surface data borrow failed: {e}"),
            Self::Draw(e) => write!(f, "annotation draw failed: {e}"),
            Self::Gifski(e) => write!(f, "gifski error: {e}"),
            Self::FeedThreadPanicked => write!(f, "frame feed thread panicked"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<cairo::Error> for ExportError {
    fn from(e: cairo::Error) -> Self {
        Self::Cairo(e)
    }
}

impl From<BorrowError> for ExportError {
    fn from(e: BorrowError) -> Self {
        Self::Borrow(e)
    }
}

impl From<DrawError> for ExportError {
    fn from(e: DrawError) -> Self {
        Self::Draw(e)
    }
}

pub struct EncodedGif {
    pub bytes: Vec<u8>,
    pub estimate: SizeEstimate,
}

/// Trims, composites annotations, dedups consecutive identical frames, and
/// encodes to GIF bytes via gifski. Frame timestamps become gifski
/// presentation timestamps, so a kept frame after a dedup run is delayed
/// by the full timespan of the dropped frames. `scale_percent` resizes the
/// output through gifski's own resizer. gifski requires the Collector and
/// Writer on separate threads, this uses scoped threads internally.
pub fn encode_gif(
    frames: &[(u64, ImageSurface)],
    annotations: &[Annotation],
    fps: u32,
    scale_percent: u32,
    trim_start_ms: u64,
    trim_end_ms: u64,
) -> Result<EncodedGif, ExportError> {
    let kept = trim_frames(frames, trim_start_ms, trim_end_ms);
    let first = kept.first().ok_or(ExportError::NoFrames)?;
    let w = first.1.width() as usize;
    let h = first.1.height() as usize;
    if kept
        .iter()
        .any(|f| f.1.width() as usize != w || f.1.height() as usize != h)
    {
        return Err(ExportError::MixedFrameSizes);
    }

    let mut pix_frames: Vec<(u64, Vec<RGBA8>)> = Vec::with_capacity(kept.len());
    for (t, src) in kept {
        let mut work = copy_surface(src)?;
        composite_annotations(&mut work, annotations, *t)?;
        pix_frames.push((*t, surface_to_rgba(&work)?));
    }

    let deduped = dedup_frames(&pix_frames);
    let t0 = deduped[0].0;
    let duration_ms = deduped[deduped.len() - 1].0 - t0;

    let out_w = ((w * scale_percent as usize) / 100).max(1) as u32;
    let out_h = ((h * scale_percent as usize) / 100).max(1) as u32;
    let settings = Settings {
        width: Some(out_w),
        height: Some(out_h),
        quality: 100,
        repeat: Repeat::Infinite,
        ..Settings::default()
    };
    let (collector, writer) = gifski::new(settings).map_err(ExportError::Gifski)?;

    let mut bytes = Vec::new();
    std::thread::scope(|scope| -> Result<(), ExportError> {
        let feed = scope.spawn(move || -> Result<(), ExportError> {
            for (i, frame) in deduped.iter().enumerate() {
                collector
                    .add_frame_rgba(
                        i,
                        ImgVec::new(frame.1.clone(), w, h),
                        (frame.0 - t0) as f64 / 1000.0,
                    )
                    .map_err(ExportError::Gifski)?;
            }
            drop(collector);
            Ok(())
        });
        writer
            .write(&mut bytes, &mut progress::NoProgress {})
            .map_err(ExportError::Gifski)?;
        feed.join().map_err(|_| ExportError::FeedThreadPanicked)??;
        Ok(())
    })?;

    Ok(EncodedGif {
        bytes,
        estimate: estimate_size(w as u32, h as u32, fps, duration_ms, scale_percent),
    })
}

fn copy_surface(src: &ImageSurface) -> Result<ImageSurface, cairo::Error> {
    let dst = ImageSurface::create(Format::ARgb32, src.width(), src.height())?;
    {
        let ctx = Context::new(&dst)?;
        ctx.set_source_surface(src, 0.0, 0.0)?;
        ctx.paint()?;
    }
    Ok(dst)
}

/// ARgb32 surfaces store B,G,R,A per byte on little-endian. Alpha is forced
/// to 255 since GIF output is opaque over a composited background.
fn surface_to_rgba(surf: &ImageSurface) -> Result<Vec<RGBA8>, ExportError> {
    let w = surf.width() as usize;
    let h = surf.height() as usize;
    let stride = surf.stride() as usize;
    let mut out = Vec::with_capacity(w * h);
    surf.with_data(|data| {
        for y in 0..h {
            let row = y * stride;
            for x in 0..w {
                let o = row + x * 4;
                out.push(RGBA8 {
                    r: data[o + 2],
                    g: data[o + 1],
                    b: data[o],
                    a: 255,
                });
            }
        }
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames() -> Vec<(u64, u8)> {
        vec![(0, 0), (1000, 1), (2000, 2), (3000, 3), (4000, 4)]
    }

    #[test]
    fn trim_with_zero_start_keeps_from_beginning() {
        let f = frames();
        let kept = trim_frames(&f, 0, 2000);
        let stamps: Vec<u64> = kept.iter().map(|f| f.0).collect();
        assert_eq!(stamps, [0, 1000, 2000]);
    }

    #[test]
    fn trim_middle_cut_keeps_only_the_window() {
        let f = frames();
        let kept = trim_frames(&f, 1000, 3000);
        let stamps: Vec<u64> = kept.iter().map(|f| f.0).collect();
        assert_eq!(stamps, [1000, 2000, 3000]);
    }

    #[test]
    fn trim_edge_cases() {
        let f = frames();
        assert_eq!(trim_frames(&f, 0, 0).len(), 1);
        assert!(trim_frames(&f, 9000, 10000).is_empty());
        assert!(trim_frames(&f, 3000, 1000).is_empty());
        assert_eq!(trim_frames(&f, 0, u64::MAX).len(), 5);
        let empty: Vec<(u64, u8)> = vec![];
        assert!(trim_frames(&empty, 0, 1000).is_empty());
    }

    #[test]
    fn dedup_no_duplicates_keeps_all() {
        let f = frames();
        let kept = dedup_frames(&f);
        assert_eq!(kept.len(), 5);
        assert_eq!(kept[3].0, 3000);
    }

    #[test]
    fn dedup_all_identical_keeps_first() {
        let f = vec![(0, 7), (500, 7), (1000, 7), (1500, 7)];
        let kept = dedup_frames(&f);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].0, 0);
    }

    #[test]
    fn dedup_only_consecutive_runs_collapse() {
        let f = vec![(0, 1), (100, 2), (200, 2), (300, 1), (400, 1), (500, 3)];
        let kept = dedup_frames(&f);
        let stamps: Vec<u64> = kept.iter().map(|f| f.0).collect();
        // First 1, first 2, second 1, 3. Non-consecutive repeats survive.
        assert_eq!(stamps, [0, 100, 300, 500]);
    }

    #[test]
    fn dedup_alternating_keeps_all() {
        let f = vec![(0, 1), (100, 2), (200, 1), (300, 2)];
        let kept = dedup_frames(&f);
        assert_eq!(kept.len(), 4);
    }

    #[test]
    fn dedup_empty_input() {
        let empty: Vec<(u64, u8)> = vec![];
        assert!(dedup_frames(&empty).is_empty());
    }

    fn solid_frame(w: i32, h: i32, r: f64, g: f64, b: f64) -> ImageSurface {
        let s = ImageSurface::create(Format::ARgb32, w, h).unwrap();
        let ctx = Context::new(&s).unwrap();
        ctx.set_source_rgb(r, g, b);
        ctx.paint().unwrap();
        s
    }

    #[test]
    fn encode_three_frames_produces_gif_bytes() {
        let frames = vec![
            (0, solid_frame(32, 24, 1.0, 0.0, 0.0)),
            (100, solid_frame(32, 24, 0.0, 1.0, 0.0)),
            (200, solid_frame(32, 24, 0.0, 0.0, 1.0)),
        ];
        let out = encode_gif(&frames, &[], 10, 100, 0, 1000).unwrap();
        assert!(!out.bytes.is_empty());
        assert!(out.bytes.starts_with(b"GIF8"), "missing GIF header");
    }

    #[test]
    fn encode_empty_input_is_rejected() {
        let frames: Vec<(u64, ImageSurface)> = vec![];
        assert!(matches!(
            encode_gif(&frames, &[], 10, 100, 0, 1000),
            Err(ExportError::NoFrames)
        ));
    }

    #[test]
    fn encode_with_duplicate_frame_still_works() {
        let frames = vec![
            (0, solid_frame(32, 24, 1.0, 0.0, 0.0)),
            (100, solid_frame(32, 24, 1.0, 0.0, 0.0)),
            (200, solid_frame(32, 24, 0.0, 0.0, 1.0)),
        ];
        let out = encode_gif(&frames, &[], 10, 100, 0, 1000).unwrap();
        assert!(out.bytes.starts_with(b"GIF8"));
    }

    #[test]
    fn estimate_scales_with_fps() {
        let est12 = estimate_size(800, 600, 12, 5000, 100);
        let est5 = estimate_size(800, 600, 5, 5000, 100);
        assert!(est12.bytes > est5.bytes);
        // Linear in fps: 12/5 = 2.4, allow rounding slack.
        let ratio = est12.bytes as f64 / est5.bytes.max(1) as f64;
        assert!((2.0..=2.8).contains(&ratio), "ratio was {ratio}");
    }

    #[test]
    fn estimate_flags_large_files() {
        let small = estimate_size(800, 600, 12, 5000, 100);
        assert!(!small.over_limit);
        // The product promise: a default 5 second clip estimates under 5 MiB.
        assert!(small.bytes < 5 * 1024 * 1024, "got {} bytes", small.bytes);

        let huge = estimate_size(7680, 4320, 20, 60_000, 100);
        assert!(huge.over_limit);
    }

    #[test]
    fn estimate_scales_with_output_scale() {
        let full = estimate_size(800, 600, 12, 5000, 100);
        let quarter = estimate_size(800, 600, 12, 5000, 50);
        assert!(quarter.bytes < full.bytes);
        let ratio = full.bytes as f64 / quarter.bytes.max(1) as f64;
        assert!(
            (3.0..=5.0).contains(&ratio),
            "50% scale should quarter pixels, ratio was {ratio}"
        );
    }
}
