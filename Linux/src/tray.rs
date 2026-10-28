//! System tray (StatusNotifierItem), implemented with GLib's built-in D-Bus (ksni doesn't support Ubuntu AppIndicator's
//! text label XAyatanaLabel).

pub mod menu;
pub mod status_notifier;

use anemobit_core::formatting::rate_units;
use gettextrs::gettext;

use crate::settings::AppSettings;
use crate::text;
use crate::traffic_model::{ConnectionState, TrafficState};
use menu::MenuEntry;
use status_notifier::TrayContent;

/// Menu item ids (0 is reserved for the root).
pub const MENU_OPEN_WINDOW: i32 = 1;
pub const MENU_SETTINGS: i32 = 2;
pub const MENU_QUIT: i32 = 4;
pub const MENU_COMPACT_WINDOW: i32 = 5;
pub const MENU_ABOUT: i32 = 6;

/// Builds the system tray contents from the current state.
/// `compact_visible`: whether the compact window is currently shown (decides whether the menu says "Show" or "Hide").
pub fn tray_content(
    state: &TrafficState,
    settings: &AppSettings,
    compact_visible: bool,
) -> TrayContent {
    let unit = rate_units(settings.unit_style)[2];
    let tooltip_body = match &state.connection {
        ConnectionState::NotConfigured => String::new(),
        ConnectionState::Failed { error, .. } => text::snmp_error(error),
        _ => match state.tray_interface(settings) {
            Some(interface) => format!("{} · {}", interface.name, settings.host),
            None => settings.host.clone(),
        },
    };
    TrayContent {
        // Temporary self-drawn monochrome icon (modeled on the app icon's gauge), to be replaced by the final one later.
        icon_name: format!("{}-symbolic", crate::config::ICON_NAME),
        label: state.tray_title(settings),
        label_guide: format!("↓ 999.9 {unit} ↑ 999.9 {unit}"),
        tooltip_title: "Anemobit".to_owned(),
        tooltip_body,
        menu: vec![
            MenuEntry::Item {
                id: MENU_OPEN_WINDOW,
                label: gettext("Open Main Window"),
                enabled: true,
            },
            MenuEntry::Item {
                id: MENU_SETTINGS,
                label: gettext("Settings…"),
                enabled: true,
            },
            MenuEntry::Item {
                id: MENU_COMPACT_WINDOW,
                label: if compact_visible {
                    gettext("Hide Compact Window")
                } else {
                    gettext("Show Compact Window")
                },
                enabled: true,
            },
            MenuEntry::Item {
                id: MENU_ABOUT,
                label: gettext("About Anemobit"),
                enabled: true,
            },
            MenuEntry::Separator { id: 3 },
            MenuEntry::Item {
                id: MENU_QUIT,
                label: gettext("Quit"),
                enabled: true,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use anemobit_core::formatting::RateUnitStyle;
    use anemobit_core::traffic::{PollerConfiguration, PollerStatus, PollerUpdate};
    use anemobit_core::{SnmpError, SnmpSecurity};

    fn configured() -> (TrafficState, AppSettings) {
        let settings = AppSettings {
            host: "192.168.88.1".into(),
            interface_names: vec!["pppoe-out1".into()],
            ..Default::default()
        };
        let configuration = PollerConfiguration::new(
            "192.168.88.1",
            SnmpSecurity::Community("public".into()),
            ["pppoe-out1"],
        );
        (TrafficState::new(Some(&configuration)), settings)
    }

    #[test]
    fn not_configured() {
        let content = tray_content(&TrafficState::new(None), &AppSettings::default(), false);
        assert_eq!(content.label, "Anemobit: Not Set");
        assert_eq!(content.tooltip_title, "Anemobit");
        assert_eq!(content.tooltip_body, "");
    }

    #[test]
    fn connected_shows_interface_and_device() {
        let (mut state, settings) = configured();
        state.apply(PollerUpdate::Status(PollerStatus::Connected));
        let content = tray_content(&state, &settings, false);
        assert_eq!(content.label, "↓ … ↑ …");
        assert_eq!(content.label_guide, "↓ 999.9 Mbps ↑ 999.9 Mbps");
        assert_eq!(content.tooltip_body, "pppoe-out1 · 192.168.88.1");
        let bytes = AppSettings {
            unit_style: RateUnitStyle::Bytes,
            ..settings
        };
        assert_eq!(
            tray_content(&state, &bytes, false).label_guide,
            "↓ 999.9 MB/s ↑ 999.9 MB/s"
        );
    }

    #[test]
    fn failure_shows_the_reason() {
        let (mut state, settings) = configured();
        state.apply(PollerUpdate::Status(PollerStatus::Failed {
            error: SnmpError::Timeout,
            retry_in: Duration::from_secs(2),
        }));
        let content = tray_content(&state, &settings, false);
        assert_eq!(content.label, "↓ -- ↑ --");
        assert_eq!(content.tooltip_body, "Connection timed out");
    }

    #[test]
    fn menu_items() {
        let content = tray_content(&TrafficState::new(None), &AppSettings::default(), false);
        let ids: Vec<i32> = content.menu.iter().map(MenuEntry::id).collect();
        assert_eq!(
            ids,
            [
                MENU_OPEN_WINDOW,
                MENU_SETTINGS,
                MENU_COMPACT_WINDOW,
                MENU_ABOUT,
                3,
                MENU_QUIT
            ]
        );
        let label = |content: &TrayContent| match &content.menu[2] {
            MenuEntry::Item { label, .. } => label.clone(),
            MenuEntry::Separator { .. } => String::new(),
        };
        assert_eq!(label(&content), "Show Compact Window");
        let shown = tray_content(&TrafficState::new(None), &AppSettings::default(), true);
        assert_eq!(label(&shown), "Hide Compact Window");
        assert!(
            matches!(&content.menu[3], MenuEntry::Item { label, .. } if label == "About Anemobit")
        );
        assert!(
            matches!(&content.menu[0], MenuEntry::Item { label, enabled: true, .. } if label == "Open Main Window")
        );
        assert!(matches!(content.menu[4], MenuEntry::Separator { .. }));
    }
}
