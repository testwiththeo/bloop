// TODO: tracker not yet created. Implement hit testing.
// Remove this allow once ui.rs or export.rs calls into this module.
#![allow(dead_code)]

use std::fmt;

use cairo::{Context, ImageSurface};
use serde::{Deserialize, Serialize};

const MAX_TEXT_CHARS: usize = 140;
const MIN_THICKNESS: u8 = 1;
const MAX_THICKNESS: u8 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnnotationError {
    InvalidTimeRange { start_ms: u64, end_ms: u64 },
    InvalidColor(String),
    EmptyText,
    TextTooLong(usize),
    InvalidOpacity(f64),
    InvalidThickness(u8),
}

impl fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeRange { start_ms, end_ms } => {
                write!(
                    f,
                    "end_ms {end_ms} must be greater than start_ms {start_ms}"
                )
            }
            Self::InvalidColor(color) => write!(f, "invalid HEX color: {color}"),
            Self::EmptyText => write!(f, "text must not be empty"),
            Self::TextTooLong(len) => {
                write!(f, "text is {len} characters, max is {MAX_TEXT_CHARS}")
            }
            Self::InvalidOpacity(opacity) => {
                write!(f, "opacity {opacity} is outside 0.0 to 1.0")
            }
            Self::InvalidThickness(thickness) => {
                write!(
                    f,
                    "thickness {thickness} is outside {MIN_THICKNESS} to {MAX_THICKNESS}"
                )
            }
        }
    }
}

impl std::error::Error for AnnotationError {}

fn check_time_range(start_ms: u64, end_ms: u64) -> Result<(), AnnotationError> {
    if start_ms >= end_ms {
        return Err(AnnotationError::InvalidTimeRange { start_ms, end_ms });
    }
    Ok(())
}

fn check_hex_color(color: &str) -> Result<(), AnnotationError> {
    let bytes = color.as_bytes();
    let valid =
        bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit);
    if valid {
        Ok(())
    } else {
        Err(AnnotationError::InvalidColor(color.to_string()))
    }
}

fn check_stroked_shape(
    thickness: u8,
    color: &str,
    start_ms: u64,
    end_ms: u64,
) -> Result<(), AnnotationError> {
    check_time_range(start_ms, end_ms)?;
    check_hex_color(color)?;
    if !(MIN_THICKNESS..=MAX_THICKNESS).contains(&thickness) {
        return Err(AnnotationError::InvalidThickness(thickness));
    }
    Ok(())
}

#[derive(Debug)]
pub enum DrawError {
    Cairo(cairo::Error),
    SurfaceBorrowed,
    Invalid(AnnotationError),
}

impl fmt::Display for DrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cairo(e) => write!(f, "cairo error: {e}"),
            Self::SurfaceBorrowed => write!(f, "frame pixels are mutably borrowed elsewhere"),
            Self::Invalid(e) => write!(f, "invalid annotation: {e}"),
        }
    }
}

impl std::error::Error for DrawError {}

impl From<cairo::Error> for DrawError {
    fn from(e: cairo::Error) -> Self {
        Self::Cairo(e)
    }
}

impl From<AnnotationError> for DrawError {
    fn from(e: AnnotationError) -> Self {
        Self::Invalid(e)
    }
}

fn parse_hex_color(color: &str) -> Option<(f64, f64, f64)> {
    check_hex_color(color).ok()?;
    let r = u8::from_str_radix(&color[1..3], 16).ok()?;
    let g = u8::from_str_radix(&color[3..5], 16).ok()?;
    let b = u8::from_str_radix(&color[5..7], 16).ok()?;
    Some((
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    ))
}

pub fn draw_arrow_on_frame(frame: &ImageSurface, arrow: &Arrow) -> Result<(), DrawError> {
    arrow.validate()?;
    let (r, g, b) =
        parse_hex_color(&arrow.color).ok_or(AnnotationError::InvalidColor(arrow.color.clone()))?;
    let ctx = Context::new(frame)?;
    ctx.set_source_rgb(r, g, b);
    ctx.set_line_width(f64::from(arrow.thickness));
    ctx.set_line_cap(cairo::LineCap::Round);
    for (x0, y0, x1, y1) in arrow_segments(arrow) {
        ctx.move_to(x0, y0);
        ctx.line_to(x1, y1);
    }
    ctx.stroke()?;
    Ok(())
}

