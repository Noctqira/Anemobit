//! User settings. Regular settings live in GSettings; the community and passwords in the Secret Service (see [`crate::secrets`]).

use std::fmt;
use std::time::Duration;

use anemobit_core::formatting::RateUnitStyle;
use anemobit_core::traffic::PollerConfiguration;
use anemobit_core::{
    AuthProtocol, PrivProtocol, SnmpSecurity, UsmCredentials, UsmFailure, UsmPrivacy,
};
use gtk::prelude::*;
use gtk::{gio, glib};

/// Security mode. GSettings stores RouterOS `/snmp community` security names (none / authorized /
/// private); the UI shows None / Auth / Priv (not translated).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SecurityMode {
    /// SNMPv2c, community only.
    #[default]
    None,
    /// SNMPv3 authNoPriv: authentication only.
    Authorized,
    /// SNMPv3 authPriv: authentication plus privacy.
    Private,
}

impl SecurityMode {
    pub const ALL: [SecurityMode; 3] = [Self::None, Self::Authorized, Self::Private];

    /// Name in GSettings.
    pub fn nick(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Authorized => "authorized",
            Self::Private => "private",
        }
    }

    pub fn from_nick(nick: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.nick() == nick)
    }

    /// Option name in Settings (not translated).
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Authorized => "Auth",
            Self::Private => "Priv",
        }
    }
}

/// Authentication protocol name in GSettings.
pub fn auth_protocol_nick(protocol: AuthProtocol) -> &'static str {
    match protocol {
        AuthProtocol::Md5 => "md5",
        AuthProtocol::Sha1 => "sha1",
        AuthProtocol::Sha224 => "sha224",
        AuthProtocol::Sha256 => "sha256",
        AuthProtocol::Sha384 => "sha384",
        AuthProtocol::Sha512 => "sha512",
    }
}

pub fn auth_protocol_from_nick(nick: &str) -> Option<AuthProtocol> {
    AuthProtocol::ALL
        .into_iter()
        .find(|&p| auth_protocol_nick(p) == nick)
}

/// Display name of the authentication protocol (matches RouterOS's authentication-protocol spelling).
pub fn auth_protocol_label(protocol: AuthProtocol) -> &'static str {
    match protocol {
        AuthProtocol::Md5 => "MD5",
        AuthProtocol::Sha1 => "SHA1",
        AuthProtocol::Sha224 => "SHA224",
        AuthProtocol::Sha256 => "SHA256",
        AuthProtocol::Sha384 => "SHA384",
        AuthProtocol::Sha512 => "SHA512",
    }
}

pub fn priv_protocol_nick(protocol: PrivProtocol) -> &'static str {
    match protocol {
        PrivProtocol::Des => "des",
        PrivProtocol::Aes => "aes",
    }
}

pub fn priv_protocol_from_nick(nick: &str) -> Option<PrivProtocol> {
    [PrivProtocol::Des, PrivProtocol::Aes]
        .into_iter()
        .find(|&p| priv_protocol_nick(p) == nick)
}

pub fn unit_style_nick(style: RateUnitStyle) -> &'static str {
    match style {
        RateUnitStyle::Bits => "bits",
        RateUnitStyle::Bytes => "bytes",
    }
}

pub fn unit_style_from_nick(nick: &str) -> Option<RateUnitStyle> {
    [RateUnitStyle::Bits, RateUnitStyle::Bytes]
        .into_iter()
        .find(|&s| unit_style_nick(s) == nick)
}

/// Connection security settings (shared by the Settings draft and the saved settings).
#[derive(Clone, PartialEq, Eq)]
pub struct SecuritySettings {
    pub mode: SecurityMode,
    /// v2c community; v3 user name (the RouterOS community name).
    pub community: String,
    pub auth_protocol: AuthProtocol,
    pub auth_password: String,
    pub priv_protocol: PrivProtocol,
    pub priv_password: String,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        SecuritySettings {
            mode: SecurityMode::None,
            community: String::new(),
            auth_protocol: AuthProtocol::Sha1,
            auth_password: String::new(),
            priv_protocol: PrivProtocol::Aes,
            priv_password: String::new(),
        }
    }
}

// The community and passwords must never appear in logs.
impl fmt::Debug for SecuritySettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecuritySettings")
            .field("mode", &self.mode)
            .field("auth_protocol", &self.auth_protocol)
            .field("priv_protocol", &self.priv_protocol)
            .finish_non_exhaustive()
    }
}

/// Why the security settings are incomplete or invalid (the app translates UI text from this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityProblem {
    MissingCommunity,
    MissingUserName,
    Usm(UsmFailure),
}

