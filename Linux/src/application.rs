use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::about;
use crate::compact_window::CompactWindow;
use crate::config;
use crate::language;
use crate::popup::Popup;
use crate::secrets::{KeyringStore, load_secrets};
use crate::settings::AppSettings;
use crate::settings_dialog::SettingsDialog;
use crate::theme::{Palette, translucency_css};
use crate::traffic_model::TrafficModel;
use crate::tray::status_notifier::{StatusNotifier, TrayHandler};
use crate::tray::{
    self, MENU_ABOUT, MENU_COMPACT_WINDOW, MENU_OPEN_WINDOW, MENU_QUIT, MENU_SETTINGS,
};
use crate::window::AnemobitWindow;

/// Argument used for Open at Login (autostart): with a system tray, only stay in the tray without opening the main window.
pub const STARTUP_OPTION: &str = "startup";

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct AnemobitApplication {
        /// Traffic state shared by the main window, system tray and compact window.
        pub(super) model: OnceCell<TrafficModel>,
        /// CSS variables for download / upload / status colors (updated for light / dark and custom colors). They can only be created after GTK initializes,
        /// so they're created at startup.
        pub(super) palette_css: OnceCell<gtk::CssProvider>,
        pub(super) tray: RefCell<Option<StatusNotifier>>,
        /// Whether the system tray service exists; `None` while still unknown.
        pub(super) tray_available: Cell<Option<bool>>,
        /// Keeps the app running when there's a system tray (closing the main window doesn't quit).
        pub(super) hold: RefCell<Option<gio::ApplicationHoldGuard>>,
        pub(super) popup: RefCell<Option<Popup>>,
        pub(super) compact: RefCell<Option<CompactWindow>>,
        /// Watches `compact-window/visible` (changed by saving Settings, the tray menu, and closing the compact window).
        pub(super) compact_settings: OnceCell<gio::Settings>,
        /// Launched with `--startup` and still waiting to learn whether a system tray exists.
        pub(super) startup_pending: Cell<bool>,
        pub(super) started_from_login: Cell<bool>,
        /// Launched with `--relaunch` (Restart Now after changing the language).
        pub(super) relaunched: Cell<bool>,
        /// Settings and passwords have been read (the keyring is read asynchronously at startup).
        pub(super) settings_loaded: Cell<bool>,
        /// Settings was requested before loading finished; opened once it has.
        pub(super) preferences_pending: Cell<bool>,
    }

    impl std::fmt::Debug for StatusNotifier {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("StatusNotifier")
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AnemobitApplication {
        const NAME: &'static str = "AnemobitApplication";
        type Type = super::AnemobitApplication;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for AnemobitApplication {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().add_main_option(
                STARTUP_OPTION,
                glib::Char::from(0),
                glib::OptionFlags::NONE,
                glib::OptionArg::None,
                "Start in the system tray (used when opening at login)",
                None,
            );
            self.obj().add_main_option(
                language::RELAUNCH_OPTION,
                glib::Char::from(0),
                glib::OptionFlags::HIDDEN,
                glib::OptionArg::None,
                "Started by Restart Now in Settings",
                None,
            );
        }
    }

    impl ApplicationImpl for AnemobitApplication {
        fn handle_local_options(
            &self,
            options: &glib::VariantDict,
        ) -> std::ops::ControlFlow<glib::ExitCode> {
            if options.contains(STARTUP_OPTION) {
                self.started_from_login.set(true);
            }
            if options.contains(language::RELAUNCH_OPTION) {
                self.relaunched.set(true);
            }
            // Continue starting normally (hand off to the primary instance).
            std::ops::ControlFlow::Continue(())
        }

        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();
            let model = TrafficModel::default();
            self.model.set(model.clone()).expect("startup runs once");

            let palette_css = gtk::CssProvider::new();
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &palette_css,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            self.palette_css
                .set(palette_css)
                .expect("startup runs once");
            let weak = app.downgrade();
            model.connect_changed(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.update_palette();
                    app.update_tray();
                }
            });
            let weak = app.downgrade();
            adw::StyleManager::default().connect_dark_notify(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.update_palette();
                }
            });
            app.update_palette();
            app.setup_actions();
            app.watch_compact_window();
            app.start_tray();
            app.load_settings();
        }

        fn activate(&self) {
            self.parent_activate();
            let app = self.obj();
            // Restarted from Settings (language change): back to Settings, over the main window
            // (a dialog without a parent window doesn't keep the app running before the tray is known).
            if self.relaunched.replace(false) {
                app.show_main_window();
                app.show_preferences();
                return;
            }
            // Launched at login: with a system tray, only stay in the tray; if it's not known yet, decide once the result comes in.
            if self.started_from_login.replace(false) {
                match self.tray_available.get() {
                    Some(true) => return,
                    Some(false) => {}
                    None => {
                        self.startup_pending.set(true);
                        return;
                    }
                }
            }
            app.show_main_window();
        }
    }

    impl GtkApplicationImpl for AnemobitApplication {}
    impl AdwApplicationImpl for AnemobitApplication {}
}