fn arrow_segments(arrow: &Arrow) -> [(f64, f64, f64, f64); 3] {
    let x0 = f64::from(arrow.x);
    let y0 = f64::from(arrow.y);
    let tip_x = f64::from(arrow.tip_x);
    let tip_y = f64::from(arrow.tip_y);
    let dx = tip_x - x0;
    let dy = tip_y - y0;
    let len = dx.hypot(dy);
    if len == 0.0 {
        return [
            (x0, y0, tip_x, tip_y),
            (tip_x, tip_y, tip_x, tip_y),
            (tip_x, tip_y, tip_x, tip_y),
        ];
    }
    let head = (10.0 + 2.0 * f64::from(arrow.thickness)).min(len / 2.0);
    let ux = dx / len;
    let uy = dy / len;
    let wing = head * 0.6;
    let base_x = tip_x - ux * head;
    let base_y = tip_y - uy * head;
    [
        (x0, y0, tip_x, tip_y),
        (base_x - uy * wing, base_y + ux * wing, tip_x, tip_y),
        (base_x + uy * wing, base_y - ux * wing, tip_x, tip_y),
    ]
}

pub fn draw_box_on_frame(frame: &ImageSurface, box_ann: &BoxAnnotation) -> Result<(), DrawError> {
    box_ann.validate()?;
    let (r, g, b) = parse_hex_color(&box_ann.color)
        .ok_or(AnnotationError::InvalidColor(box_ann.color.clone()))?;
    let ctx = Context::new(frame)?;
    ctx.set_source_rgb(r, g, b);
    ctx.set_line_width(f64::from(box_ann.thickness));
    ctx.rectangle(
        f64::from(box_ann.x),
        f64::from(box_ann.y),
        f64::from(box_ann.width),
        f64::from(box_ann.height),
    );
    ctx.stroke()?;
    Ok(())
}

pub fn draw_highlight_on_frame(
    frame: &ImageSurface,
    highlight: &Highlight,
) -> Result<(), DrawError> {
    highlight.validate()?;
    let (r, g, b) = parse_hex_color(&highlight.color)
        .ok_or(AnnotationError::InvalidColor(highlight.color.clone()))?;
    let ctx = Context::new(frame)?;
    ctx.set_source_rgba(r, g, b, highlight.opacity);
    ctx.rectangle(
        f64::from(highlight.x),
        f64::from(highlight.y),
        f64::from(highlight.width),
        f64::from(highlight.height),
    );
    ctx.fill()?;
    Ok(())
}

pub fn draw_text_on_frame(frame: &ImageSurface, text: &Text) -> Result<(), DrawError> {
    text.validate()?;
    let (r, g, b) =
        parse_hex_color(&text.color).ok_or(AnnotationError::InvalidColor(text.color.clone()))?;
    let (br, bg, bb) = parse_hex_color(&text.background)
        .ok_or(AnnotationError::InvalidColor(text.background.clone()))?;
    let ctx = Context::new(frame)?;
    let l = measure_text(&ctx, text)?;

    rounded_rectangle(&ctx, l.x, l.y, l.w, l.h, l.h / 2.0);
    ctx.set_source_rgb(br, bg, bb);
    ctx.fill()?;

    ctx.set_source_rgb(r, g, b);
    ctx.move_to(l.x + l.pad - l.x_bearing, l.y + l.pad - l.y_bearing);
    ctx.show_text(&text.text)?;
    Ok(())
}

struct TextLayout {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    pad: f64,
    x_bearing: f64,
    y_bearing: f64,
}

fn measure_text(ctx: &Context, text: &Text) -> Result<TextLayout, cairo::Error> {
    ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    ctx.set_font_size(f64::from(text.font_size));
    let ext = ctx.text_extents(&text.text)?;
    let pad = f64::from(text.font_size) * 0.5;
    Ok(TextLayout {
        x: f64::from(text.x),
        y: f64::from(text.y),
        w: ext.width() + 2.0 * pad,
        h: ext.height() + 2.0 * pad,
        pad,
        x_bearing: ext.x_bearing(),
        y_bearing: ext.y_bearing(),
    })
}

