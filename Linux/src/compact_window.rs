//! Compact window: shows only the download / upload rates of the tray interface (for desktops without a system tray, or for keeping rates on the desktop).
//! GTK4 can't position windows or keep them on top itself: the whole window is draggable (GtkWindowHandle), and right-click opens the system window menu
//! (where "Always on Top" can be set). Closing the window is the same as turning it off in Settings.

use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::config;
use crate::traffic_model::TrafficModel;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct CompactWindow {
        pub(super) down: gtk::Label,
        pub(super) up: gtk::Label,
        pub(super) not_set: gtk::Label,
        pub(super) model: OnceCell<TrafficModel>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CompactWindow {
        const NAME: &'static str = "AnemobitCompactWindow";
        type Type = super::CompactWindow;
        type ParentType = adw::Window;
    }

    impl ObjectImpl for CompactWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_title(Some("Anemobit"));
            obj.set_resizable(false);
            // AdwWindow's default minimum is 360 × 200; the compact window only needs to be slightly larger than its text.
            obj.set_size_request(1, 1);

            self.down.add_css_class("anemobit-download");
            self.up.add_css_class("anemobit-upload");
            self.not_set.set_label(&gettext("Anemobit: Not Set"));
            let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            content.add_css_class("anemobit-compact");
            content.append(&self.down);
            content.append(&self.up);
            content.append(&self.not_set);
            obj.set_content(Some(&gtk::WindowHandle::builder().child(&content).build()));
        }
    }

    impl WidgetImpl for CompactWindow {}

    impl WindowImpl for CompactWindow {
        /// Closing the window turns off the compact window in Settings (the application removes the window per the setting).
        fn close_request(&self) -> glib::Propagation {
            let settings = gio::Settings::new(config::APP_ID).child("compact-window");
            if let Err(error) = settings.set_boolean("visible", false) {
                glib::g_warning!(
                    "anemobit",
                    "Could not save the compact window setting: {error}"
                );
            }
            self.parent_close_request()
        }
    }

    impl AdwWindowImpl for CompactWindow {}
}

glib::wrapper! {
    pub struct CompactWindow(ObjectSubclass<imp::CompactWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root,
            gtk::ShortcutManager;
}

impl CompactWindow {
    pub fn new(app: &impl IsA<gtk::Application>, model: &TrafficModel) -> Self {
        let window: Self = glib::Object::builder().property("application", app).build();
        window
            .imp()
            .model
            .set(model.clone())
            .expect("model is set once");
        let weak = window.downgrade();
        model.connect_changed(move |_| {
            if let Some(window) = weak.upgrade() {
                window.refresh();
            }
        });
        window.refresh();
        window
    }

    fn refresh(&self) {
        let imp = self.imp();
        let Some(model) = imp.model.get() else { return };
        let rates = model.state().compact_rates(&model.settings());
        imp.not_set.set_visible(rates.is_none());
        imp.down.set_visible(rates.is_some());
        imp.up.set_visible(rates.is_some());
        if let Some((down, up)) = rates {
            imp.down.set_label(&down);
            imp.up.set_label(&up);
            self.update_property(&[gtk::accessible::Property::Label(&format!("{down} {up}"))]);
        }
    }
}
