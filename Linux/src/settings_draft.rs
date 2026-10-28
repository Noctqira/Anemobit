//! Draft for the Settings window (the data part of SettingsViewModel on macOS). Only Save writes GSettings and the keyring.

use anemobit_core::formatting::{DeviceAddress, RateUnitStyle};

use crate::settings::{AppSettings, SecurityProblem, SecuritySettings};

/// Why the draft can't connect (UI text translated by [`crate::text`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftProblem {
    MissingHost,
    InvalidHost,
    InvalidPort,
    Security(SecurityProblem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsDraft {
    pub host: String,
    pub port_text: String,
    pub security: SecuritySettings,
    /// Selected interfaces, in [`SettingsDraft::interface_choices`] order.
    interface_names: Vec<String>,
    pub tray_interface: String,
    pub interval_seconds: u32,
    pub unit_style: RateUnitStyle,
    pub charts_when_multiple: bool,
    pub download_color: Option<String>,
    pub upload_color: Option<String>,
    pub compact_window: bool,
    pub main_window_opacity: f64,
    pub tray_window_opacity: f64,
    /// Scanned interface names (in the order the device returned them).
    scanned: Vec<String>,
}

impl SettingsDraft {
    pub fn new(settings: &AppSettings) -> Self {
        SettingsDraft {
            host: settings.host.clone(),
            port_text: settings.port.to_string(),
            security: settings.security.clone(),
            interface_names: settings.interface_names.clone(),
            tray_interface: settings
                .effective_tray_interface()
                .unwrap_or_default()
                .to_owned(),
            interval_seconds: settings.interval_seconds,
            unit_style: settings.unit_style,
            charts_when_multiple: settings.charts_when_multiple,
            download_color: settings.download_color.clone(),
            upload_color: settings.upload_color.clone(),
            compact_window: settings.compact_window,
            main_window_opacity: settings.main_window_opacity,
            tray_window_opacity: settings.tray_window_opacity,
            scanned: Vec::new(),
        }
    }

    pub fn interface_names(&self) -> &[String] {
        &self.interface_names
    }

    /// 1–65535; anything else (including empty or 0) is `None`.
    pub fn port(&self) -> Option<u16> {
        self.port_text
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|&port| port != 0)
    }

    /// Host to connect to: IPv4, IPv6 (without brackets), or a host name; `None` when the format is invalid.
    pub fn device_host(&self) -> Option<String> {
        DeviceAddress::parse(&self.host).map(|address| address.host().to_owned())
    }

    /// Why it can't connect right now; `None` when it can.
    pub fn connection_problem(&self) -> Option<DraftProblem> {
        if self.host.trim().is_empty() {
            return Some(DraftProblem::MissingHost);
        }
        if self.device_host().is_none() {
            return Some(DraftProblem::InvalidHost);
        }
        if self.port().is_none() {
            return Some(DraftProblem::InvalidPort);
        }
        self.security.problem().map(DraftProblem::Security)
    }

    pub fn can_connect(&self) -> bool {
        self.connection_problem().is_none()
    }

    pub fn can_save(&self) -> bool {
        self.can_connect() && !self.interface_names.is_empty()
    }

    /// Interface list: names that are selected but not in the scan results (e.g. before scanning), plus the scan results.
    pub fn interface_choices(&self) -> Vec<String> {
        self.interface_names
            .iter()
            .filter(|name| !self.scanned.contains(name))
            .chain(&self.scanned)
            .cloned()
            .collect()
    }

    pub fn is_selected(&self, name: &str) -> bool {
        self.interface_names.iter().any(|n| n == name)
    }

    /// Selects / deselects an interface, keeping the list order; the tray interface must be one of the selected ones.
    pub fn set_selected(&mut self, name: &str, selected: bool) {
        let mut names = self.interface_names.clone();
        if selected {
            if !names.iter().any(|n| n == name) {
                names.push(name.to_owned());
            }
        } else {
            names.retain(|n| n != name);
        }
        let order = self.interface_choices();
        let mut ordered: Vec<String> = order
            .iter()
            .filter(|n| names.contains(n))
            .cloned()
            .collect();
        ordered.extend(names.into_iter().filter(|n| !order.contains(n)));
        self.interface_names = ordered;
        if !self.is_selected(&self.tray_interface) {
            self.tray_interface = self.interface_names.first().cloned().unwrap_or_default();
        }
    }

    /// Applies scan results; if no interface is selected yet, selects the first pppoe-out automatically (a common RouterOS WAN).
    pub fn apply_scan(&mut self, found: Vec<String>) {
        self.scanned = found;
        if self.interface_names.is_empty()
            && let Some(wan) = self
                .scanned
                .iter()
                .find(|name| name.starts_with("pppoe-out"))
                .cloned()
        {
            self.set_selected(&wan, true);
        }
    }

    /// Settings to save; `None` when they can't be saved.
    pub fn to_settings(&self) -> Option<AppSettings> {
        if !self.can_save() {
            return None;
        }
        Some(AppSettings {
            host: self.device_host()?,
            port: self.port()?,
            interface_names: self.interface_names.clone(),
            tray_interface: self.tray_interface.clone(),
            security: self.security.clone(),
            interval_seconds: self.interval_seconds,
            unit_style: self.unit_style,
            charts_when_multiple: self.charts_when_multiple,
            download_color: self.download_color.clone(),
            upload_color: self.upload_color.clone(),
            compact_window: self.compact_window,
            main_window_opacity: self.main_window_opacity,
            tray_window_opacity: self.tray_window_opacity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SecurityMode;
    use anemobit_core::UsmFailure;

    fn draft() -> SettingsDraft {
        let mut draft = SettingsDraft::new(&AppSettings::default());
        draft.host = "192.168.88.1".into();
        draft.security.community = "public".into();
        draft
    }

    #[test]
    fn starts_from_saved_settings() {
        let settings = AppSettings {
            host: "router.lan".into(),
            port: 1161,
            interface_names: vec!["ether1".into(), "pppoe-out1".into()],
            tray_interface: "wlan9".into(),
            interval_seconds: 5,
            ..Default::default()
        };
        let draft = SettingsDraft::new(&settings);
        assert_eq!(draft.host, "router.lan");
        assert_eq!(draft.port_text, "1161");
        assert_eq!(draft.interface_names(), ["ether1", "pppoe-out1"]);
        // When the tray interface isn't in the list, use the first one actually shown.
        assert_eq!(draft.tray_interface, "ether1");
        assert_eq!(draft.interval_seconds, 5);
    }

    #[test]
    fn port_must_be_1_to_65535() {
        let mut draft = draft();
        for (text, port) in [
            ("161", Some(161)),
            (" 1161 ", Some(1161)),
            ("65535", Some(65_535)),
            ("0", None),
            ("65536", None),
            ("", None),
            ("abc", None),
        ] {
            draft.port_text = text.into();
            assert_eq!(draft.port(), port, "{text:?}");
        }
    }

    #[test]
    fn connection_problems_in_order() {
        let mut draft = draft();
        assert_eq!(draft.connection_problem(), None);
        draft.security.community.clear();
        assert_eq!(
            draft.connection_problem(),
            Some(DraftProblem::Security(SecurityProblem::MissingCommunity))
        );
        draft.port_text = "0".into();
        assert_eq!(draft.connection_problem(), Some(DraftProblem::InvalidPort));
        draft.host = "999.1.1.1".into();
        assert_eq!(draft.connection_problem(), Some(DraftProblem::InvalidHost));
        draft.host = "  ".into();
        assert_eq!(draft.connection_problem(), Some(DraftProblem::MissingHost));
    }

    #[test]
    fn security_problems_come_from_the_credentials() {
        let mut draft = draft();
        draft.security.mode = SecurityMode::Private;
        draft.security.auth_password = "authpass12".into();
        draft.security.priv_password = "short".into();
        assert_eq!(
            draft.connection_problem(),
            Some(DraftProblem::Security(SecurityProblem::Usm(
                UsmFailure::PrivPasswordTooShort { minimum: 8 }
            )))
        );
    }

    #[test]
    fn saving_needs_an_interface() {
        let mut draft = draft();
        assert!(draft.can_connect());
        assert!(!draft.can_save());
        draft.set_selected("ether1", true);
        assert!(draft.can_save());
    }

    #[test]
    fn choices_list_unscanned_selections_first() {
        let mut draft = draft();
        draft.set_selected("wlan9", true);
        draft.apply_scan(vec!["ether1".into(), "pppoe-out1".into(), "bridge".into()]);
        assert_eq!(
            draft.interface_choices(),
            ["wlan9", "ether1", "pppoe-out1", "bridge"]
        );
    }

    #[test]
    fn selection_follows_list_order() {
        let mut draft = draft();
        draft.apply_scan(vec!["ether1".into(), "ether2".into(), "pppoe-out1".into()]);
        draft.set_selected("ether1", false);
        draft.set_selected("pppoe-out1", true);
        draft.set_selected("ether1", true);
        assert_eq!(draft.interface_names(), ["ether1", "pppoe-out1"]);
        draft.set_selected("pppoe-out1", true); // Selecting twice doesn't add another entry
        assert_eq!(draft.interface_names(), ["ether1", "pppoe-out1"]);
    }

    #[test]
    fn tray_interface_stays_among_selected() {
        let mut draft = draft();
        draft.apply_scan(vec!["ether1".into(), "pppoe-out1".into()]);
        draft.set_selected("ether1", true);
        assert_eq!(draft.tray_interface, "pppoe-out1"); // WAN selected automatically during the scan
        draft.tray_interface = "ether1".into();
        draft.set_selected("ether1", false);
        assert_eq!(draft.tray_interface, "pppoe-out1");
        draft.set_selected("pppoe-out1", false);
        assert_eq!(draft.tray_interface, "");
    }

    #[test]
    fn scan_selects_the_first_pppoe_only_when_nothing_is_selected() {
        let mut draft = draft();
        draft.apply_scan(vec![
            "ether1".into(),
            "pppoe-out1".into(),
            "pppoe-out2".into(),
        ]);
        assert_eq!(draft.interface_names(), ["pppoe-out1"]);

        let mut selected = draft.clone();
        selected.set_selected("pppoe-out1", false);
        selected.set_selected("ether1", true);
        selected.apply_scan(vec!["ether1".into(), "pppoe-out1".into()]);
        assert_eq!(selected.interface_names(), ["ether1"]);
    }

    #[test]
    fn to_settings_normalizes_the_host() {
        let mut draft = draft();
        assert_eq!(draft.to_settings(), None); // No interface selected yet
        draft.host = " [2001:db8::1] ".into();
        draft.port_text = "1161".into();
        draft.set_selected("ether1", true);
        draft.unit_style = RateUnitStyle::Bytes;
        draft.download_color = Some("#102030".into());
        draft.compact_window = true;
        draft.main_window_opacity = 0.4;
        let settings = draft.to_settings().unwrap();
        assert!(settings.compact_window);
        assert_eq!(settings.main_window_opacity, 0.4);
        assert_eq!(settings.tray_window_opacity, 0.7);
        assert_eq!(settings.host, "2001:db8::1");
        assert_eq!(settings.port, 1161);
        assert_eq!(settings.interface_names, ["ether1"]);
        assert_eq!(settings.tray_interface, "ether1");
        assert_eq!(settings.unit_style, RateUnitStyle::Bytes);
        assert_eq!(settings.download_color.as_deref(), Some("#102030"));
        assert_eq!(settings.security.community, "public");
    }
}