fn rounded_rectangle(ctx: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let right = x + w;
    let bottom = y + h;
    ctx.new_sub_path();
    ctx.arc(right - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    ctx.arc(right - r, bottom - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    ctx.arc(
        x + r,
        bottom - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + r,
        y + r,
        r,
        std::f64::consts::PI,
        3.0 * std::f64::consts::FRAC_PI_2,
    );
    ctx.close_path();
}

pub fn draw_blur_on_frame(frame: &mut ImageSurface, blur: &Blur) -> Result<(), DrawError> {
    blur.validate()?;
    let surface_w = frame.width() as usize;
    let surface_h = frame.height() as usize;
    let stride = frame.stride() as usize;
    let x0 = (blur.x as usize).min(surface_w);
    let y0 = (blur.y as usize).min(surface_h);
    let x1 = (blur.x as usize + blur.width as usize).min(surface_w);
    let y1 = (blur.y as usize + blur.height as usize).min(surface_h);
    if x1 <= x0 || y1 <= y0 {
        return Ok(());
    }
    frame.flush();
    let mut data = frame.data().map_err(|_| DrawError::SurfaceBorrowed)?;
    box_blur_region(&mut data, stride, x0, y0, x1, y1, blur.radius as usize);
    Ok(())
}

fn box_blur_region(
    data: &mut [u8],
    stride: usize,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    radius: usize,
) {
    if radius == 0 {
        return;
    }
    let region_w = x1 - x0;
    let region_h = y1 - y0;
    let mut line = vec![0u8; region_w.max(region_h) * 4];

    // Horizontal pass.
    for y in y0..y1 {
        let base = y * stride + x0 * 4;
        line[..region_w * 4].copy_from_slice(&data[base..base + region_w * 4]);
        for i in 0..region_w {
            let lo = i.saturating_sub(radius);
            let hi = (i + radius).min(region_w - 1);
            let n = (hi - lo + 1) as u32;
            for c in 0..4 {
                let sum: u32 = (lo..=hi).map(|j| line[j * 4 + c] as u32).sum();
                data[base + i * 4 + c] = (sum / n) as u8;
            }
        }
    }

    // Vertical pass.
    for x in x0..x1 {
        for (i, y) in (y0..y1).enumerate() {
            let src = y * stride + x * 4;
            line[i * 4..i * 4 + 4].copy_from_slice(&data[src..src + 4]);
        }
        for (i, y) in (y0..y1).enumerate() {
            let lo = i.saturating_sub(radius);
            let hi = (i + radius).min(region_h - 1);
            let n = (hi - lo + 1) as u32;
            for c in 0..4 {
                let sum: u32 = (lo..=hi).map(|j| line[j * 4 + c] as u32).sum();
                data[y * stride + x * 4 + c] = (sum / n) as u8;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Arrow {
    // Tail point, in frame pixels.
    pub x: u32,
    pub y: u32,
    // Tip point where the arrowhead sits, in frame pixels. Any direction.
    pub tip_x: u32,
    pub tip_y: u32,
    pub color: String,
    pub thickness: u8,
    pub start_ms: u64,
    pub end_ms: u64,
}

// v1 files wrote arrows as an x/y/width/height bounding box whose tip was
// always the bottom-right corner. Accept that form on load and convert it
// to tip coordinates; new files always write tip_x/tip_y.
impl<'de> Deserialize<'de> for Arrow {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Repr {
            x: u32,
            y: u32,
            tip_x: Option<u32>,
            tip_y: Option<u32>,
            width: Option<u32>,
            height: Option<u32>,
            color: String,
            thickness: u8,
            start_ms: u64,
            end_ms: u64,
        }
        let r = Repr::deserialize(d)?;
        let (tip_x, tip_y) = match (r.tip_x, r.tip_y, r.width, r.height) {
            (Some(tx), Some(ty), _, _) => (tx, ty),
            (None, None, Some(w), Some(h)) => (r.x.saturating_add(w), r.y.saturating_add(h)),
            _ => return Err(serde::de::Error::missing_field("tip_x")),
        };
        Ok(Arrow {
            x: r.x,
            y: r.y,
            tip_x,
            tip_y,
            color: r.color,
            thickness: r.thickness,
            start_ms: r.start_ms,
            end_ms: r.end_ms,
        })
    }
}

impl Arrow {
    pub fn validate(&self) -> Result<(), AnnotationError> {
        check_stroked_shape(self.thickness, &self.color, self.start_ms, self.end_ms)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxAnnotation {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub color: String,
    pub thickness: u8,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl BoxAnnotation {
    pub fn validate(&self) -> Result<(), AnnotationError> {
        check_stroked_shape(self.thickness, &self.color, self.start_ms, self.end_ms)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub x: u32,
    pub y: u32,
    pub text: String,
    pub color: String,
    pub background: String,
    pub font_size: u32,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl Text {
    pub fn validate(&self) -> Result<(), AnnotationError> {
        check_time_range(self.start_ms, self.end_ms)?;
        check_hex_color(&self.color)?;
        check_hex_color(&self.background)?;
        let len = self.text.chars().count();
        if len == 0 {
            return Err(AnnotationError::EmptyText);
        }
        if len > MAX_TEXT_CHARS {
            return Err(AnnotationError::TextTooLong(len));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Highlight {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub color: String,
    pub opacity: f64,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl Highlight {
    pub fn validate(&self) -> Result<(), AnnotationError> {
        check_time_range(self.start_ms, self.end_ms)?;
        check_hex_color(&self.color)?;
        if !(0.0..=1.0).contains(&self.opacity) {
            return Err(AnnotationError::InvalidOpacity(self.opacity));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Blur {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub radius: u32,
    pub start_ms: u64,
    pub end_ms: u64,
}

impl Blur {
    pub fn validate(&self) -> Result<(), AnnotationError> {
        check_time_range(self.start_ms, self.end_ms)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Annotation {
    Arrow(Arrow),
    Box(BoxAnnotation),
    Text(Text),
    Highlight(Highlight),
    Blur(Blur),
}

impl Annotation {
    pub fn position(&self) -> (u32, u32) {
        match self {
            Self::Arrow(a) => (a.x, a.y),
            Self::Box(b) => (b.x, b.y),
            Self::Text(t) => (t.x, t.y),
            Self::Highlight(h) => (h.x, h.y),
            Self::Blur(b) => (b.x, b.y),
        }
    }

    pub fn time_range(&self) -> (u64, u64) {
        match self {
            Self::Arrow(a) => (a.start_ms, a.end_ms),
            Self::Box(b) => (b.start_ms, b.end_ms),
            Self::Text(t) => (t.start_ms, t.end_ms),
            Self::Highlight(h) => (h.start_ms, h.end_ms),
            Self::Blur(b) => (b.start_ms, b.end_ms),
        }
    }

    pub fn validate(&self) -> Result<(), AnnotationError> {
        match self {
            Self::Arrow(a) => a.validate(),
            Self::Box(b) => b.validate(),
            Self::Text(t) => t.validate(),
            Self::Highlight(h) => h.validate(),
            Self::Blur(b) => b.validate(),
        }
    }
}

const GRAB_TOLERANCE: f64 = 4.0;

pub fn hit_test(annotations: &[Annotation], x: f64, y: f64, time_ms: u64) -> Option<&Annotation> {
    annotations.iter().rev().find(|a| {
        let (start, end) = a.time_range();
        start <= time_ms && time_ms <= end && is_hit(a, x, y)
    })
}

fn is_hit(annotation: &Annotation, px: f64, py: f64) -> bool {
    match annotation {
        Annotation::Arrow(a) => arrow_hit(a, px, py),
        Annotation::Box(b) => box_hit(b, px, py),
        Annotation::Text(t) => text_hit(t, px, py),
        Annotation::Highlight(h) => point_in_rect(px, py, h.x, h.y, h.width, h.height),
        Annotation::Blur(b) => point_in_rect(px, py, b.x, b.y, b.width, b.height),
    }
}

fn point_in_rect(px: f64, py: f64, x: u32, y: u32, w: u32, h: u32) -> bool {
    px >= f64::from(x) && px <= f64::from(x + w) && py >= f64::from(y) && py <= f64::from(y + h)
}

fn arrow_hit(arrow: &Arrow, px: f64, py: f64) -> bool {
    let tolerance = f64::from(arrow.thickness) / 2.0 + GRAB_TOLERANCE;
    arrow_segments(arrow)
        .iter()
        .any(|&(x0, y0, x1, y1)| distance_to_segment(px, py, x0, y0, x1, y1) <= tolerance)
}

fn distance_to_segment(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;
    if len_sq == 0.0 {
        return (px - x0).hypot(py - y0);
    }
    let t = (((px - x0) * dx + (py - y0) * dy) / len_sq).clamp(0.0, 1.0);
    (px - (x0 + t * dx)).hypot(py - (y0 + t * dy))
}

fn box_hit(b: &BoxAnnotation, px: f64, py: f64) -> bool {
    let t = f64::from(b.thickness) / 2.0 + GRAB_TOLERANCE;
    let (x, y, w, h) = (
        f64::from(b.x),
        f64::from(b.y),
        f64::from(b.width),
        f64::from(b.height),
    );
    let in_outer = px >= x - t && px <= x + w + t && py >= y - t && py <= y + h + t;
    let in_inner = px >= x + t && px <= x + w - t && py >= y + t && py <= y + h - t;
    in_outer && !in_inner
}

fn text_hit(text: &Text, px: f64, py: f64) -> bool {
    let Ok(surface) = ImageSurface::create(cairo::Format::ARgb32, 1, 1) else {
        return false;
    };
    let Ok(ctx) = Context::new(&surface) else {
        return false;
    };
    let Ok(l) = measure_text(&ctx, text) else {
        return false;
    };
    px >= l.x && px <= l.x + l.w && py >= l.y && py <= l.y + l.h
}

pub fn composite_annotations(
    frame: &mut ImageSurface,
    annotations: &[Annotation],
    time_ms: u64,
) -> Result<usize, DrawError> {
    let mut drawn = 0;
    for ann in annotations {
        let (start, end) = ann.time_range();
        if !(start <= time_ms && time_ms <= end) {
            continue;
        }
        match ann {
            Annotation::Arrow(a) => draw_arrow_on_frame(frame, a)?,
            Annotation::Box(b) => draw_box_on_frame(frame, b)?,
            Annotation::Text(t) => draw_text_on_frame(frame, t)?,
            Annotation::Highlight(h) => draw_highlight_on_frame(frame, h)?,
            Annotation::Blur(b) => draw_blur_on_frame(frame, b)?,
        }
        drawn += 1;
    }
    Ok(drawn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arrow() -> Arrow {
        Arrow {
            x: 120,
            y: 80,
            tip_x: 320,
            tip_y: 120,
            color: "#FF0000".to_string(),
            thickness: 3,
            start_ms: 1000,
            end_ms: 3000,
        }
    }

    fn box_shape() -> BoxAnnotation {
        BoxAnnotation {
            x: 100,
            y: 100,
            width: 200,
            height: 100,
            color: "#FF0000".to_string(),
            thickness: 3,
            start_ms: 1000,
            end_ms: 3000,
        }
    }

    fn text() -> Text {
        Text {
            x: 100,
            y: 100,
            text: "Clicking here does nothing".to_string(),
            color: "#FFFFFF".to_string(),
            background: "#FF0000".to_string(),
            font_size: 14,
            start_ms: 1000,
            end_ms: 3000,
        }
    }

    fn highlight() -> Highlight {
        Highlight {
            x: 100,
            y: 100,
            width: 200,
            height: 100,
            color: "#FFE500".to_string(),
            opacity: 0.4,
            start_ms: 1000,
            end_ms: 3000,
        }
    }

    fn blur() -> Blur {
        Blur {
            x: 500,
            y: 10,
            width: 200,
            height: 30,
            radius: 12,
            start_ms: 0,
            end_ms: 4500,
        }
    }

    #[test]
    fn valid_annotations_pass_validation() {
        assert!(arrow().validate().is_ok());
        assert!(box_shape().validate().is_ok());
        assert!(text().validate().is_ok());
        assert!(highlight().validate().is_ok());
        assert!(blur().validate().is_ok());
    }

    #[test]
    fn invalid_time_ranges_are_rejected() {
        let mut a = arrow();
        a.end_ms = a.start_ms;
        assert_eq!(
            a.validate(),
            Err(AnnotationError::InvalidTimeRange {
                start_ms: 1000,
                end_ms: 1000
            })
        );

        let mut b = blur();
        b.start_ms = 5000;
        b.end_ms = 1000;
        assert!(matches!(
            b.validate(),
            Err(AnnotationError::InvalidTimeRange { .. })
        ));
    }

    #[test]
    fn hex_colors_are_validated() {
        for bad in ["FF0000", "#FF000", "#FF00000", "#GGGGGG", "", "#FF 000"] {
            let mut a = arrow();
            a.color = bad.to_string();
            assert_eq!(
                a.validate(),
                Err(AnnotationError::InvalidColor(bad.to_string())),
                "color {bad:?} should be rejected"
            );
        }
        for good in ["#FF0000", "#ff00a0", "#a1B2c3"] {
            let mut a = arrow();
            a.color = good.to_string();
            assert!(a.validate().is_ok(), "color {good:?} should be accepted");
        }

        let mut t = text();
        t.background = "red".to_string();
        assert_eq!(
            t.validate(),
            Err(AnnotationError::InvalidColor("red".to_string()))
        );
    }

    #[test]
    fn text_length_is_validated() {
        let mut t = text();
        t.text = String::new();
        assert_eq!(t.validate(), Err(AnnotationError::EmptyText));

        t.text = "a".repeat(141);
        assert_eq!(t.validate(), Err(AnnotationError::TextTooLong(141)));
        t.text = "a".repeat(140);
        assert!(t.validate().is_ok());

        // The limit is characters, not bytes.
        t.text = "é".repeat(140);
        assert!(t.validate().is_ok());
        t.text = "é".repeat(141);
        assert_eq!(t.validate(), Err(AnnotationError::TextTooLong(141)));
    }

    #[test]
    fn opacity_outside_range_is_rejected() {
        for bad in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            let mut h = highlight();
            h.opacity = bad;
            assert!(
                matches!(h.validate(), Err(AnnotationError::InvalidOpacity(_))),
                "opacity {bad} should be rejected"
            );
        }
        for good in [0.0, 0.4, 1.0] {
            let mut h = highlight();
            h.opacity = good;
            assert!(h.validate().is_ok(), "opacity {good} should be accepted");
        }
    }

    #[test]
    fn thickness_outside_range_is_rejected() {
        for bad in [0, 11] {
            let mut b = box_shape();
            b.thickness = bad;
            assert_eq!(
                b.validate(),
                Err(AnnotationError::InvalidThickness(bad)),
                "thickness {bad} should be rejected"
            );
        }
        for good in [1, 3, 10] {
            let mut b = box_shape();
            b.thickness = good;
            assert!(b.validate().is_ok(), "thickness {good} should be accepted");
        }
    }

    fn blank_surface() -> ImageSurface {
        ImageSurface::create(cairo::Format::ARgb32, 800, 600).expect("surface creation works")
    }

    fn alpha_at(surface: &mut ImageSurface, x: usize, y: usize) -> u8 {
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        data[y * stride + x * 4 + 3]
    }

    fn count_set_alpha(surface: &mut ImageSurface) -> usize {
        surface.flush();
        let mut count = 0;
        for y in 0..surface.height() as usize {
            for x in 0..surface.width() as usize {
                if alpha_at(surface, x, y) != 0 {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn arrow_drawing_changes_pixels() {
        let mut surface = blank_surface();
        assert_eq!(count_set_alpha(&mut surface), 0);
        draw_arrow_on_frame(&surface, &arrow()).expect("arrow draws");
        assert!(
            count_set_alpha(&mut surface) > 100,
            "arrow left too few pixels"
        );
        // The shaft runs from (120, 80) to (320, 120), its midpoint is on the line.
        assert_ne!(
            alpha_at(&mut surface, 220, 100),
            0,
            "shaft midpoint is empty"
        );
    }

    #[test]
    fn arrow_draws_in_all_directions() {
        // Tail fixed at (200,100); tip in each quadrant. The midpoint
        // values sit on each shaft line.
        for (tip_x, tip_y, mid_x, mid_y) in [
            (300, 150, 250, 125), // down-right
            (100, 150, 150, 125), // down-left
            (300, 50, 250, 75),   // up-right
            (100, 50, 150, 75),   // up-left
        ] {
            let mut surface = blank_surface();
            let mut a = arrow();
            a.x = 200;
            a.y = 100;
            a.tip_x = tip_x;
            a.tip_y = tip_y;
            draw_arrow_on_frame(&surface, &a).expect("arrow draws");
            assert_ne!(
                alpha_at(&mut surface, mid_x, mid_y),
                0,
                "shaft midpoint empty for tip ({tip_x}, {tip_y})"
            );
        }
    }

    #[test]
    fn zero_length_arrow_does_not_panic() {
        let surface = blank_surface();
        let mut a = arrow();
        a.tip_x = a.x;
        a.tip_y = a.y;
        draw_arrow_on_frame(&surface, &a).expect("zero-length arrow draws");
    }

    #[test]
    fn box_drawing_strokes_outline_only() {
        let mut surface = blank_surface();
        let b = box_shape();
        draw_box_on_frame(&surface, &b).expect("box draws");
        assert!(count_set_alpha(&mut surface) > 0, "box left no pixels");
        // The outline is drawn but the interior stays transparent.
        assert_eq!(
            alpha_at(
                &mut surface,
                (b.x + b.width / 2) as usize,
                (b.y + b.height / 2) as usize
            ),
            0,
            "box interior should be untouched"
        );
    }

    #[test]
    fn highlight_drawing_fills_region_translucently() {
        let mut surface = blank_surface();
        let h = highlight();
        draw_highlight_on_frame(&surface, &h).expect("highlight draws");
        let inside = alpha_at(
            &mut surface,
            (h.x + h.width / 2) as usize,
            (h.y + h.height / 2) as usize,
        );
        assert!(inside > 0, "highlight left region untouched");
        assert!(
            inside < 255,
            "highlight should be translucent, got {inside}"
        );
        assert_eq!(
            alpha_at(&mut surface, 0, 0),
            0,
            "highlight bled outside its region"
        );
    }

    #[test]
    fn drawing_invalid_annotation_is_rejected() {
        let surface = blank_surface();
        let mut a = arrow();
        a.thickness = 0;
        assert!(matches!(
            draw_arrow_on_frame(&surface, &a),
            Err(DrawError::Invalid(_))
        ));

        let mut surface = blank_surface();
        let mut b = blur();
        b.end_ms = b.start_ms;
        assert!(matches!(
            draw_blur_on_frame(&mut surface, &b),
            Err(DrawError::Invalid(_))
        ));
    }

    fn channel_at(surface: &mut ImageSurface, x: usize, y: usize, c: usize) -> u8 {
        let stride = surface.stride() as usize;
        let data = surface.data().expect("surface data");
        data[y * stride + x * 4 + c]
    }

    #[test]
    fn text_drawing_changes_pixels() {
        let mut surface = blank_surface();
        let t = text();
        draw_text_on_frame(&surface, &t).expect("text draws");
        assert!(count_set_alpha(&mut surface) > 0, "text left no pixels");

        // White glyphs on a pure red pill: some pixel inside must have green > 0.
        // ARgb32 is B,G,R,A per pixel.
        let mut found_glyph_pixel = false;
        for y in t.y..t.y + 40 {
            for x in t.x..t.x + 200 {
                if alpha_at(&mut surface, x as usize, y as usize) != 0
                    && channel_at(&mut surface, x as usize, y as usize, 1) > 200
                {
                    found_glyph_pixel = true;
                }
            }
        }
        assert!(found_glyph_pixel, "only the pill was drawn, no glyphs");
    }

    #[test]
    fn blur_makes_pattern_unrecoverable() {
        let mut surface = blank_surface();
        let b = blur();

        // Draw a maximum frequency pattern: 1px alternating black/white columns.
        {
            let ctx = Context::new(&surface).expect("context");
            for i in 0..b.width {
                let v = f64::from(i % 2);
                ctx.set_source_rgb(v, v, v);
                ctx.rectangle(f64::from(b.x + i), f64::from(b.y), 1.0, f64::from(b.height));
                ctx.fill().expect("fill");
            }
        }

        // Sanity check: the pattern exists before blurring.
        let before = channel_at(&mut surface, b.x as usize + 1, b.y as usize + 1, 0);
        assert!(
            before == 0 || before == 255,
            "pattern missing, got {before}"
        );

        draw_blur_on_frame(&mut surface, &b).expect("blur draws");

        // After blurring, no pixel in the region may keep a pure 0 or 255 value:
        // every channel must be a blend, so the original pattern is destroyed.
        let mut checked = 0;
        for y in b.y + 1..b.y + b.height - 1 {
            for x in b.x + 1..b.x + b.width - 1 {
                for c in 0..3 {
                    let v = channel_at(&mut surface, x as usize, y as usize, c);
                    assert!(
                        (10..=245).contains(&v),
                        "pixel ({x}, {y}) channel {c} kept value {v}, pattern recoverable"
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "test did not check any pixels");
    }

    #[test]
    fn hit_test_inside_each_type() {
        // Arrow shaft runs (120,80) to (320,120). At x=200 the line is at y=96.
        let arrow_ann = Annotation::Arrow(arrow());
        assert!(matches!(
            hit_test(&[arrow_ann], 200.0, 96.0, 1500),
            Some(Annotation::Arrow(_))
        ));

        let box_ann = Annotation::Box(box_shape());
        assert!(matches!(
            hit_test(&[box_ann], 200.0, 100.0, 1500),
            Some(Annotation::Box(_))
        ));

        let text_ann = Annotation::Text(text());
        assert!(matches!(
            hit_test(&[text_ann], 105.0, 105.0, 1500),
            Some(Annotation::Text(_))
        ));

        let hl_ann = Annotation::Highlight(highlight());
        assert!(matches!(
            hit_test(&[hl_ann], 200.0, 150.0, 1500),
            Some(Annotation::Highlight(_))
        ));

        let blur_ann = Annotation::Blur(blur());
        assert!(matches!(
            hit_test(&[blur_ann], 600.0, 25.0, 1500),
            Some(Annotation::Blur(_))
        ));
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let hl_ann = Annotation::Highlight(highlight());
        assert!(hit_test(&[hl_ann], 500.0, 500.0, 1500).is_none());

        // A box is only its outline. The interior is not a hit.
        let box_ann = Annotation::Box(box_shape());
        assert!(hit_test(&[box_ann], 200.0, 150.0, 1500).is_none());
    }

    #[test]
    fn hit_test_respects_time_range() {
        let hl_ann = Annotation::Highlight(highlight()); // active 1000 to 3000
        assert!(hit_test(&[hl_ann], 200.0, 150.0, 999).is_none());
        let hl_ann = Annotation::Highlight(highlight());
        assert!(hit_test(&[hl_ann], 200.0, 150.0, 3001).is_none());
        let hl_ann = Annotation::Highlight(highlight());
        // Boundaries are inclusive per ARCHITECTURE.md.
        assert!(hit_test(&[hl_ann], 200.0, 150.0, 1000).is_some());
        let hl_ann = Annotation::Highlight(highlight());
        assert!(hit_test(&[hl_ann], 200.0, 150.0, 3000).is_some());
    }

    #[test]
    fn hit_test_overlapping_returns_topmost() {
        let mut top = highlight();
        top.color = "#00FF00".to_string();
        let anns = [
            Annotation::Highlight(highlight()),
            Annotation::Highlight(top),
        ];
        let hit = hit_test(&anns, 200.0, 150.0, 1500).expect("overlap should hit");
        assert!(
            matches!(hit, Annotation::Highlight(h) if h.color == "#00FF00"),
            "expected the last annotation in the list to win"
        );
    }

    #[test]
    fn composite_draws_zero_annotations() {
        let mut surface = blank_surface();
        let drawn = composite_annotations(&mut surface, &[], 1500).expect("composite");
        assert_eq!(drawn, 0);
        assert_eq!(count_set_alpha(&mut surface), 0);
    }

    #[test]
    fn composite_draws_only_active_annotations() {
        let mut surface = blank_surface();
        let mut inactive = blur();
        inactive.start_ms = 4000;
        inactive.end_ms = 5000;
        let anns = [
            Annotation::Highlight(highlight()),
            Annotation::Blur(inactive),
        ];

        let drawn = composite_annotations(&mut surface, &anns, 1500).expect("composite");
        assert_eq!(drawn, 1, "only the highlight is active at 1500");
        assert_ne!(alpha_at(&mut surface, 200, 150), 0, "highlight not drawn");
        assert_eq!(alpha_at(&mut surface, 600, 25), 0, "inactive blur drawn");
    }

    #[test]
    fn composite_draws_topmost_last() {
        let mut surface = blank_surface();
        let mut bottom = highlight();
        bottom.color = "#FF0000".to_string();
        bottom.opacity = 1.0;
        let mut top = highlight();
        top.color = "#00FF00".to_string();
        top.opacity = 1.0;
        let anns = [Annotation::Highlight(bottom), Annotation::Highlight(top)];

        let drawn = composite_annotations(&mut surface, &anns, 1500).expect("composite");
        assert_eq!(drawn, 2);
        // ARgb32 pixel order is B,G,R,A. Opaque green over opaque red wins.
        assert_eq!(channel_at(&mut surface, 200, 150, 1), 255, "green lost");
        assert_eq!(channel_at(&mut surface, 200, 150, 2), 0, "red bled through");
    }
}
