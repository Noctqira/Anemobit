//! Settings window (same as SettingsView on macOS). The draft is in [`SettingsDraft`]; only Save writes GSettings
//! (delay + apply, all at once) and the keyring, then hands off to [`TrafficModel`]. Open at Login applies immediately.

use std::cell::{Cell, OnceCell, RefCell};
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use anemobit_core::formatting::{filter_host, filter_port};
use anemobit_core::oids::SYS_UP_TIME;
use anemobit_core::transport::UdpTransport;
use anemobit_core::{AuthProtocol, SnmpClient, SnmpError, SnmpSecurity, SnmpValue};
use gettextrs::gettext;
use gtk::{CompositeTemplate, gio, glib};

use crate::autostart::Autostart;
use crate::config;
use crate::language::{self, AppLanguage};
use crate::secrets::{KeyringStore, save_secrets};
use crate::settings::{
    SecurityMode, auth_protocol_label, priv_protocol_from_nick, priv_protocol_nick,
    unit_style_from_nick, unit_style_nick,
};
use crate::settings_draft::SettingsDraft;
use crate::text;
use crate::theme::{Palette, parse_hex, to_hex};
use crate::traffic_model::TrafficModel;
use crate::widgets::rate_gauge::TrafficDirection;

/// Status text at the bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Activity {
    Idle,
    Working(String),
    Success(String),
    Failure(String),
}

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/io/github/noctqira/Anemobit/ui/settings-dialog.ui")]
    pub struct SettingsDialog {
        #[template_child]
        pub(super) host_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) port_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) security_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) mode_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub(super) community_row: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub(super) user_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub(super) auth_protocol_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) auth_password_row: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub(super) privacy_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) privacy_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub(super) priv_password_row: TemplateChild<adw::PasswordEntryRow>,
        #[template_child]
        pub(super) problem_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) interfaces_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub(super) spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub(super) scan_button: TemplateChild<adw::ButtonRow>,
        #[template_child]
        pub(super) test_button: TemplateChild<adw::ButtonRow>,
        #[template_child]
        pub(super) no_interfaces_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) tray_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) interval_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub(super) interval_1: TemplateChild<adw::Toggle>,
        #[template_child]
        pub(super) interval_5: TemplateChild<adw::Toggle>,
        #[template_child]
        pub(super) interval_10: TemplateChild<adw::Toggle>,
        #[template_child]
        pub(super) unit_group: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub(super) charts_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) compact_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) main_opacity_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub(super) tray_opacity_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        pub(super) download_color_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) download_reset: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) download_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub(super) upload_color_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub(super) upload_reset: TemplateChild<gtk::Button>,
        #[template_child]
        pub(super) upload_color: TemplateChild<gtk::ColorDialogButton>,
        #[template_child]
        pub(super) language_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub(super) restart_button: TemplateChild<adw::ButtonRow>,
        #[template_child]
        pub(super) language_error: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) autostart_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub(super) autostart_error: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) activity_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub(super) save_button: TemplateChild<gtk::Button>,

        pub(super) model: OnceCell<TrafficModel>,
        pub(super) draft: RefCell<Option<SettingsDraft>>,
        /// Scan or connection test work; cancelled when the window closes or new work starts.
        pub(super) task: RefCell<Option<glib::JoinHandle<()>>>,
        /// Interface rows (name, row, check box), rebuilt only when the name list changes.
        pub(super) interface_rows: RefCell<Vec<(String, adw::ActionRow, gtk::CheckButton)>>,
        /// True while the code sets widget values, to avoid triggering change handlers.
        pub(super) updating: Cell<bool>,
        pub(super) activity: RefCell<Option<Activity>>,
        pub(super) autostart: Autostart,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsDialog {
        const NAME: &'static str = "AnemobitSettingsDialog";
        type Type = super::SettingsDialog;
        type ParentType = adw::Dialog;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SettingsDialog {}
    impl WidgetImpl for SettingsDialog {}

    impl AdwDialogImpl for SettingsDialog {
        fn closed(&self) {
            if let Some(task) = self.task.take() {
                task.abort();
            }
            self.parent_closed();
        }
    }
}