glib::wrapper! {
    pub struct AnemobitApplication(ObjectSubclass<imp::AnemobitApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

/// Forwards system tray events to the application.
struct TrayEvents(glib::WeakRef<AnemobitApplication>);

impl TrayHandler for TrayEvents {
    fn activate(&self) {
        if let Some(app) = self.0.upgrade() {
            app.toggle_popup();
        }
    }

    fn menu_item(&self, id: i32) {
        let Some(app) = self.0.upgrade() else { return };
        match id {
            MENU_OPEN_WINDOW => app.show_main_window(),
            MENU_SETTINGS => app.show_preferences(),
            MENU_COMPACT_WINDOW => app.toggle_compact_window(),
            MENU_ABOUT => app.show_about(),
            MENU_QUIT => app.quit(),
            _ => {}
        }
    }

    fn availability_changed(&self, available: bool) {
        if let Some(app) = self.0.upgrade() {
            app.tray_availability_changed(available);
        }
    }
}

impl AnemobitApplication {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", config::APP_ID)
            // The development build has a different app ID, but the resource path stays fixed to the release one.
            .property("resource-base-path", "/io/github/noctqira/Anemobit/")
            .build()
    }

    fn setup_actions(&self) {
        let preferences = gio::ActionEntry::builder("preferences")
            .activate(|app: &Self, _, _| app.show_preferences())
            .build();
        let show_window = gio::ActionEntry::builder("show-window")
            .activate(|app: &Self, _, _| app.show_main_window())
            .build();
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &Self, _, _| app.quit())
            .build();
        let about = gio::ActionEntry::builder("about")
            .activate(|app: &Self, _, _| app.show_about())
            .build();
        self.add_action_entries([preferences, show_window, quit, about]);
        self.set_accels_for_action("app.preferences", &["<Control>comma"]);
        self.set_accels_for_action("app.quit", &["<Control>q"]);
    }

    /// About Anemobit (over the main window; standalone if no main window is open).
    fn show_about(&self) {
        if let Some(popup) = self.imp().popup.borrow().as_ref() {
            popup.close();
        }
        let parent = self
            .windows()
            .into_iter()
            .find(|window| window.is::<AnemobitWindow>() && window.is_visible());
        about::show(parent.as_ref());
    }

    /// Shows or removes the compact window according to `compact-window/visible`.
    fn watch_compact_window(&self) {
        let settings = gio::Settings::new(config::APP_ID).child("compact-window");
        let weak = self.downgrade();
        settings.connect_changed(Some("visible"), move |settings, _| {
            if let Some(app) = weak.upgrade() {
                app.set_compact_window_visible(settings.boolean("visible"));
            }
        });
        self.set_compact_window_visible(settings.boolean("visible"));
        self.imp()
            .compact_settings
            .set(settings)
            .expect("startup runs once");
    }

    fn set_compact_window_visible(&self, visible: bool) {
        let imp = self.imp();
        if visible == imp.compact.borrow().is_some() {
            return;
        }
        if visible {
            let window = CompactWindow::new(self, self.model());
            window.present();
            imp.compact.replace(Some(window));
        } else if let Some(window) = imp.compact.take() {
            window.destroy();
        }
        self.update_tray();
    }

    /// System tray menu: show / hide immediately (writes the setting, matching the Settings toggle).
    fn toggle_compact_window(&self) {
        let Some(settings) = self.imp().compact_settings.get() else {
            return;
        };
        let visible = settings.boolean("visible");
        if let Err(error) = settings.set_boolean("visible", !visible) {
            glib::g_warning!(
                "anemobit",
                "Could not save the compact window setting: {error}"
            );
        }
    }

    fn model(&self) -> &TrafficModel {
        self.imp().model.get().expect("model is created at startup")
    }

    /// Opens (or brings forward) the main window.
    pub fn show_main_window(&self) {
        let window = self
            .windows()
            .into_iter()
            .find(|window| window.is::<AnemobitWindow>())
            .unwrap_or_else(|| AnemobitWindow::new(self, self.model()).upcast());
        if let Some(popup) = self.imp().popup.borrow().as_ref() {
            popup.close();
        }
        window.present();
    }

    /// Opens the Settings window (over the current window; standalone if there's none).
    fn show_preferences(&self) {
        if let Some(popup) = self.imp().popup.borrow().as_ref() {
            popup.close();
        }
        // Before loading finishes, Settings would start from empty values (and saving would
        // overwrite the stored ones), so it opens once loading is done.
        if !self.imp().settings_loaded.get() {
            self.imp().preferences_pending.set(true);
            return;
        }
        let parent = self
            .windows()
            .into_iter()
            .find(|window| window.is::<AnemobitWindow>() && window.is_visible());
        SettingsDialog::new(self.model()).present(parent.as_ref());
    }

    /// Clicking the system tray icon: opens or closes the popup.
    fn toggle_popup(&self) {
        let imp = self.imp();
        let existing = imp.popup.borrow().clone();
        if let Some(popup) = existing.filter(|popup| popup.is_visible()) {
            popup.close();
            return;
        }
        let popup = Popup::new(self, self.model());
        popup.set_hide_on_close(false);
        imp.popup.replace(Some(popup.clone()));
        popup.present();
    }

    fn start_tray(&self) {
        let weak = self.downgrade();
        let content = {
            let model = self.model();
            tray::tray_content(
                &model.state(),
                &model.settings(),
                self.imp().compact.borrow().is_some(),
            )
        };
        glib::spawn_future_local(async move {
            let handler = Box::new(TrayEvents(weak.clone()));
            let result = StatusNotifier::start(content, handler).await;
            let Some(app) = weak.upgrade() else { return };
            match result {
                Ok(tray) => {
                    app.imp().tray.replace(Some(tray));
                    app.update_tray();
                }
                Err(error) => {
                    glib::g_warning!("anemobit", "Could not create the tray icon: {error}");
                    app.tray_availability_changed(false);
                }
            }
        });
    }

    fn update_tray(&self) {
        if let Some(tray) = self.imp().tray.borrow().as_ref() {
            let model = self.model();
            tray.update(tray::tray_content(
                &model.state(),
                &model.settings(),
                self.imp().compact.borrow().is_some(),
            ));
        }
    }

    /// With a system tray, keep running; without one, closing the main window quits, with a tip on first launch.
    fn tray_availability_changed(&self, available: bool) {
        let imp = self.imp();
        if imp.tray_available.replace(Some(available)) == Some(available) {
            return;
        }
        if available {
            if imp.hold.borrow().is_none() {
                imp.hold.replace(Some(self.hold()));
            }
        } else {
            imp.hold.replace(None);
        }
        if imp.startup_pending.replace(false) && !available {
            self.show_main_window();
        }
        if !available {
            self.show_tray_hint_once();
        }
    }

    /// No system tray detected on first launch: suggest installing the AppIndicator extension or turning on the compact window.
    fn show_tray_hint_once(&self) {
        let settings = gio::Settings::new(config::APP_ID);
        if settings.boolean("tray-hint-shown") {
            return;
        }
        let _ = settings.set_boolean("tray-hint-shown", true);
        self.show_main_window();
        let dialog = adw::AlertDialog::new(
            Some(&gettext("System Tray Not Available")),
            Some(&gettext(
                "To show the rate in the top bar, install and enable the AppIndicator extension. You can also turn on the compact window in Settings.",
            )),
        );
        dialog.add_response("ok", &gettext("OK"));
        let parent = self
            .windows()
            .into_iter()
            .find(|window| window.is::<AnemobitWindow>());
        dialog.present(parent.as_ref());
    }

    /// Reads GSettings and the passwords in the keyring, applies them, and starts polling.
    fn load_settings(&self) {
        let mut settings = AppSettings::read(&gio::Settings::new(config::APP_ID));
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            if let Err(error) = load_secrets(&KeyringStore, &mut settings.security).await {
                // Start anyway without a keyring service; the UI shows it as not set up.
                glib::g_warning!("anemobit", "Could not read the saved passwords: {error}");
            }
            let Some(app) = weak.upgrade() else {
                return;
            };
            app.model().apply_settings(settings);
            let imp = app.imp();
            imp.settings_loaded.set(true);
            if imp.preferences_pending.replace(false) {
                app.show_preferences();
            }
        });
    }

    /// CSS for colors and the translucent background (for light / dark and settings).
    fn update_palette(&self) {
        let dark = adw::StyleManager::default().is_dark();
        let settings = self.model().settings();
        let palette = Palette::from_settings(dark, &settings);
        let css = palette.css()
            + "\n"
            + &translucency_css(
                dark,
                settings.main_window_opacity,
                settings.tray_window_opacity,
            );
        if let Some(provider) = self.imp().palette_css.get() {
            provider.load_from_string(&css);
        }
    }
}
