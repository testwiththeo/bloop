// Main window: LibAdwaita ApplicationWindow with preview Picture,
// annotation canvas overlay, and Record/Stop/Clear controls.
// Rectangle is the only annotation tool so far; no export dialog yet.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use cairo::ImageSurface;
use gtk4::gio::prelude::FileExt;

use crate::annotation::{Annotation, BoxAnnotation};
use crate::capture::{self, AshpdPortal, Recording};
use crate::export;
use crate::pipeline::Pipeline;

const WINDOW_TITLE: &str = "Bloop";
const BOX_COLOR: &str = "#FF0000";
const BOX_THICKNESS: f64 = 3.0;
const MAX_FRAMES: usize = 600;

type ActiveRecording = Recording<AshpdPortal, Pipeline>;

/// Builds the main window: header bar, preview placeholder, and a
/// Record/Stop control row. The caller presents the window.
pub fn build_main_window(app: &adw::Application) -> adw::ApplicationWindow {
    let record_button = gtk4::Button::with_label("Record");
    let stop_button = gtk4::Button::with_label("Stop");
    stop_button.set_sensitive(false);

    let preview = gtk4::Picture::new();
    preview.set_hexpand(true);
    preview.set_vexpand(true);
    // Picture's documented default content-fit is Contain — uniform
    // scale, centered, letterboxed — which is what the coordinate math
    // in picture_content_rect assumes (gtk4 GtkPicture docs). If a
    // later task sets content-fit, that math must be revisited.
    // DrawingArea floats over the Picture; gestures land on the
    // overlay child, the Picture shows through where nothing is drawn.
    let canvas = gtk4::DrawingArea::new();
    canvas.set_hexpand(true);
    canvas.set_vexpand(true);
    let preview_stack = gtk4::Overlay::new();
    preview_stack.set_child(Some(&preview));
    preview_stack.add_overlay(&canvas);
    let preview_frame = gtk4::Frame::new(Some("Preview"));
    preview_frame.set_child(Some(&preview_stack));
    preview_frame.set_vexpand(true);

    let controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    controls.set_halign(gtk4::Align::Center);
    controls.append(&record_button);
    controls.append(&stop_button);
    let clear_button = gtk4::Button::with_label("Clear");
    controls.append(&clear_button);
    let export_button = gtk4::Button::with_label("Export");
    // Enabled only after Stop with a non-empty frame buffer.
    export_button.set_sensitive(false);
    controls.append(&export_button);

    // Export settings; encode_gif takes fps + scale_percent.
    let fps_spin = gtk4::SpinButton::with_range(5.0, 30.0, 1.0);
    fps_spin.set_value(12.0);
    let scale_spin = gtk4::SpinButton::with_range(25.0, 100.0, 5.0);
    scale_spin.set_value(100.0);
    controls.append(&gtk4::Label::new(Some("FPS")));
    controls.append(&fps_spin);
    controls.append(&gtk4::Label::new(Some("Scale %")));
    controls.append(&scale_spin);

    // Trim in ms; 0 end means "to the last frame" — replaced with the
    // real last timestamp on Stop.
    let trim_start = gtk4::SpinButton::with_range(0.0, 100_000.0, 100.0);
    let trim_end = gtk4::SpinButton::with_range(0.0, 100_000.0, 100.0);
    controls.append(&gtk4::Label::new(Some("Start ms")));
    controls.append(&trim_start);
    controls.append(&gtk4::Label::new(Some("End ms")));
    controls.append(&trim_end);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&preview_frame);
    content.append(&controls);

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&adw::HeaderBar::new());
    toolbar_view.set_content(Some(&content));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title(WINDOW_TITLE)
        .default_width(800)
        .default_height(600)
        .build();
    window.set_content(Some(&toolbar_view));

    let recording: Rc<RefCell<Option<ActiveRecording>>> = Rc::new(RefCell::new(None));
    let timer_id: Rc<RefCell<Option<gtk4::glib::SourceId>>> = Rc::new(RefCell::new(None));
    let annotations: Rc<RefCell<Vec<Annotation>>> = Rc::new(RefCell::new(Vec::new()));
    let frames: Rc<RefCell<Vec<(u64, ImageSurface)>>> = Rc::new(RefCell::new(Vec::new()));
    let frame_size: Rc<Cell<(f64, f64)>> = Rc::new(Cell::new((0.0, 0.0)));
    let drag_start: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
    let drag_now: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));

    canvas.set_draw_func({
        let annotations = annotations.clone();
        let frame_size = frame_size.clone();
        let drag_start = drag_start.clone();
        let drag_now = drag_now.clone();
        move |_, cr, w, h| {
            let widget = (f64::from(w), f64::from(h));
            let frame = effective_frame_size(frame_size.get(), widget);
            let content = picture_content_rect(widget, frame);
            for ann in annotations.borrow().iter() {
                // Only the rectangle tool exists; other variants cannot
                // appear in the vec yet.
                if let Annotation::Box(b) = ann {
                    let (x1, y1) = frame_to_widget(f64::from(b.x), f64::from(b.y), content, frame);
                    let (x2, y2) = frame_to_widget(
                        f64::from(b.x + b.width),
                        f64::from(b.y + b.height),
                        content,
                        frame,
                    );
                    stroke_box(
                        cr,
                        &b.color,
                        f64::from(b.thickness),
                        1.0,
                        (x1, y1, x2 - x1, y2 - y1),
                    );
                }
            }
            if let (Some((sx, sy)), Some((ex, ey))) = (drag_start.get(), drag_now.get()) {
                stroke_box(
                    cr,
                    BOX_COLOR,
                    BOX_THICKNESS,
                    0.7,
                    (sx.min(ex), sy.min(ey), (ex - sx).abs(), (ey - sy).abs()),
                );
            }
        }
    });

    let drag = gtk4::GestureDrag::new();
    drag.connect_drag_begin({
        let drag_start = drag_start.clone();
        let drag_now = drag_now.clone();
        let canvas = canvas.clone();
        move |_, x, y| {
            drag_start.set(Some((x, y)));
            drag_now.set(Some((x, y)));
            canvas.queue_draw();
        }
    });
    drag.connect_drag_update({
        let drag_start = drag_start.clone();
        let drag_now = drag_now.clone();
        let canvas = canvas.clone();
        move |_, ox, oy| {
            if let Some((sx, sy)) = drag_start.get() {
                drag_now.set(Some((sx + ox, sy + oy)));
                canvas.queue_draw();
            }
        }
    });
    drag.connect_drag_end({
        let annotations = annotations.clone();
        let frame_size = frame_size.clone();
        let drag_start = drag_start.clone();
        let drag_now = drag_now.clone();
        let canvas = canvas.clone();
        move |_, ox, oy| {
            if let Some((sx, sy)) = drag_start.get() {
                let (ex, ey) = (sx + ox, sy + oy);
                let widget = (f64::from(canvas.width()), f64::from(canvas.height()));
                let frame = effective_frame_size(frame_size.get(), widget);
                let content = picture_content_rect(widget, frame);
                // Clamped inside widget_to_frame, so off-canvas drags
                // land on the frame edge, not a saturated u32.
                let (x1, y1) = widget_to_frame(sx, sy, content, frame);
                let (x2, y2) = widget_to_frame(ex, ey, content, frame);
                let (x, y) = (x1.min(x2), y1.min(y2));
                let (w, h) = (x1.abs_diff(x2), y1.abs_diff(y2));
                if w >= 1 && h >= 1 {
                    // start_ms..end_ms spans everything so the box is
                    // visible for the whole recording.
                    annotations
                        .borrow_mut()
                        .push(Annotation::Box(BoxAnnotation {
                            x,
                            y,
                            width: w,
                            height: h,
                            color: BOX_COLOR.to_string(),
                            thickness: BOX_THICKNESS as u8,
                            start_ms: 0,
                            end_ms: u64::MAX,
                        }));
                }
            }
            drag_start.set(None);
            drag_now.set(None);
            canvas.queue_draw();
        }
    });
    canvas.add_controller(drag);

    clear_button.connect_clicked({
        let annotations = annotations.clone();
        let canvas = canvas.clone();
        move |_| {
            annotations.borrow_mut().clear();
            canvas.queue_draw();
        }
    });

    record_button.connect_clicked({
        let recording = recording.clone();
        let timer_id = timer_id.clone();
        let frame_size = frame_size.clone();
        let frames = frames.clone();
        let annotations = annotations.clone();
        let canvas = canvas.clone();
        let preview = preview.clone();
        let record_button = record_button.clone();
        let stop_button = stop_button.clone();
        let export_button = export_button.clone();
        move |_| {
            record_button.set_sensitive(false);
            export_button.set_sensitive(false);
            // A new recording starts empty: previous frames and
            // annotations are dropped.
            frames.borrow_mut().clear();
            annotations.borrow_mut().clear();
            canvas.queue_draw();
            let recording = recording.clone();
            let timer_id = timer_id.clone();
            let frame_size = frame_size.clone();
            let frames = frames.clone();
            let preview = preview.clone();
            let record_button = record_button.clone();
            let stop_button = stop_button.clone();
            gtk4::glib::MainContext::default().spawn_local(async move {
                match capture::start_recording().await {
                    Ok(active) => {
                        *recording.borrow_mut() = Some(active);
                        stop_button.set_sensitive(true);
                        let source_id = gtk4::glib::timeout_add_local(Duration::from_millis(33), {
                            let recording = recording.clone();
                            let frame_size = frame_size.clone();
                            let frames = frames.clone();
                            let preview = preview.clone();
                            move || {
                                // None or Err keeps the last frame
                                // and continues until Stop.
                                let next = recording
                                    .borrow()
                                    .as_ref()
                                    .and_then(|active| active.pipeline().next_frame());
                                if let Some(Ok((ts, surface))) = next {
                                    // Export buffer, capped so a long
                                    // session cannot grow unbounded.
                                    let mut buf = frames.borrow_mut();
                                    if buf.len() < MAX_FRAMES {
                                        buf.push((ts, surface.clone()));
                                    }
                                    drop(buf);
                                    frame_size.set((
                                        f64::from(surface.width()),
                                        f64::from(surface.height()),
                                    ));
                                    if let Some(texture) = surface_to_texture(&surface) {
                                        preview.set_paintable(Some(&texture));
                                    }
                                }
                                gtk4::glib::ControlFlow::Continue
                            }
                        });
                        *timer_id.borrow_mut() = Some(source_id);
                    }
                    Err(e) => {
                        eprintln!("recording failed: {e}");
                        record_button.set_sensitive(true);
                    }
                }
            });
        }
    });

    stop_button.connect_clicked({
        let recording = recording.clone();
        let timer_id = timer_id.clone();
        let frames = frames.clone();
        let trim_end = trim_end.clone();
        let record_button = record_button.clone();
        let export_button = export_button.clone();
        move |button| {
            button.set_sensitive(false);
            if let Some(source_id) = timer_id.borrow_mut().take() {
                source_id.remove();
            }
            if let Some(active) = recording.borrow_mut().take() {
                // Pipeline first, then Session::close — see
                // Recording::stop in capture.rs.
                if let Err(e) = active.stop() {
                    eprintln!("stop failed: {e}");
                }
            }
            let last_ts = frames.borrow().last().map(|f| f.0);
            export_button.set_sensitive(last_ts.is_some());
            if let Some(ts) = last_ts {
                // Default trim covers the whole recording.
                trim_end.set_range(0.0, ts as f64);
                trim_end.set_value(ts as f64);
            }
            record_button.set_sensitive(true);
        }
    });

    export_button.connect_clicked({
        let frames = frames.clone();
        let annotations = annotations.clone();
        let window = window.clone();
        let export_button = export_button.clone();
        move |_| {
            // FileChooserDialog (FileChooserExt::file/set_action/
            // set_current_name, docs.rs gtk4). Deprecated markers on
            // it are v4_10-gated and inactive here; FileDialog would
            // need that feature.
            let dialog = gtk4::FileChooserDialog::new(
                Some("Export GIF"),
                Some(&window),
                gtk4::FileChooserAction::Save,
                &[
                    ("Cancel", gtk4::ResponseType::Cancel),
                    ("Export", gtk4::ResponseType::Accept),
                ],
            );
            dialog.set_current_name("bloop.gif");
            // GIF-only filter — FileFilter::add_mime_type,
            // FileChooserExt::add_filter (docs.rs gtk4).
            let filter = gtk4::FileFilter::new();
            filter.set_name(Some("GIF images"));
            filter.add_mime_type("image/gif");
            dialog.add_filter(&filter);
            dialog.connect_response({
                let frames = frames.clone();
                let annotations = annotations.clone();
                let export_button = export_button.clone();
                let fps_spin = fps_spin.clone();
                let scale_spin = scale_spin.clone();
                let trim_start = trim_start.clone();
                let trim_end = trim_end.clone();
                move |dialog, response| {
                    if response == gtk4::ResponseType::Accept {
                        let start_ms = trim_start.value() as u64;
                        let end_raw = trim_end.value() as u64;
                        let end_ms = if end_raw == 0 { u64::MAX } else { end_raw };
                        if end_raw != 0 && start_ms > end_raw {
                            eprintln!(
                                "trim start {start_ms} is after trim end {end_raw}; export aborted"
                            );
                        } else if let Some(path) = dialog.file().and_then(|f| f.path()) {
                            // fps feeds the size estimate (timing comes
                            // from frame PTS), scale_percent resizes
                            // output.
                            match export::encode_gif(
                                frames.borrow().as_slice(),
                                annotations.borrow().as_slice(),
                                fps_spin.value() as u32,
                                scale_spin.value() as u32,
                                start_ms,
                                end_ms,
                            ) {
                                Ok(gif) => match std::fs::write(&path, &gif.bytes) {
                                    Ok(()) => {
                                        eprintln!(
                                            "exported {} bytes to {}",
                                            gif.bytes.len(),
                                            path.display()
                                        );
                                        frames.borrow_mut().clear();
                                        export_button.set_sensitive(false);
                                    }
                                    Err(e) => eprintln!("write failed: {e}"),
                                },
                                Err(e) => eprintln!("encode failed: {e}"),
                            }
                        }
                    }
                    dialog.close();
                }
            });
            dialog.present();
        }
    });

    window
}