glib::wrapper! {
    pub struct SettingsDialog(ObjectSubclass<imp::SettingsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

impl SettingsDialog {
    pub fn new(model: &TrafficModel) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.model.set(model.clone()).expect("model is set once");
        imp.draft
            .replace(Some(SettingsDraft::new(&model.settings())));
        imp.activity.replace(Some(Activity::Idle));
        dialog.setup();
        dialog.populate();
        dialog.refresh();
        dialog
    }

    /// For previews: fills in a sample draft (Priv mode, scanned interfaces, two selected).
    #[cfg(test)]
    pub(crate) fn fill_preview(&self) {
        let imp = self.imp();
        if let Some(draft) = imp.draft.borrow_mut().as_mut() {
            draft.host = "192.168.88.1".into();
            draft.security.mode = SecurityMode::Private;
            draft.security.community = "anemobit".into();
            draft.security.auth_password = "authpass12".into();
            draft.security.priv_password = "short".into();
            draft.apply_scan(vec!["ether1".into(), "bridge".into(), "pppoe-out1".into()]);
            draft.set_selected("ether1", true);
        }
        self.populate();
        self.refresh();
    }

    /// Done once at creation: fixed text, menu contents and change handlers.
    fn setup(&self) {
        let imp = self.imp();
        imp.interval_1.set_label(Some(&text::interval(1)));
        imp.interval_5.set_label(Some(&text::interval(5)));
        imp.interval_10.set_label(Some(&text::interval(10)));
        imp.download_color_row
            .set_title(&TrafficDirection::Download.title());
        imp.upload_color_row
            .set_title(&TrafficDirection::Upload.title());
        let labels: Vec<&str> = AuthProtocol::ALL
            .iter()
            .map(|&p| auth_protocol_label(p))
            .collect();
        imp.auth_protocol_row
            .set_model(Some(&gtk::StringList::new(&labels)));
        let follow_system = gettext("Follow System");
        let languages: Vec<&str> = AppLanguage::ALL
            .iter()
            .map(|language| language.native_name().unwrap_or(&follow_system))
            .collect();
        imp.language_row
            .set_model(Some(&gtk::StringList::new(&languages)));

        // Address and port: whatever keyboard, input method or paste is used, only allowed characters remain.
        self.connect_text(&*imp.host_row, filter_host, |draft, text| draft.host = text);
        self.connect_text(&*imp.port_row, filter_port, |draft, text| {
            draft.port_text = text
        });
        // The community and the v3 user name are the same value (the RouterOS community name).
        self.connect_text(&*imp.community_row, str::to_owned, |draft, text| {
            draft.security.community = text
        });
        self.connect_text(&*imp.user_row, str::to_owned, |draft, text| {
            draft.security.community = text
        });
        self.connect_text(&*imp.auth_password_row, str::to_owned, |draft, text| {
            draft.security.auth_password = text
        });
        self.connect_text(&*imp.priv_password_row, str::to_owned, |draft, text| {
            draft.security.priv_password = text
        });

        let weak = self.downgrade();
        imp.mode_group.connect_active_name_notify(move |group| {
            let Some(dialog) = weak.upgrade() else { return };
            let mode = group
                .active_name()
                .and_then(|n| SecurityMode::from_nick(&n));
            dialog.edit(|draft| draft.security.mode = mode.unwrap_or_default());
            // After switching modes, the visible field gets the current community.
            dialog.populate_community();
        });
        let weak = self.downgrade();
        imp.auth_protocol_row.connect_selected_notify(move |row| {
            let Some(dialog) = weak.upgrade() else { return };
            if let Some(&protocol) = AuthProtocol::ALL.get(row.selected() as usize) {
                dialog.edit(|draft| draft.security.auth_protocol = protocol);
            }
        });
        let weak = self.downgrade();
        imp.privacy_group.connect_active_name_notify(move |group| {
            let Some(dialog) = weak.upgrade() else { return };
            if let Some(protocol) = group
                .active_name()
                .and_then(|n| priv_protocol_from_nick(&n))
            {
                dialog.edit(|draft| draft.security.priv_protocol = protocol);
            }
        });
        let weak = self.downgrade();
        imp.tray_row.connect_selected_notify(move |row| {
            let Some(dialog) = weak.upgrade() else { return };
            let name = row
                .selected_item()
                .and_downcast::<gtk::StringObject>()
                .map(|s| s.string().to_string());
            if let Some(name) = name {
                dialog.edit(|draft| draft.tray_interface = name);
            }
        });
        let weak = self.downgrade();
        imp.interval_group.connect_active_name_notify(move |group| {
            let Some(dialog) = weak.upgrade() else { return };
            if let Some(seconds) = group.active_name().and_then(|n| n.parse().ok()) {
                dialog.edit(|draft| draft.interval_seconds = seconds);
            }
        });
        let weak = self.downgrade();
        imp.unit_group.connect_active_name_notify(move |group| {
            let Some(dialog) = weak.upgrade() else { return };
            if let Some(style) = group.active_name().and_then(|n| unit_style_from_nick(&n)) {
                dialog.edit(|draft| draft.unit_style = style);
            }
        });
        let weak = self.downgrade();
        imp.charts_row.connect_active_notify(move |row| {
            let Some(dialog) = weak.upgrade() else { return };
            let active = row.is_active();
            dialog.edit(|draft| draft.charts_when_multiple = active);
        });
        for (scale, main) in [
            (&imp.main_opacity_scale, true),
            (&imp.tray_opacity_scale, false),
        ] {
            // Shown as a percentage, in 5% steps.
            scale.set_format_value_func(|_, value| format!("{value:.0}%"));
            let weak = self.downgrade();
            scale.connect_value_changed(move |scale| {
                let Some(dialog) = weak.upgrade() else { return };
                let opacity = (scale.value() / 5.0).round() * 5.0 / 100.0;
                dialog.edit(|draft| {
                    if main {
                        draft.main_window_opacity = opacity;
                    } else {
                        draft.tray_window_opacity = opacity;
                    }
                });
            });
        }
        let weak = self.downgrade();
        imp.compact_row.connect_active_notify(move |row| {
            let Some(dialog) = weak.upgrade() else { return };
            let active = row.is_active();
            dialog.edit(|draft| draft.compact_window = active);
        });

        for (button, reset, download) in [
            (&imp.download_color, &imp.download_reset, true),
            (&imp.upload_color, &imp.upload_reset, false),
        ] {
            let weak = self.downgrade();
            button.connect_rgba_notify(move |button| {
                let Some(dialog) = weak.upgrade() else { return };
                let hex = Some(to_hex(&button.rgba()));
                dialog.edit(|draft| {
                    if download {
                        draft.download_color = hex;
                    } else {
                        draft.upload_color = hex;
                    }
                });
            });
            let weak = self.downgrade();
            reset.connect_clicked(move |_| {
                let Some(dialog) = weak.upgrade() else { return };
                dialog.edit(|draft| {
                    if download {
                        draft.download_color = None;
                    } else {
                        draft.upload_color = None;
                    }
                });
            });
        }

        let weak = self.downgrade();
        imp.language_row.connect_selected_notify(move |row| {
            if let Some(dialog) = weak.upgrade()
                && let Some(&language) = AppLanguage::ALL.get(row.selected() as usize)
            {
                dialog.set_language(language);
            }
        });
        let weak = self.downgrade();
        imp.restart_button.connect_activated(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.restart();
            }
        });
        let weak = self.downgrade();
        imp.autostart_row.connect_active_notify(move |row| {
            if let Some(dialog) = weak.upgrade() {
                dialog.set_autostart(row.is_active());
            }
        });
        let weak = self.downgrade();
        imp.scan_button.connect_activated(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.scan_interfaces();
            }
        });
        let weak = self.downgrade();
        imp.test_button.connect_activated(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.test_connection();
            }
        });
        let weak = self.downgrade();
        imp.save_button.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.save();
            }
        });
        // When switching light / dark, color buttons that aren't customized switch to the theme default.
        let weak = self.downgrade();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.refresh();
            }
        });
    }

    /// Text field: filtered, then written to the draft.
    fn connect_text(
        &self,
        row: &impl IsA<gtk::Editable>,
        filter: fn(&str) -> String,
        apply: fn(&mut SettingsDraft, String),
    ) {
        let weak = self.downgrade();
        row.connect_changed(move |row| {
            let Some(dialog) = weak.upgrade() else { return };
            let text = row.text();
            let filtered = filter(&text);
            if filtered != text.as_str() {
                // Rewriting triggers changed again, and the draft is written then.
                row.set_text(&filtered);
                return;
            }
            dialog.edit(|draft| apply(draft, filtered));
        });
    }

    /// Changes the draft from user actions, then updates the UI.
    fn edit(&self, change: impl FnOnce(&mut SettingsDraft)) {
        let imp = self.imp();
        if imp.updating.get() {
            return;
        }
        if let Some(draft) = imp.draft.borrow_mut().as_mut() {
            change(draft);
        }
        self.refresh();
    }

    fn draft(&self) -> SettingsDraft {
        self.imp()
            .draft
            .borrow()
            .clone()
            .expect("draft is set at construction")
    }

    /// Doesn't trigger change handlers while the code sets widget values.
    fn without_events(&self, update: impl FnOnce()) {
        let imp = self.imp();
        let previous = imp.updating.replace(true);
        update();
        imp.updating.set(previous);
    }

    /// Fills in the fields from the draft (only when opening).
    fn populate(&self) {
        let imp = self.imp();
        let draft = self.draft();
        self.without_events(|| {
            imp.host_row.set_text(&draft.host);
            imp.port_row.set_text(&draft.port_text);
            imp.mode_group
                .set_active_name(Some(draft.security.mode.nick()));
            imp.auth_password_row
                .set_text(&draft.security.auth_password);
            imp.priv_password_row
                .set_text(&draft.security.priv_password);
            let index = AuthProtocol::ALL
                .iter()
                .position(|&p| p == draft.security.auth_protocol)
                .unwrap_or(1);
            imp.auth_protocol_row.set_selected(index as u32);
            imp.privacy_group
                .set_active_name(Some(priv_protocol_nick(draft.security.priv_protocol)));
            imp.interval_group
                .set_active_name(Some(&draft.interval_seconds.to_string()));
            imp.unit_group
                .set_active_name(Some(unit_style_nick(draft.unit_style)));
            imp.charts_row.set_active(draft.charts_when_multiple);
            imp.compact_row.set_active(draft.compact_window);
            imp.main_opacity_scale
                .set_value((draft.main_window_opacity * 100.0).round());
            imp.tray_opacity_scale
                .set_value((draft.tray_window_opacity * 100.0).round());
            imp.autostart_row.set_active(imp.autostart.is_enabled());
        });
        self.populate_language();
        self.populate_community();
    }

    fn populate_community(&self) {
        let imp = self.imp();
        let community = self.draft().security.community;
        self.without_events(|| {
            imp.community_row.set_text(&community);
            imp.user_row.set_text(&community);
        });
    }

    /// Updates the derived UI state from the draft.
    fn refresh(&self) {
        let imp = self.imp();
        let draft = self.draft();
        let mode = draft.security.mode;

        imp.community_row.set_visible(mode == SecurityMode::None);
        imp.user_row.set_visible(mode != SecurityMode::None);
        imp.auth_protocol_row
            .set_visible(mode != SecurityMode::None);
        imp.auth_password_row
            .set_visible(mode != SecurityMode::None);
        imp.privacy_row.set_visible(mode == SecurityMode::Private);
        imp.priv_password_row
            .set_visible(mode == SecurityMode::Private);
        imp.security_group
            .set_description(Some(&text::mode_explanation(mode)));
        let problem = draft.connection_problem().map(text::draft_problem);
        imp.problem_label
            .set_label(problem.as_deref().unwrap_or(""));
        imp.problem_label.set_visible(problem.is_some());

        let working = matches!(*imp.activity.borrow(), Some(Activity::Working(_)));
        imp.scan_button
            .set_sensitive(draft.can_connect() && !working);
        imp.test_button
            .set_sensitive(draft.can_connect() && !working);
        imp.spinner.set_visible(working);
        imp.save_button.set_sensitive(draft.can_save());

        self.refresh_interfaces(&draft);
        self.refresh_colors(&draft);
        self.refresh_activity();
    }

    fn refresh_interfaces(&self, draft: &SettingsDraft) {
        let imp = self.imp();
        let choices = draft.interface_choices();
        let names_changed = {
            let rows = imp.interface_rows.borrow();
            rows.len() != choices.len()
                || rows
                    .iter()
                    .zip(&choices)
                    .any(|((name, _, _), choice)| name != choice)
        };
        if names_changed {
            for (_, row, _) in imp.interface_rows.take() {
                imp.interfaces_group.remove(&row);
            }
            // Interface rows must come before the tray interface and buttons.
            for fixed in [
                imp.tray_row.upcast_ref::<gtk::Widget>(),
                imp.scan_button.upcast_ref(),
                imp.test_button.upcast_ref(),
            ] {
                imp.interfaces_group.remove(fixed);
            }
            let mut rows = Vec::new();
            for name in &choices {
                let check = gtk::CheckButton::new();
                check.set_valign(gtk::Align::Center);
                let row = adw::ActionRow::builder()
                    .title(name)
                    .use_markup(false)
                    .activatable_widget(&check)
                    .build();
                row.add_prefix(&check);
                let weak = self.downgrade();
                let interface = name.clone();
                check.connect_toggled(move |check| {
                    if let Some(dialog) = weak.upgrade() {
                        let selected = check.is_active();
                        dialog.edit(|draft| draft.set_selected(&interface, selected));
                    }
                });
                imp.interfaces_group.add(&row);
                rows.push((name.clone(), row, check));
            }
            for fixed in [
                imp.tray_row.upcast_ref::<gtk::Widget>(),
                imp.scan_button.upcast_ref(),
                imp.test_button.upcast_ref(),
            ] {
                imp.interfaces_group.add(fixed);
            }
            imp.interface_rows.replace(rows);
        }
        self.without_events(|| {
            for (name, _, check) in imp.interface_rows.borrow().iter() {
                check.set_active(draft.is_selected(name));
            }
            let selected = draft.interface_names();
            imp.tray_row.set_model(Some(&gtk::StringList::new(
                &selected.iter().map(String::as_str).collect::<Vec<_>>(),
            )));
            let index = selected
                .iter()
                .position(|n| *n == draft.tray_interface)
                .unwrap_or(0);
            imp.tray_row.set_selected(index as u32);
        });
        imp.no_interfaces_row.set_visible(choices.is_empty());
        // Choosing which interface the tray shows is only needed when monitoring two or more interfaces.
        imp.tray_row.set_visible(draft.interface_names().len() > 1);
    }

    fn refresh_colors(&self, draft: &SettingsDraft) {
        let imp = self.imp();
        let dark = adw::StyleManager::default().is_dark();
        let defaults = Palette::new(dark, None, None);
        self.without_events(|| {
            for (button, reset, custom, default) in [
                (
                    &imp.download_color,
                    &imp.download_reset,
                    &draft.download_color,
                    defaults.download,
                ),
                (
                    &imp.upload_color,
                    &imp.upload_reset,
                    &draft.upload_color,
                    defaults.upload,
                ),
            ] {
                let color = custom.as_deref().and_then(parse_hex);
                button.set_rgba(&color.unwrap_or(default));
                reset.set_visible(color.is_some());
            }
        });
    }

    fn set_activity(&self, activity: Activity) {
        self.imp().activity.replace(Some(activity));
        self.refresh();
    }

    fn refresh_activity(&self) {
        let imp = self.imp();
        let label = &imp.activity_label;
        for class in ["dim-label", "success", "error"] {
            label.remove_css_class(class);
        }
        let (text, class) = match imp.activity.borrow().clone().unwrap_or(Activity::Idle) {
            Activity::Idle => (gettext("Changes take effect when you save."), "dim-label"),
            Activity::Working(message) => (message, "dim-label"),
            Activity::Success(message) => (message, "success"),
            Activity::Failure(message) => (message, "error"),
        };
        label.set_label(&text);
        label.add_css_class(class);
    }

    /// Address, port and security settings when the draft can connect.
    fn connection(&self) -> Option<(String, u16, SnmpSecurity)> {
        let draft = self.draft();
        if !draft.can_connect() {
            return None;
        }
        Some((
            draft.device_host()?,
            draft.port()?,
            draft.security.snmp_security(),
        ))
    }

    /// Runs `work` in the background and applies the result to the UI when done. Starting new work cancels the previous one.
    fn run<F, Fut>(&self, progress: String, work: F)
    where
        F: FnOnce(SnmpClient<UdpTransport>) -> Fut + 'static,
        Fut: Future<Output = Result<Box<dyn FnOnce(&SettingsDialog) -> Activity>, SnmpError>>
            + 'static,
    {
        let Some((host, port, security)) = self.connection() else {
            return;
        };
        if let Some(task) = self.imp().task.take() {
            task.abort();
        }
        self.set_activity(Activity::Working(progress));
        let weak = self.downgrade();
        let task = glib::spawn_future_local(async move {
            let result = match UdpTransport::connect(&host, port).await {
                Ok(transport) => work(SnmpClient::new(transport, security)).await,
                Err(error) => Err(SnmpError::Transport(error.to_string())),
            };
            let Some(dialog) = weak.upgrade() else { return };
            let activity = match result {
                Ok(apply) => apply(&dialog),
                Err(error) => Activity::Failure(text::snmp_error(&error)),
            };
            dialog.set_activity(activity);
        });
        self.imp().task.replace(Some(task));
    }

    fn scan_interfaces(&self) {
        self.run(gettext("Scanning interfaces…"), |mut client| async move {
            let found: Vec<String> = client
                .list_interfaces()
                .await?
                .into_iter()
                .map(|i| i.name)
                .collect();
            Ok(Box::new(move |dialog: &SettingsDialog| {
                let count = found.len();
                dialog.edit(|draft| draft.apply_scan(found));
                if count == 0 {
                    Activity::Failure(gettext("No interfaces found"))
                } else {
                    // Translators: {} is the number of interfaces.
                    Activity::Success(gettext("Interfaces found: {}").replacen(
                        "{}",
                        &count.to_string(),
                        1,
                    ))
                }
            })
                as Box<dyn FnOnce(&SettingsDialog) -> Activity>)
        });
    }

    fn test_connection(&self) {
        let names = self.draft().interface_names().to_vec();
        self.run(gettext("Testing connection…"), |mut client| async move {
            let binds = client.get(std::slice::from_ref(&*SYS_UP_TIME)).await?;
            let mut message = gettext("Connected.");
            if let Some(SnmpValue::TimeTicks(ticks)) = binds.first().map(|b| &b.value) {
                let uptime = text::uptime(Duration::from_millis(u64::from(*ticks) * 10));
                // Translators: {} is an uptime such as "12 days, 5 hours".
                message += &format!(" {}", gettext("Device up {}.").replacen("{}", &uptime, 1));
            }
            let indexes = client.resolve_interface_indexes(&names).await?;
            for (name, index) in names.iter().zip(indexes) {
                message += &format!(" {}", text::interface_index(name, index));
            }
            Ok(
                Box::new(move |_: &SettingsDialog| Activity::Success(message))
                    as Box<dyn FnOnce(&SettingsDialog) -> Activity>,
            )
        });
    }

    fn save(&self) {
        let Some(settings) = self.draft().to_settings() else {
            return;
        };
        let gsettings = gio::Settings::new(config::APP_ID);
        gsettings.delay();
        if let Err(error) = settings.write(&gsettings) {
            gsettings.revert();
            self.set_activity(Activity::Failure(error.to_string()));
            return;
        }
        gsettings.apply();

        let model = self.imp().model.get().cloned();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let stored = save_secrets(&KeyringStore, &settings.security).await;
            if let Some(model) = model {
                model.apply_settings(settings);
            }
            let Some(dialog) = weak.upgrade() else { return };
            dialog.set_activity(match stored {
                Ok(()) => Activity::Success(gettext("Saved")),
                Err(error) => {
                    glib::g_warning!("anemobit", "{error}");
                    Activity::Failure(gettext(
                        "Could not save the community or passwords to the keyring",
                    ))
                }
            });
        });
    }

    /// The stored language (re-read each time Settings opens) and whether a restart is needed.
    fn populate_language(&self) {
        let imp = self.imp();
        let language = AppLanguage::read(&gio::Settings::new(config::APP_ID));
        let index = AppLanguage::ALL
            .iter()
            .position(|&l| l == language)
            .unwrap_or(0);
        self.without_events(|| imp.language_row.set_selected(index as u32));
        imp.restart_button
            .set_visible(language != language::launch_language());
    }

    /// The display language is saved immediately (not with Save) and applies after a restart;
    /// on failure, the selection is restored and the reason shown.
    fn set_language(&self, language: AppLanguage) {
        let imp = self.imp();
        if imp.updating.get() {
            return;
        }
        match language.write(&gio::Settings::new(config::APP_ID)) {
            Ok(()) => imp.language_error.set_visible(false),
            Err(error) => {
                // Translators: {} is a system error message.
                let message = gettext("Could not save the language setting: {}").replacen(
                    "{}",
                    &error.to_string(),
                    1,
                );
                imp.language_error.set_label(&message);
                imp.language_error.set_visible(true);
            }
        }
        self.populate_language();
    }

    /// Restart Now: starts a new instance in the chosen language, then quits.
    fn restart(&self) {
        let imp = self.imp();
        match language::relaunch() {
            Ok(()) => {
                if let Some(app) = gio::Application::default() {
                    app.quit();
                }
            }
            Err(error) => {
                // Translators: {} is a system error message.
                let message =
                    gettext("Could not restart Anemobit: {}").replacen("{}", &error.to_string(), 1);
                imp.language_error.set_label(&message);
                imp.language_error.set_visible(true);
            }
        }
    }

    /// Open at Login applies immediately; on failure, the toggle is restored and the reason shown.
    fn set_autostart(&self, enabled: bool) {
        let imp = self.imp();
        if imp.updating.get() {
            return;
        }
        let result =
            std::env::current_exe().and_then(|exe| imp.autostart.set_enabled(enabled, &exe));
        match result {
            Ok(()) => imp.autostart_error.set_visible(false),
            Err(error) => {
                // Translators: {} is a system error message.
                let message = gettext("Could not change the startup setting: {}").replacen(
                    "{}",
                    &error.to_string(),
                    1,
                );
                imp.autostart_error.set_label(&message);
                imp.autostart_error.set_visible(true);
                self.without_events(|| imp.autostart_row.set_active(imp.autostart.is_enabled()));
            }
        }
    }
}