impl SecuritySettings {
    pub fn snmp_security(&self) -> SnmpSecurity {
        let privacy = match self.mode {
            SecurityMode::None => return SnmpSecurity::Community(self.community.clone()),
            SecurityMode::Authorized => None,
            SecurityMode::Private => Some(UsmPrivacy {
                protocol: self.priv_protocol,
                password: self.priv_password.clone(),
            }),
        };
        SnmpSecurity::Usm(UsmCredentials {
            user_name: self.community.clone(),
            auth_protocol: self.auth_protocol,
            auth_password: self.auth_password.clone(),
            privacy,
        })
    }

    /// Why fields are incomplete or invalid; `None` when ready to connect.
    pub fn problem(&self) -> Option<SecurityProblem> {
        if self.community.is_empty() {
            return Some(match self.mode {
                SecurityMode::None => SecurityProblem::MissingCommunity,
                _ => SecurityProblem::MissingUserName,
            });
        }
        match self.snmp_security() {
            SnmpSecurity::Usm(credentials) => {
                credentials.validation_failure().map(SecurityProblem::Usm)
            }
            SnmpSecurity::Community(_) => None,
        }
    }
}

/// Snapshot of the user settings.
#[derive(Debug, Clone, PartialEq)]
pub struct AppSettings {
    pub host: String,
    pub port: u16,
    /// Monitored interfaces (ifName), in the order shown in Settings.
    pub interface_names: Vec<String>,
    /// Interface shown in the system tray and compact window; read it through [`AppSettings::effective_tray_interface`].
    pub tray_interface: String,
    pub security: SecuritySettings,
    pub interval_seconds: u32,
    pub unit_style: RateUnitStyle,
    /// Whether interface cards show charts when monitoring two or more interfaces (always shown for a single interface).
    pub charts_when_multiple: bool,
    /// Custom download color (`#RRGGBB`); `None` means the theme default.
    pub download_color: Option<String>,
    pub upload_color: Option<String>,
    /// Compact window (off by default).
    pub compact_window: bool,
    /// Main window background opacity (0–1, default 0.7; same as the Windows floating window).
    pub main_window_opacity: f64,
    /// System tray popup background opacity (0–1, default 0.7).
    pub tray_window_opacity: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            host: String::new(),
            port: PollerConfiguration::DEFAULT_PORT,
            interface_names: Vec::new(),
            tray_interface: String::new(),
            security: SecuritySettings::default(),
            interval_seconds: 1,
            unit_style: RateUnitStyle::Bits,
            charts_when_multiple: false,
            download_color: None,
            upload_color: None,
            compact_window: false,
            main_window_opacity: Self::DEFAULT_OPACITY,
            tray_window_opacity: Self::DEFAULT_OPACITY,
        }
    }
}

impl AppSettings {
    /// Default background opacity (semi-transparent).
    pub const DEFAULT_OPACITY: f64 = 0.7;

    /// Refresh interval options (seconds).
    pub const REFRESH_INTERVALS: [u32; 3] = [1, 5, 10];

    /// Reads GSettings (`settings` is the top-level schema). The community and passwords are left empty and filled in by
    /// [`crate::secrets::load_secrets`]. Invalid values fall back to the defaults.
    pub fn read(settings: &gio::Settings) -> Self {
        let device = settings.child("device");
        let snmp = settings.child("snmp");
        let display = settings.child("display");
        let defaults = AppSettings::default();
        let color = |key: &str| Some(display.string(key).to_string()).filter(|c| !c.is_empty());
        let interval = settings.child("poll").uint("interval");
        AppSettings {
            host: device.string("host").into(),
            port: device.value("port").get().unwrap_or(defaults.port),
            interface_names: device
                .strv("interface-names")
                .iter()
                .map(|s| s.to_string())
                .collect(),
            tray_interface: display.string("tray-interface").into(),
            security: SecuritySettings {
                mode: SecurityMode::from_nick(&snmp.string("security-mode")).unwrap_or_default(),
                auth_protocol: auth_protocol_from_nick(&snmp.string("auth-protocol"))
                    .unwrap_or(defaults.security.auth_protocol),
                priv_protocol: priv_protocol_from_nick(&snmp.string("priv-protocol"))
                    .unwrap_or(defaults.security.priv_protocol),
                ..SecuritySettings::default()
            },
            interval_seconds: if Self::REFRESH_INTERVALS.contains(&interval) {
                interval
            } else {
                defaults.interval_seconds
            },
            unit_style: unit_style_from_nick(&display.string("unit-style")).unwrap_or_default(),
            charts_when_multiple: display.boolean("charts-when-multiple"),
            download_color: color("download-color"),
            upload_color: color("upload-color"),
            compact_window: settings.child("compact-window").boolean("visible"),
            main_window_opacity: display.double("main-window-opacity").clamp(0.0, 1.0),
            tray_window_opacity: display.double("tray-window-opacity").clamp(0.0, 1.0),
        }
    }