/// The rect inside the preview widget where the frame is actually
/// drawn. Picture uses ContentFit::Contain — uniform scale to fit,
/// centered, letterboxed (gtk4 GtkPicture content-fit docs). Returns
/// (x, y, width, height) in widget coordinates.
fn picture_content_rect(widget: (f64, f64), frame: (f64, f64)) -> (f64, f64, f64, f64) {
    let (ww, wh) = widget;
    let (fw, fh) = frame;
    if ww <= 0.0 || wh <= 0.0 || fw <= 0.0 || fh <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let scale = (ww / fw).min(wh / fh);
    let (cw, ch) = (fw * scale, fh * scale);
    ((ww - cw) / 2.0, (wh - ch) / 2.0, cw, ch)
}

/// Widget coordinates to frame pixel coordinates through the
/// letterboxed content rect; points outside the rect clamp to the
/// frame edge.
fn widget_to_frame(x: f64, y: f64, content: (f64, f64, f64, f64), frame: (f64, f64)) -> (u32, u32) {
    let (cx, cy, cw, ch) = content;
    let (fw, fh) = frame;
    if cw <= 0.0 || ch <= 0.0 {
        return (0, 0);
    }
    let fx = ((x - cx) / cw * fw).round().clamp(0.0, fw);
    let fy = ((y - cy) / ch * fh).round().clamp(0.0, fh);
    (fx as u32, fy as u32)
}

