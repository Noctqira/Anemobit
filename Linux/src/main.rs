mod about;
mod application;
mod autostart;
mod compact_window;
mod config;
mod language;
mod popup;
mod secrets;
mod settings;
mod settings_dialog;
mod settings_draft;
mod text;
mod theme;
mod traffic_model;
mod tray;
mod widgets;
mod window;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::application::AnemobitApplication;

fn main() -> glib::ExitCode {
    // Before anything is translated: may restart the process in the chosen language.
    language::apply_at_startup();
    language::wait_for_previous_instance();

    // No need to call setlocale ourselves: GTK does it during initialization (setlocale(LC_ALL, "")).
    gettextrs::bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR)
        .expect("Unable to bind the text domain");
    gettextrs::bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8")
        .expect("Unable to set the text domain encoding");
    gettextrs::textdomain(config::GETTEXT_PACKAGE).expect("Unable to switch to the text domain");

    let resources = gio::Resource::from_data(&glib::Bytes::from_static(config::RESOURCES))
        .expect("Could not load resources");
    gio::resources_register(&resources);

    glib::set_application_name("Anemobit");

    AnemobitApplication::new().run()
}
