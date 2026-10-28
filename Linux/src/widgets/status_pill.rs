//! Connection status pill: OK (green), connecting (yellow), error (red), not set up (gray). Same as StatusPill on macOS.

use gettextrs::gettext;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::text;
use crate::traffic_model::ConnectionState;

const STATE_CLASSES: [&str; 4] = ["connected", "connecting", "failed", "not-configured"];

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct StatusPill {
        pub(super) label: gtk::Label,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StatusPill {
        const NAME: &'static str = "AnemobitStatusPill";
        type Type = super::StatusPill;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("anemobit-status-pill");
        }
    }

    impl ObjectImpl for StatusPill {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_spacing(6);
            obj.set_valign(gtk::Align::Center);
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("dot");
            dot.set_valign(gtk::Align::Center);
            obj.append(&dot);
            self.label.add_css_class("caption");
            self.label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            obj.append(&self.label);
        }
    }

    impl WidgetImpl for StatusPill {}
    impl BoxImpl for StatusPill {}
}

glib::wrapper! {
    pub struct StatusPill(ObjectSubclass<imp::StatusPill>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for StatusPill {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl StatusPill {
    pub fn set_state(&self, state: &ConnectionState) {
        let (class, label, tooltip) = match state {
            ConnectionState::Connected => ("connected", "OK".to_owned(), None),
            ConnectionState::Connecting => ("connecting", gettext("Connecting…"), None),
            ConnectionState::Failed { error, retry_in } => (
                "failed",
                text::retry_status(*retry_in),
                Some(text::snmp_error(error)),
            ),
            ConnectionState::NotConfigured => ("not-configured", gettext("Not Set"), None),
        };
        for other in STATE_CLASSES {
            self.remove_css_class(other);
        }
        self.add_css_class(class);
        self.imp().label.set_label(&label);
        self.set_tooltip_text(Some(tooltip.as_deref().unwrap_or(&label)));
        // Screen readers can read the error reason too.
        self.update_property(&[gtk::accessible::Property::Description(
            tooltip.as_deref().unwrap_or(""),
        )]);
    }
}
