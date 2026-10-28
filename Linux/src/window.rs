use std::cell::{Cell, OnceCell, RefCell};
use std::time::SystemTime;

use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};

use crate::application::AnemobitApplication;
use crate::config;
use crate::text;
use crate::theme::Palette;
use crate::traffic_model::{ConnectionState, TrafficModel};
use crate::widgets::interface_card::InterfaceCard;
use crate::widgets::status_pill::StatusPill;

mod imp {
    use super::*;

    #[derive(Debug, gtk::CompositeTemplate, glib::Properties)]
    #[template(resource = "/io/github/noctqira/Anemobit/ui/window.ui")]
    #[properties(wrapper_type = super::AnemobitWindow)]
    pub struct AnemobitWindow {
        #[template_child]
        pub(super) window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub(super) status_pill: TemplateChild<StatusPill>,
        #[template_child]
        pub(super) content_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) not_configured: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) cards: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) footer: TemplateChild<gtk::Box>,
        #[template_child]
        pub(super) uptime_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) updated_label: TemplateChild<gtk::Label>,

        /// True when the window is narrow (< 700), set by AdwBreakpoint.
        #[property(get, set = Self::set_compact)]
        compact: Cell<bool>,

        pub(super) settings: gio::Settings,
        pub(super) model: OnceCell<TrafficModel>,
        /// Interface names the current cards correspond to (cards are rebuilt only when interfaces change).
        pub(super) card_names: RefCell<Vec<String>>,
    }

    impl Default for AnemobitWindow {
        fn default() -> Self {
            AnemobitWindow {
                window_title: TemplateChild::default(),
                status_pill: TemplateChild::default(),
                content_box: TemplateChild::default(),
                not_configured: TemplateChild::default(),
                cards: TemplateChild::default(),
                footer: TemplateChild::default(),
                uptime_label: TemplateChild::default(),
                updated_label: TemplateChild::default(),
                compact: Cell::new(false),
                settings: gio::Settings::new(config::APP_ID),
                model: OnceCell::new(),
                card_names: RefCell::default(),
            }
        }
    }

    impl AnemobitWindow {
        fn set_compact(&self, compact: bool) {
            self.compact.set(compact);
            let margin = if compact { 16 } else { 20 };
            self.content_box.set_margin_start(margin);
            self.content_box.set_margin_end(margin);
            self.content_box.set_spacing(if compact { 12 } else { 16 });
            self.cards.set_spacing(if compact { 12 } else { 16 });
            let mut child = self.cards.first_child();
            while let Some(card) = child {
                if let Some(card) = card.downcast_ref::<InterfaceCard>() {
                    card.set_compact(compact);
                }
                child = card.next_sibling();
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AnemobitWindow {
        const NAME: &'static str = "AnemobitWindow";
        type Type = super::AnemobitWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            StatusPill::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for AnemobitWindow {
        fn constructed(&self) {
            self.parent_constructed();
            // The development build's header bar gets stripes to tell it apart from the release build.
            if config::PROFILE == "Devel" {
                self.obj().add_css_class("devel");
            }
            // Background opacity follows the settings (theme::translucency_css).
            self.obj().add_css_class("anemobit-main-window");
            self.obj().restore_size();
        }
    }

    impl WidgetImpl for AnemobitWindow {}

    impl WindowImpl for AnemobitWindow {
        fn close_request(&self) -> glib::Propagation {
            self.obj().save_size();
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for AnemobitWindow {}
    impl AdwApplicationWindowImpl for AnemobitWindow {}
}

glib::wrapper! {
    pub struct AnemobitWindow(ObjectSubclass<imp::AnemobitWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
            gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl AnemobitWindow {
    pub fn new(app: &AnemobitApplication, model: &TrafficModel) -> Self {
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
        // Pick colors again when switching light / dark.
        let weak = window.downgrade();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(window) = weak.upgrade() {
                window.refresh();
            }
        });
        window.refresh();
        window
    }

    /// Updates the whole UI from the model's current state.
    fn refresh(&self) {
        let imp = self.imp();
        let Some(model) = imp.model.get() else { return };
        let settings = model.settings();
        let state = model.state();

        let (title, subtitle) = state.titles(&settings.host);
        imp.window_title.set_title(&title);
        imp.window_title.set_subtitle(&subtitle);
        let configured = state.connection != ConnectionState::NotConfigured;
        imp.status_pill.set_visible(configured);
        imp.status_pill.set_state(&state.connection);
        imp.not_configured.set_visible(!configured);
        imp.cards.set_visible(configured);
        imp.footer.set_visible(configured);

        // Rebuild cards only when interfaces change.
        let names: Vec<String> = state.interfaces.iter().map(|i| i.name.clone()).collect();
        if *imp.card_names.borrow() != names {
            while let Some(child) = imp.cards.first_child() {
                imp.cards.remove(&child);
            }
            for name in &names {
                let card = InterfaceCard::new(name);
                card.set_compact(self.compact());
                imp.cards.append(&card);
            }
            imp.card_names.replace(names);
        }

        let palette = Palette::from_settings(adw::StyleManager::default().is_dark(), &settings);
        // Charts are always shown for a single interface.
        let shows_chart = settings.charts_when_multiple || state.interfaces.len() == 1;
        let now = SystemTime::now();
        let mut child = imp.cards.first_child();
        for interface in &state.interfaces {
            let Some(widget) = child else { break };
            if let Some(card) = widget.downcast_ref::<InterfaceCard>() {
                card.set_palette(&palette);
                card.update(interface, settings.unit_style, shows_chart, now);
            }
            child = widget.next_sibling();
        }

        // Translators: {} is an uptime such as "12 days, 5 hours".
        let uptime = state
            .device_uptime
            .map(|d| gettext("Device up {}").replacen("{}", &text::uptime(d), 1));
        imp.uptime_label.set_label(uptime.as_deref().unwrap_or(""));
        let updated = state.last_updated.and_then(|time| {
            let seconds = time.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
            let local = glib::DateTime::from_unix_local(i64::try_from(seconds).ok()?).ok()?;
            // Translators: {} is a time of day such as "14:05:09".
            Some(gettext("Updated {}").replacen("{}", local.format("%X").ok()?.as_str(), 1))
        });
        imp.updated_label
            .set_label(updated.as_deref().unwrap_or(""));
    }

    /// Restores the size and maximized state from the last close (GNOME convention).
    fn restore_size(&self) {
        let settings = &self.imp().settings;
        self.set_default_size(settings.int("window-width"), settings.int("window-height"));
        if settings.boolean("window-maximized") {
            self.maximize();
        }
    }

    /// When maximized, only the state is recorded, not the maximized size, so restoring returns to the original size.
    fn save_size(&self) {
        let settings = &self.imp().settings;
        let result = if self.is_maximized() {
            settings.set_boolean("window-maximized", true)
        } else {
            let (width, height) = self.default_size();
            settings
                .set_int("window-width", width)
                .and_then(|()| settings.set_int("window-height", height))
                .and_then(|()| settings.set_boolean("window-maximized", false))
        };
        if let Err(error) = result {
            glib::g_warning!("anemobit", "Could not save the window size: {error}");
        }
    }
}