/// Inverse of widget_to_frame, so stored frame-space annotations draw
/// back onto the letterboxed overlay.
fn frame_to_widget(x: f64, y: f64, content: (f64, f64, f64, f64), frame: (f64, f64)) -> (f64, f64) {
    let (cx, cy, cw, ch) = content;
    let (fw, fh) = frame;
    if fw <= 0.0 || fh <= 0.0 {
        return (x, y);
    }
    (cx + x / fw * cw, cy + y / fh * ch)
}

/// Real frame size once frames arrive; before the first frame the
/// widget itself is a 1:1 sketch space.
fn effective_frame_size(frame: (f64, f64), widget: (f64, f64)) -> (f64, f64) {
    if frame.0 > 0.0 && frame.1 > 0.0 {
        frame
    } else {
        widget
    }
}

/// Strokes one rectangle on the overlay canvas. Color parses through
/// gdk::RGBA::parse which accepts the "#RRGGBB" form annotation.rs
/// validates (docs.rs gtk4 gdk::RGBA). rect is (x, y, width, height).
fn stroke_box(
    cr: &cairo::Context,
    color: &str,
    thickness: f64,
    alpha: f64,
    rect: (f64, f64, f64, f64),
) {
    if let Ok(rgba) = gtk4::gdk::RGBA::parse(color) {
        cr.set_source_rgba(
            f64::from(rgba.red()),
            f64::from(rgba.green()),
            f64::from(rgba.blue()),
            alpha,
        );
        cr.set_line_width(thickness);
        cr.rectangle(rect.0, rect.1, rect.2, rect.3);
        let _ = cr.stroke();
    }
}

