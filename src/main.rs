mod annotation;
mod capture;
mod export;
mod pipeline;
mod project;
mod ui;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};

const APP_ID: &str = "io.github.open-qa-tools.Bloop";

fn main() -> gtk4::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_main_window);
    app.run()
}

fn build_main_window(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Bloop")
        .default_width(800)
        .default_height(600)
        .build();
    window.present();
}
