// Main window: LibAdwaita ApplicationWindow with a preview placeholder
// and Record/Stop controls wired to capture::start_recording.
// No annotation editor, no export dialog yet.
use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

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

    let preview = gtk4::Frame::new(Some("Preview"));
    let preview_label = gtk4::Label::new(Some("Screen preview appears here"));
    preview_label.set_hexpand(true);
    preview_label.set_vexpand(true);
    preview.set_child(Some(&preview_label));
    preview.set_vexpand(true);

    let controls = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    controls.set_halign(gtk4::Align::Center);
    controls.append(&record_button);
    controls.append(&stop_button);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&preview);
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

    record_button.connect_clicked({
        let recording = recording.clone();
        let record_button = record_button.clone();
        let stop_button = stop_button.clone();
        move |_| {
            record_button.set_sensitive(false);
            let recording = recording.clone();
            let record_button = record_button.clone();
            let stop_button = stop_button.clone();
            gtk4::glib::MainContext::default().spawn_local(async move {
                match capture::start_recording().await {
                    Ok(active) => {
                        *recording.borrow_mut() = Some(active);
                        stop_button.set_sensitive(true);
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
        let record_button = record_button.clone();
        move |button| {
            button.set_sensitive(false);
            if let Some(active) = recording.borrow_mut().take() {
                // Sets the pipeline to Null so PipeWire disconnects;
                // dropping the session closes the remote fd. Portal
                // Session::close needs a Recording::stop in capture.rs
                // (it consumes the session, which Recording keeps
                // private) — follow-up task.
                let _ = active.pipeline().stop();
            }
            record_button.set_sensitive(true);
        }
    });

    window
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