    /// Writes GSettings. The caller calls `delay()` first and `apply()` after writing, so all keys take effect at once.
    pub fn write(&self, settings: &gio::Settings) -> Result<(), glib::BoolError> {
        let device = settings.child("device");
        device.set_string("host", &self.host)?;
        device.set_value("port", &self.port.to_variant())?;
        device.set_strv(
            "interface-names",
            self.interface_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )?;
        let snmp = settings.child("snmp");
        snmp.set_string("security-mode", self.security.mode.nick())?;
        snmp.set_string(
            "auth-protocol",
            auth_protocol_nick(self.security.auth_protocol),
        )?;
        snmp.set_string(
            "priv-protocol",
            priv_protocol_nick(self.security.priv_protocol),
        )?;
        settings
            .child("poll")
            .set_uint("interval", self.interval_seconds)?;
        let display = settings.child("display");
        display.set_string("tray-interface", &self.tray_interface)?;
        display.set_string("unit-style", unit_style_nick(self.unit_style))?;
        display.set_boolean("charts-when-multiple", self.charts_when_multiple)?;
        display.set_string(
            "download-color",
            self.download_color.as_deref().unwrap_or_default(),
        )?;
        display.set_string(
            "upload-color",
            self.upload_color.as_deref().unwrap_or_default(),
        )?;
        display.set_double(
            "main-window-opacity",
            self.main_window_opacity.clamp(0.0, 1.0),
        )?;
        display.set_double(
            "tray-window-opacity",
            self.tray_window_opacity.clamp(0.0, 1.0),
        )?;
        settings
            .child("compact-window")
            .set_boolean("visible", self.compact_window)
    }

    pub fn is_configured(&self) -> bool {
        !self.host.is_empty()
            && !self.interface_names.is_empty()
            && self.security.problem().is_none()
    }

    /// Interface actually shown in the system tray: falls back to the first one when the chosen interface isn't monitored.
    pub fn effective_tray_interface(&self) -> Option<&str> {
        if self.interface_names.contains(&self.tray_interface) {
            Some(&self.tray_interface)
        } else {
            self.interface_names.first().map(String::as_str)
        }
    }