/// Uploads a cairo frame to a texture for the preview Picture.
/// gdk::MemoryTexture::new copies raw pixel rows with an explicit
/// format and stride (docs.rs gtk4 gdk::MemoryTexture::new). cairo
/// Format::ARgb32 is premultiplied native-endian ARGB — B,G,R,A byte
/// order on little-endian — which is MemoryFormat::B8g8r8a8Premultiplied
/// (cairo-rs Format docs; gdk4 MemoryFormat docs). flush() is required
/// by cairo before reading surface data outside cairo (cairo_surface_
/// flush); with_data gives a shared borrow of the pixels (cairo-rs
/// ImageSurface::with_data).
fn surface_to_texture(surface: &ImageSurface) -> Option<gtk4::gdk::Texture> {
    let width = surface.width();
    let height = surface.height();
    if width <= 0 || height <= 0 {
        return None;
    }
    surface.flush();
    let mut data = Vec::new();
    surface.with_data(|d| data = d.to_vec()).ok()?;
    let stride = surface.stride() as usize;
    if data.len() < stride * height as usize {
        return None;
    }
    let bytes = gtk4::glib::Bytes::from_owned(data);
    Some(
        gtk4::gdk::MemoryTexture::new(
            width,
            height,
            gtk4::gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &bytes,
            stride,
        )
        .upcast(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_window_title_is_bloop() {
        // Widget tests need a display; skip when headless.
        if gtk4::init().is_err() {
            eprintln!("no display available; skipping window test");
            return;
        }
        adw::init().unwrap();
        let app = adw::Application::builder()
            .application_id("io.github.open-qa-tools.Bloop.Test")
            .build();
        // Registration emits GApplication::startup; windows created
        // before it trigger a Gtk-CRITICAL warning.
        app.register(gtk4::gio::Cancellable::NONE).unwrap();
        let window = build_main_window(&app);
        assert_eq!(window.title().as_deref(), Some("Bloop"));
    }

    #[test]
    fn maps_centered_video_no_letterbox() {
        let content = picture_content_rect((100.0, 100.0), (100.0, 100.0));
        assert_eq!(content, (0.0, 0.0, 100.0, 100.0));
        let frame = (100.0, 100.0);
        assert_eq!(widget_to_frame(50.0, 50.0, content, frame), (50, 50));
        assert_eq!(widget_to_frame(0.0, 0.0, content, frame), (0, 0));
        assert_eq!(widget_to_frame(100.0, 100.0, content, frame), (100, 100));
    }

    #[test]
    fn maps_letterboxed_video_bars_top_bottom() {
        // 100x120 widget, 100x100 frame: 10px bars top and bottom.
        let content = picture_content_rect((100.0, 120.0), (100.0, 100.0));
        assert_eq!(content, (0.0, 10.0, 100.0, 100.0));
        let frame = (100.0, 100.0);
        assert_eq!(widget_to_frame(50.0, 60.0, content, frame), (50, 50));
        assert_eq!(widget_to_frame(50.0, 10.0, content, frame), (50, 0));
        assert_eq!(widget_to_frame(50.0, 110.0, content, frame), (50, 100));
    }

    #[test]
    fn maps_letterboxed_video_bars_left_right() {
        // 140x100 widget, 100x100 frame: 20px bars left and right.
        let content = picture_content_rect((140.0, 100.0), (100.0, 100.0));
        assert_eq!(content, (20.0, 0.0, 100.0, 100.0));
        let frame = (100.0, 100.0);
        assert_eq!(widget_to_frame(70.0, 50.0, content, frame), (50, 50));
        assert_eq!(widget_to_frame(20.0, 50.0, content, frame), (0, 50));
        assert_eq!(widget_to_frame(120.0, 50.0, content, frame), (100, 50));
    }

    #[test]
    fn point_outside_content_rect_clamps_to_edge() {
        let content = (10.0, 10.0, 80.0, 80.0);
        let frame = (100.0, 100.0);
        assert_eq!(widget_to_frame(-50.0, -50.0, content, frame), (0, 0));
        assert_eq!(widget_to_frame(200.0, 200.0, content, frame), (100, 100));
        // Outside horizontally but inside vertically: x clamps, y maps.
        assert_eq!(widget_to_frame(0.0, 50.0, content, frame), (0, 50));
        assert_eq!(widget_to_frame(500.0, 50.0, content, frame), (100, 50));
    }

    #[test]
    fn frame_to_widget_round_trips() {
        let widget = (140.0, 100.0);
        let frame = (100.0, 100.0);
        let content = picture_content_rect(widget, frame);
        let (fx, fy) = widget_to_frame(70.0, 50.0, content, frame);
        let (wx, wy) = frame_to_widget(f64::from(fx), f64::from(fy), content, frame);
        assert!((wx - 70.0).abs() < 1.0 && (wy - 50.0).abs() < 1.0);
    }
}
