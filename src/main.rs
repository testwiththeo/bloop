mod annotation;
mod capture;
mod export;
mod pipeline;
mod project;
mod ui;

use adw::prelude::*;

const APP_ID: &str = "io.github.open-qa-tools.Bloop";

fn main() -> gtk4::glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| ui::build_main_window(app).present());
    app.run()
}