    /// Polling settings when the settings are complete.
    pub fn poller_configuration(&self) -> Option<PollerConfiguration> {
        self.is_configured().then(|| {
            PollerConfiguration::new(
                &self.host,
                self.security.snmp_security(),
                &self.interface_names,
            )
            .with_port(self.port)
            .with_interval(Duration::from_secs(self.interval_seconds.into()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    /// GSettings with a memory backend (never reads or writes the user's dconf).
    fn memory_settings() -> gio::Settings {
        gio::Settings::with_backend(config::APP_ID, &gio::memory_settings_backend_new())
    }

    fn configured() -> AppSettings {
        AppSettings {
            host: "192.168.88.1".into(),
            interface_names: vec!["pppoe-out1".into(), "ether1".into()],
            security: SecuritySettings {
                community: "public".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn nicks_round_trip_and_match_the_schema() {
        let settings = memory_settings();
        let snmp = settings.child("snmp");
        for mode in SecurityMode::ALL {
            assert_eq!(SecurityMode::from_nick(mode.nick()), Some(mode));
            snmp.set_string("security-mode", mode.nick()).unwrap();
        }
        for protocol in AuthProtocol::ALL {
            assert_eq!(
                auth_protocol_from_nick(auth_protocol_nick(protocol)),
                Some(protocol)
            );
            snmp.set_string("auth-protocol", auth_protocol_nick(protocol))
                .unwrap();
        }
        for protocol in [PrivProtocol::Des, PrivProtocol::Aes] {
            assert_eq!(
                priv_protocol_from_nick(priv_protocol_nick(protocol)),
                Some(protocol)
            );
            snmp.set_string("priv-protocol", priv_protocol_nick(protocol))
                .unwrap();
        }
        let display = settings.child("display");
        for style in [RateUnitStyle::Bits, RateUnitStyle::Bytes] {
            assert_eq!(unit_style_from_nick(unit_style_nick(style)), Some(style));
            display
                .set_string("unit-style", unit_style_nick(style))
                .unwrap();
        }
        assert_eq!(SecurityMode::from_nick("nope"), None);
        assert_eq!(auth_protocol_from_nick("sha3"), None);
    }

    #[test]
    fn labels_are_untranslated_names() {
        let modes: Vec<_> = SecurityMode::ALL.iter().map(|m| m.label()).collect();
        assert_eq!(modes, ["None", "Auth", "Priv"]);
        let auth: Vec<_> = AuthProtocol::ALL
            .iter()
            .map(|&p| auth_protocol_label(p))
            .collect();
        assert_eq!(
            auth,
            ["MD5", "SHA1", "SHA224", "SHA256", "SHA384", "SHA512"]
        );
    }

    #[test]
    fn read_defaults() {
        let settings = AppSettings::read(&memory_settings());
        assert_eq!(settings, AppSettings::default());
        assert_eq!(settings.port, 161);
        assert_eq!(settings.main_window_opacity, 0.7);
        assert_eq!(settings.tray_window_opacity, 0.7);
        assert_eq!(settings.security.auth_protocol, AuthProtocol::Sha1);
        assert_eq!(settings.security.priv_protocol, PrivProtocol::Aes);
        assert!(!settings.is_configured());
    }

    #[test]
    fn read_stored_values() {
        let gsettings = memory_settings();
        let device = gsettings.child("device");
        device.set_string("host", "router.lan").unwrap();
        device.set_value("port", &1161u16.to_variant()).unwrap();
        device
            .set_strv("interface-names", ["pppoe-out1", "ether1"])
            .unwrap();
        let snmp = gsettings.child("snmp");
        snmp.set_string("security-mode", "private").unwrap();
        snmp.set_string("auth-protocol", "sha224").unwrap();
        snmp.set_string("priv-protocol", "des").unwrap();
        gsettings.child("poll").set_uint("interval", 5).unwrap();
        let display = gsettings.child("display");
        display.set_string("tray-interface", "ether1").unwrap();
        display.set_string("unit-style", "bytes").unwrap();
        display.set_boolean("charts-when-multiple", true).unwrap();
        display.set_string("download-color", "#1E88E5").unwrap();

        let settings = AppSettings::read(&gsettings);
        assert_eq!(settings.host, "router.lan");
        assert_eq!(settings.port, 1161);
        assert_eq!(settings.interface_names, ["pppoe-out1", "ether1"]);
        assert_eq!(settings.security.mode, SecurityMode::Private);
        assert_eq!(settings.security.auth_protocol, AuthProtocol::Sha224);
        assert_eq!(settings.security.priv_protocol, PrivProtocol::Des);
        assert_eq!(settings.interval_seconds, 5);
        assert_eq!(settings.tray_interface, "ether1");
        assert_eq!(settings.unit_style, RateUnitStyle::Bytes);
        assert!(settings.charts_when_multiple);
        assert_eq!(settings.download_color.as_deref(), Some("#1E88E5"));
        assert_eq!(settings.upload_color, None);
        // Passwords aren't in GSettings.
        assert!(settings.security.community.is_empty());
    }

    #[test]
    fn write_then_read_round_trips() {
        // Read back with another GSettings object to confirm the writes really took effect after apply() (including child schemas).
        let backend = gio::memory_settings_backend_new();
        let gsettings = gio::Settings::with_backend(config::APP_ID, &backend);
        let reader = gio::Settings::with_backend(config::APP_ID, &backend);
        let saved = AppSettings {
            host: "2001:db8::1".into(),
            port: 1161,
            interface_names: vec!["pppoe-out1".into(), "ether1".into()],
            tray_interface: "ether1".into(),
            security: SecuritySettings {
                mode: SecurityMode::Authorized,
                auth_protocol: AuthProtocol::Sha512,
                priv_protocol: PrivProtocol::Des,
                ..Default::default()
            },
            interval_seconds: 10,
            unit_style: RateUnitStyle::Bytes,
            charts_when_multiple: true,
            download_color: Some("#102030".into()),
            upload_color: None,
            compact_window: true,
            main_window_opacity: 0.25,
            tray_window_opacity: 1.0,
        };
        gsettings.delay();
        saved.write(&gsettings).unwrap();
        assert_eq!(
            AppSettings::read(&reader).host,
            "",
            "in delay() mode, writes take effect only after apply()"
        );
        gsettings.apply();
        assert_eq!(AppSettings::read(&reader), saved);
        // Passwords aren't written to GSettings.
        let with_secret = AppSettings {
            security: SecuritySettings {
                community: "s3cret".into(),
                ..saved.security.clone()
            },
            ..saved.clone()
        };
        with_secret.write(&gsettings).unwrap();
        gsettings.apply();
        assert_eq!(AppSettings::read(&reader).security.community, "");
    }

    #[test]
    fn unsupported_interval_falls_back_to_one_second() {
        let gsettings = memory_settings();
        gsettings.child("poll").set_uint("interval", 3).unwrap();
        assert_eq!(AppSettings::read(&gsettings).interval_seconds, 1);
    }

    #[test]
    fn effective_tray_interface_falls_back_to_first() {
        let mut settings = configured();
        assert_eq!(settings.effective_tray_interface(), Some("pppoe-out1"));
        settings.tray_interface = "ether1".into();
        assert_eq!(settings.effective_tray_interface(), Some("ether1"));
        settings.tray_interface = "wlan1".into();
        assert_eq!(settings.effective_tray_interface(), Some("pppoe-out1"));
        settings.interface_names.clear();
        assert_eq!(settings.effective_tray_interface(), None);
    }

    #[test]
    fn configured_settings_give_a_poller_configuration() {
        let mut settings = configured();
        settings.port = 1161;
        settings.interval_seconds = 10;
        let configuration = settings.poller_configuration().unwrap();
        assert_eq!(configuration.host(), "192.168.88.1");
        assert_eq!(configuration.port(), 1161);
        assert_eq!(configuration.interface_names(), ["pppoe-out1", "ether1"]);
        assert_eq!(
            configuration,
            configuration.clone().with_interval(Duration::from_secs(10))
        );
    }

    #[test]
    fn incomplete_settings_are_not_configured() {
        let mut no_host = configured();
        no_host.host.clear();
        let mut no_interfaces = configured();
        no_interfaces.interface_names.clear();
        let mut no_community = configured();
        no_community.security.community.clear();
        for settings in [no_host, no_interfaces, no_community] {
            assert!(!settings.is_configured());
            assert_eq!(settings.poller_configuration(), None);
        }
    }

    #[test]
    fn security_problems() {
        let mut security = SecuritySettings::default();
        assert_eq!(security.problem(), Some(SecurityProblem::MissingCommunity));
        security.mode = SecurityMode::Authorized;
        assert_eq!(security.problem(), Some(SecurityProblem::MissingUserName));
        security.community = "anemobit".into();
        security.auth_password = "short".into();
        assert_eq!(
            security.problem(),
            Some(SecurityProblem::Usm(UsmFailure::AuthPasswordTooShort {
                minimum: 8
            }))
        );
        security.auth_password = "authpass12".into();
        assert_eq!(security.problem(), None);
        security.mode = SecurityMode::Private;
        assert_eq!(
            security.problem(),
            Some(SecurityProblem::Usm(UsmFailure::PrivPasswordTooShort {
                minimum: 8
            }))
        );
        security.priv_password = "privpass12".into();
        assert_eq!(security.problem(), None);
    }

    #[test]
    fn snmp_security_follows_mode() {
        let security = SecuritySettings {
            mode: SecurityMode::None,
            community: "public".into(),
            auth_password: "authpass12".into(),
            priv_password: "privpass12".into(),
            ..Default::default()
        };
        assert_eq!(
            security.snmp_security(),
            SnmpSecurity::Community("public".into())
        );

        let authorized = SecuritySettings {
            mode: SecurityMode::Authorized,
            ..security.clone()
        };
        let SnmpSecurity::Usm(credentials) = authorized.snmp_security() else {
            panic!("USM expected")
        };
        assert_eq!(credentials.user_name, "public");
        assert_eq!(credentials.auth_protocol, AuthProtocol::Sha1);
        assert_eq!(credentials.auth_password, "authpass12");
        assert_eq!(credentials.privacy, None);

        let private = SecuritySettings {
            mode: SecurityMode::Private,
            priv_protocol: PrivProtocol::Des,
            ..security
        };
        let SnmpSecurity::Usm(credentials) = private.snmp_security() else {
            panic!("USM expected")
        };
        assert_eq!(
            credentials.privacy,
            Some(UsmPrivacy {
                protocol: PrivProtocol::Des,
                password: "privpass12".into()
            })
        );
    }

    #[test]
    fn debug_does_not_reveal_secrets() {
        let security = SecuritySettings {
            community: "s3cret-community".into(),
            auth_password: "authpass12".into(),
            priv_password: "privpass12".into(),
            ..Default::default()
        };
        let text = format!(
            "{:?}",
            AppSettings {
                security,
                ..Default::default()
            }
        );
        for secret in ["s3cret-community", "authpass12", "privpass12"] {
            assert!(!text.contains(secret), "{text}");
        }
    }
}
