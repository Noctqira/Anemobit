//! Popup opened from the system tray icon (same as the macOS menu bar popover): rates for the tray interface, a mini chart,
//! connection status, and Main Window / Settings / Quit. On Wayland the system decides the position; it closes on focus loss or Esc.

use std::cell::OnceCell;
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use anemobit_core::formatting::rate_parts;
use gettextrs::gettext;
use gtk::{CompositeTemplate, glib};

use crate::text;
use crate::theme::Palette;
use crate::traffic_model::{ConnectionState, TrafficModel};
use crate::widgets::rate_gauge::TrafficDirection;
use crate::widgets::status_pill::StatusPill;
use crate::widgets::traffic_chart::TrafficChart;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/noctqira/Anemobit/ui/popup.ui")]
    pub struct Popup {
        #[template_child]
        pub(super) not_configured: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) configured: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) interface_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) host_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) status_pill: TemplateChild<StatusPill>,
        #[template_child]
        pub(super) download_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) download_value: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) download_unit: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) upload_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) upload_value: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) upload_unit: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) chart: TemplateChild<TrafficChart>,
        #[template_child]
        pub(super) detail_label: TemplateChild<gtk::Label>,
        pub(super) model: OnceCell<TrafficModel>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Popup {
        const NAME: &'static str = "AnemobitPopup";
        type Type = super::Popup;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            StatusPill::ensure_type();
            TrafficChart::ensure_type();
            klass.bind_template();
            klass.add_binding_action(
                gtk::gdk::Key::Escape,
                gtk::gdk::ModifierType::empty(),
                "window.close",
            );
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Popup {
        fn constructed(&self) {
            self.parent_constructed();
            // Background opacity follows the settings (theme::translucency_css).
            self.obj().add_css_class("anemobit-tray-window");
            self.download_title
                .set_label(&TrafficDirection::Download.title());
            self.upload_title
                .set_label(&TrafficDirection::Upload.title());
            // Like a popover, close when clicking elsewhere (focus loss). It has no focus right after opening, so skip the first change.
            self.obj().connect_is_active_notify(|popup| {
                if !popup.is_active() && popup.is_visible() {
                    let popup = popup.clone();
                    glib::timeout_add_local_once(Duration::from_millis(150), move || {
                        if !popup.is_active() {
                            popup.close();
                        }
                    });
                }
            });
        }
    }

    impl WidgetImpl for Popup {}
    impl WindowImpl for Popup {}
    impl AdwWindowImpl for Popup {}
}

glib::wrapper! {
    pub struct Popup(ObjectSubclass<imp::Popup>)
        @extends adw::Window, gtk::Window, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root,
            gtk::ShortcutManager;
}

impl Popup {
    pub fn new(app: &impl IsA<gtk::Application>, model: &TrafficModel) -> Self {
        let popup: Self = glib::Object::builder().property("application", app).build();
        popup
            .imp()
            .model
            .set(model.clone())
            .expect("model is set once");
        let weak = popup.downgrade();
        model.connect_changed(move |_| {
            if let Some(popup) = weak.upgrade() {
                popup.refresh();
            }
        });
        popup.refresh();
        popup
    }

    fn refresh(&self) {
        let imp = self.imp();
        let Some(model) = imp.model.get() else { return };
        let settings = model.settings();
        let state = model.state();
        let configured = state.connection != ConnectionState::NotConfigured;
        imp.not_configured.set_visible(!configured);
        imp.configured.set_visible(configured);
        if !configured {
            return;
        }

        let interface = state.tray_interface(&settings);
        imp.interface_label
            .set_label(interface.map_or("", |i| i.name.as_str()));
        imp.host_label.set_label(&settings.host);
        imp.status_pill.set_state(&state.connection);

        let latest = interface.and_then(|i| i.latest());
        for (rate, value, unit) in [
            (
                latest.map(|s| s.in_bps),
                &imp.download_value,
                &imp.download_unit,
            ),
            (
                latest.map(|s| s.out_bps),
                &imp.upload_value,
                &imp.upload_unit,
            ),
        ] {
            match rate {
                Some(bps) => {
                    let parts = rate_parts(bps, settings.unit_style);
                    value.set_label(&parts.value);
                    unit.set_label(parts.unit);
                }
                None => {
                    value.set_label("--");
                    unit.set_label("");
                }
            }
        }

        let palette = Palette::from_settings(adw::StyleManager::default().is_dark(), &settings);
        imp.chart.set_palette(&palette);
        imp.chart.set_unit_style(settings.unit_style);
        imp.chart.set_samples(
            interface
                .map(|i| i.history().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
        );

        let detail = &imp.detail_label;
        detail.remove_css_class("error");
        detail.remove_css_class("dim-label");
        match &state.connection {
            ConnectionState::Failed { error, .. } => {
                detail.set_label(&text::snmp_error(error));
                detail.add_css_class("error");
                detail.set_visible(true);
            }
            _ => match state.device_uptime {
                Some(uptime) => {
                    detail.set_label(&gettext("Device up {}").replacen(
                        "{}",
                        &text::uptime(uptime),
                        1,
                    ));
                    detail.add_css_class("dim-label");
                    detail.set_visible(true);
                }
                None => detail.set_visible(false),
            },
        }
    }
}
