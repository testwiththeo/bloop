// Main window: LibAdwaita ApplicationWindow with a preview placeholder
// and Record/Stop controls wired to capture::start_recording.
// No annotation editor, no export dialog yet.
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use cairo::ImageSurface;

use crate::capture::{self, AshpdPortal, Recording};
use crate::pipeline::Pipeline;

const WINDOW_TITLE: &str = "Bloop";

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
    let preview_frame = gtk4::Frame::new(Some("Preview"));
    preview_frame.set_child(Some(&preview));
    preview_frame.set_vexpand(true);

    let controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    controls.set_halign(gtk4::Align::Center);
    controls.append(&record_button);
    controls.append(&stop_button);

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

    record_button.connect_clicked({
        let recording = recording.clone();
        let timer_id = timer_id.clone();
        let preview = preview.clone();
        let record_button = record_button.clone();
        let stop_button = stop_button.clone();
        move |_| {
            record_button.set_sensitive(false);
            let recording = recording.clone();
            let timer_id = timer_id.clone();
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
                            let preview = preview.clone();
                            move || {
                                // None or Err keeps the last frame
                                // and continues until Stop.
                                let next = recording
                                    .borrow()
                                    .as_ref()
                                    .and_then(|active| active.pipeline().next_frame());
                                if let Some(Ok((_, surface))) = next {
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
        let record_button = record_button.clone();
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
            record_button.set_sensitive(true);
        }
    });

    window
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
}
