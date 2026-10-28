//! Live traffic state (same as TrafficViewModel on macOS). The main window, system tray and compact window share one
//! [`TrafficModel`] and reread it when it emits the `changed` signal.

use std::cell::{Ref, RefCell};
use std::ops::ControlFlow;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

use anemobit_core::SnmpError;
use anemobit_core::formatting::{RateUnitStyle, format_rate, rate_axis, rate_summary};
use anemobit_core::traffic::{
    PollerConfiguration, PollerStatus, PollerUpdate, RingBuffer, TrafficPoller, TrafficSample,
    TrafficStatistics,
};
use gettextrs::gettext;
use gtk::glib;
use gtk::glib::subclass::Signal;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::settings::AppSettings;
use crate::text;
use crate::widgets::rate_gauge::TrafficDirection;

/// Charts and statistics cover the last 5 minutes.
pub const HISTORY_WINDOW: Duration = Duration::from_secs(5 * 60);
/// Buffer capacity is sized for the fastest 1-second interval.
pub const HISTORY_CAPACITY: usize = 300;

/// Live rate and recent history for one interface.
#[derive(Debug, Clone)]
pub struct InterfaceTraffic {
    pub name: String,
    latest: Option<TrafficSample>,
    history: RingBuffer<TrafficSample>,
}

impl InterfaceTraffic {
    pub fn new(name: impl Into<String>) -> Self {
        InterfaceTraffic {
            name: name.into(),
            latest: None,
            history: RingBuffer::new(HISTORY_CAPACITY),
        }
    }

    /// The latest sample; `None` after a connection failure (chart data is kept).
    pub fn latest(&self) -> Option<&TrafficSample> {
        self.latest.as_ref()
    }

    /// All samples in the buffer (oldest to newest).
    pub fn history(&self) -> impl Iterator<Item = &TrafficSample> {
        self.history.iter()
    }

    /// Samples within 5 minutes before `now` (at 5- or 10-second intervals the buffer covers longer, so filter by time).
    pub fn recent_samples(&self, now: SystemTime) -> Vec<&TrafficSample> {
        let cutoff = now
            .checked_sub(HISTORY_WINDOW)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        self.history
            .iter()
            .filter(|sample| sample.timestamp >= cutoff)
            .collect()
    }

    /// Average and peak over the last 5 minutes.
    pub fn statistics(&self, now: SystemTime) -> Option<TrafficStatistics> {
        TrafficStatistics::from_samples(self.recent_samples(now))
    }

    /// Gauge maximum: this direction's peak over the last 5 minutes, rounded up in the display unit (e.g. 2 MB/s, 50 Mbps).
    pub fn gauge_scale(
        &self,
        direction: TrafficDirection,
        style: RateUnitStyle,
        now: SystemTime,
    ) -> f64 {
        let statistics = self.statistics(now);
        let peak = match direction {
            TrafficDirection::Download => statistics.map(|s| s.peak_in),
            TrafficDirection::Upload => statistics.map(|s| s.peak_out),
        };
        rate_axis(peak.unwrap_or(0.0), style).maximum
    }

    pub(crate) fn append(&mut self, sample: TrafficSample) {
        self.latest = Some(sample.clone());
        self.history.push(sample);
    }
}

/// Connection status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    NotConfigured,
    Connecting,
    Connected,
    Failed {
        error: SnmpError,
        retry_in: Duration,
    },
}

/// State of all monitored interfaces.
#[derive(Debug, Clone)]
pub struct TrafficState {
    pub connection: ConnectionState,
    /// Monitored interfaces, in settings order.
    pub interfaces: Vec<InterfaceTraffic>,
    /// When the last sample was received.
    pub last_updated: Option<SystemTime>,
    pub device_uptime: Option<Duration>,
}

impl TrafficState {
    /// Created from the polling settings; `None` means not set up.
    pub fn new(configuration: Option<&PollerConfiguration>) -> Self {
        let (connection, interfaces) = match configuration {
            Some(configuration) => (
                ConnectionState::Connecting,
                configuration
                    .interface_names()
                    .iter()
                    .map(InterfaceTraffic::new)
                    .collect(),
            ),
            None => (ConnectionState::NotConfigured, Vec::new()),
        };
        TrafficState {
            connection,
            interfaces,
            last_updated: None,
            device_uptime: None,
        }
    }

    pub fn apply(&mut self, update: PollerUpdate) {
        match update {
            PollerUpdate::Snapshot(snapshot) => {
                for interface in &mut self.interfaces {
                    if let Some(sample) = snapshot.samples.get(&interface.name) {
                        interface.append(sample.clone());
                    }
                }
                self.last_updated = Some(snapshot.timestamp);
                self.device_uptime = snapshot.device_uptime;
            }
            PollerUpdate::Status(PollerStatus::Connecting) => {
                self.connection = ConnectionState::Connecting
            }
            PollerUpdate::Status(PollerStatus::Connected) => {
                self.connection = ConnectionState::Connected
            }
            PollerUpdate::Status(PollerStatus::Failed { error, retry_in }) => {
                self.connection = ConnectionState::Failed { error, retry_in };
                for interface in &mut self.interfaces {
                    interface.latest = None;
                }
            }
        }
    }

    pub fn interface(&self, name: &str) -> Option<&InterfaceTraffic> {
        self.interfaces
            .iter()
            .find(|interface| interface.name == name)
    }

    /// Interface shown in the system tray (and compact window): the one chosen in settings, or the first if it's missing.
    pub fn tray_interface(&self, settings: &AppSettings) -> Option<&InterfaceTraffic> {
        settings
            .effective_tray_interface()
            .and_then(|name| self.interface(name))
            .or_else(|| self.interfaces.first())
    }

    /// System tray text (same as the macOS menu bar title).
    pub fn tray_title(&self, settings: &AppSettings) -> String {
        match &self.connection {
            ConnectionState::NotConfigured => gettext("Anemobit: Not Set"),
            ConnectionState::Connected => match self
                .tray_interface(settings)
                .and_then(InterfaceTraffic::latest)
            {
                Some(latest) => rate_summary(latest, settings.unit_style),
                None => "↓ … ↑ …".to_owned(),
            },
            ConnectionState::Connecting => "↓ … ↑ …".to_owned(),
            ConnectionState::Failed { .. } => "↓ -- ↑ --".to_owned(),
        }
    }

    /// Download / upload text for the compact window ("↓ 12.3 Mbps", "↑ 1.2 Mbps"); `None` when not set up.
    pub fn compact_rates(&self, settings: &AppSettings) -> Option<(String, String)> {
        let (down, up) = match &self.connection {
            ConnectionState::NotConfigured => return None,
            ConnectionState::Connected => match self
                .tray_interface(settings)
                .and_then(InterfaceTraffic::latest)
            {
                Some(latest) => (
                    format_rate(latest.in_bps, settings.unit_style),
                    format_rate(latest.out_bps, settings.unit_style),
                ),
                None => ("…".to_owned(), "…".to_owned()),
            },
            ConnectionState::Connecting => ("…".to_owned(), "…".to_owned()),
            ConnectionState::Failed { .. } => ("--".to_owned(), "--".to_owned()),
        };
        Some((format!("↓ {down}"), format!("↑ {up}")))
    }

    /// Window title and subtitle. One interface: title is the interface name, subtitle the device. Several: title is the device,
    /// subtitle the interface count. Not set up: Anemobit.
    pub fn titles(&self, host: &str) -> (String, String) {
        match (&self.connection, self.interfaces.as_slice()) {
            (ConnectionState::NotConfigured, _) => ("Anemobit".to_owned(), String::new()),
            (_, [only]) => (only.name.clone(), host.to_owned()),
            (_, interfaces) => (host.to_owned(), text::interface_count(interfaces.len())),
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct TrafficModel {
        pub(super) settings: RefCell<AppSettings>,
        pub(super) state: RefCell<TrafficState>,
        /// Settings used by the current polling; restart only if the saved settings differ.
        pub(super) active: RefCell<Option<PollerConfiguration>>,
        pub(super) poller: RefCell<Option<glib::JoinHandle<()>>>,
    }

    impl Default for TrafficModel {
        fn default() -> Self {
            TrafficModel {
                settings: RefCell::default(),
                state: RefCell::new(TrafficState::new(None)),
                active: RefCell::default(),
                poller: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TrafficModel {
        const NAME: &'static str = "AnemobitTrafficModel";
        type Type = super::TrafficModel;
    }

    impl ObjectImpl for TrafficModel {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| vec![Signal::builder("changed").build()])
        }

        fn dispose(&self) {
            if let Some(handle) = self.poller.take() {
                handle.abort();
            }
        }
    }
}

glib::wrapper! {
    pub struct TrafficModel(ObjectSubclass<imp::TrafficModel>);
}

impl Default for TrafficModel {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl TrafficModel {
    pub fn settings(&self) -> Ref<'_, AppSettings> {
        self.imp().settings.borrow()
    }

    pub fn state(&self) -> Ref<'_, TrafficState> {
        self.imp().state.borrow()
    }

    /// Applies new settings: restarts polling only if connection settings (including the refresh interval) changed; changing only the display style doesn't interrupt.
    pub fn apply_settings(&self, settings: AppSettings) {
        let configuration = settings.poller_configuration();
        self.imp().settings.replace(settings);
        if configuration == *self.imp().active.borrow() {
            self.emit_changed();
            return;
        }
        self.restart();
    }

    /// Restarts polling with the current settings (clears the charts).
    pub fn restart(&self) {
        let imp = self.imp();
        if let Some(handle) = imp.poller.take() {
            handle.abort();
        }
        let configuration = imp.settings.borrow().poller_configuration();
        imp.state.replace(TrafficState::new(configuration.as_ref()));
        imp.active.replace(configuration.clone());
        if let Some(configuration) = configuration {
            let weak = self.downgrade();
            let poller = TrafficPoller::new(configuration);
            let handle = glib::spawn_future_local(poller.run(move |update| {
                let Some(model) = weak.upgrade() else {
                    return ControlFlow::Break(());
                };
                model.imp().state.borrow_mut().apply(update);
                model.emit_changed();
                ControlFlow::Continue(())
            }));
            imp.poller.replace(Some(handle));
        }
        self.emit_changed();
    }

    pub fn connect_changed<F: Fn(&Self) + 'static>(&self, callback: F) -> glib::SignalHandlerId {
        self.connect_local("changed", false, move |values| {
            let model = values[0]
                .get::<Self>()
                .expect("signal emitted by TrafficModel");
            callback(&model);
            None
        })
    }

    fn emit_changed(&self) {
        self.emit_by_name::<()>("changed", &[]);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use anemobit_core::SnmpSecurity;

    fn configuration(names: &[&str]) -> PollerConfiguration {
        PollerConfiguration::new(
            "192.168.88.1",
            SnmpSecurity::Community("public".into()),
            names.iter().copied(),
        )
    }

    fn sample(seconds_ago: u64, in_bps: f64, out_bps: f64, now: SystemTime) -> TrafficSample {
        TrafficSample {
            timestamp: now - Duration::from_secs(seconds_ago),
            in_bps,
            out_bps,
            device_uptime: Some(Duration::from_secs(3_600)),
        }
    }

    fn snapshot(samples: &[(&str, TrafficSample)]) -> PollerUpdate {
        let timestamp = samples[0].1.timestamp;
        PollerUpdate::Snapshot(anemobit_core::traffic::TrafficSnapshot {
            timestamp,
            device_uptime: samples[0].1.device_uptime,
            samples: samples
                .iter()
                .map(|(n, s)| ((*n).to_owned(), s.clone()))
                .collect::<HashMap<_, _>>(),
        })
    }

    #[test]
    fn new_state_follows_configuration() {
        let state = TrafficState::new(None);
        assert_eq!(state.connection, ConnectionState::NotConfigured);
        assert!(state.interfaces.is_empty());

        let state = TrafficState::new(Some(&configuration(&["pppoe-out1", "ether1"])));
        assert_eq!(state.connection, ConnectionState::Connecting);
        let names: Vec<_> = state.interfaces.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["pppoe-out1", "ether1"]);
        assert_eq!(state.last_updated, None);
    }

    #[test]
    fn snapshots_go_to_their_interfaces() {
        let now = SystemTime::now();
        let mut state = TrafficState::new(Some(&configuration(&["pppoe-out1", "ether1"])));
        state.apply(PollerUpdate::Status(PollerStatus::Connected));
        assert_eq!(state.connection, ConnectionState::Connected);

        state.apply(snapshot(&[("pppoe-out1", sample(0, 10.0, 1.0, now))]));
        assert_eq!(
            state
                .interface("pppoe-out1")
                .unwrap()
                .latest()
                .map(|s| s.in_bps),
            Some(10.0)
        );
        assert!(state.interface("ether1").unwrap().latest().is_none());
        assert_eq!(state.last_updated, Some(now));
        assert_eq!(state.device_uptime, Some(Duration::from_secs(3_600)));
    }

    #[test]
    fn failure_clears_latest_but_keeps_history() {
        let now = SystemTime::now();
        let mut state = TrafficState::new(Some(&configuration(&["pppoe-out1"])));
        state.apply(snapshot(&[("pppoe-out1", sample(0, 10.0, 1.0, now))]));
        let failed = ConnectionState::Failed {
            error: SnmpError::Timeout,
            retry_in: Duration::from_secs(2),
        };
        state.apply(PollerUpdate::Status(PollerStatus::Failed {
            error: SnmpError::Timeout,
            retry_in: Duration::from_secs(2),
        }));
        assert_eq!(state.connection, failed);
        let interface = state.interface("pppoe-out1").unwrap();
        assert!(interface.latest().is_none());
        assert_eq!(interface.history().count(), 1);
    }

    #[test]
    fn history_keeps_the_last_300_samples() {
        let now = SystemTime::now();
        let mut interface = InterfaceTraffic::new("ether1");
        for i in 0..350 {
            interface.append(sample(350 - i, i as f64, 0.0, now));
        }
        assert_eq!(interface.history().count(), HISTORY_CAPACITY);
        assert_eq!(interface.history().next().map(|s| s.in_bps), Some(50.0));
    }

    #[test]
    fn statistics_cover_the_last_five_minutes() {
        let now = SystemTime::now();
        let mut interface = InterfaceTraffic::new("ether1");
        interface.append(sample(400, 1_000.0, 1_000.0, now)); // Older than 5 minutes, not counted
        interface.append(sample(200, 10.0, 1.0, now));
        interface.append(sample(100, 30.0, 5.0, now));
        assert_eq!(interface.recent_samples(now).len(), 2);
        let stats = interface.statistics(now).unwrap();
        assert_eq!(
            (stats.average_in, stats.peak_in, stats.peak_out),
            (20.0, 30.0, 5.0)
        );
        assert_eq!(InterfaceTraffic::new("x").statistics(now), None);
    }

    #[test]
    fn gauge_scale_rounds_the_recent_peak_in_the_display_unit() {
        let now = SystemTime::now();
        let mut interface = InterfaceTraffic::new("ether1");
        interface.append(sample(10, 37_000_000.0, 1_200_000.0, now));
        assert_eq!(
            interface.gauge_scale(TrafficDirection::Download, RateUnitStyle::Bits, now),
            50_000_000.0
        );
        assert_eq!(
            interface.gauge_scale(TrafficDirection::Upload, RateUnitStyle::Bits, now),
            2_000_000.0
        );
        // Minimum when there's no data: 1 Kbps / 1 KB/s
        let empty = InterfaceTraffic::new("x");
        assert_eq!(
            empty.gauge_scale(TrafficDirection::Download, RateUnitStyle::Bits, now),
            1_000.0
        );
        assert_eq!(
            empty.gauge_scale(TrafficDirection::Download, RateUnitStyle::Bytes, now),
            8_192.0
        );
    }

    fn tray_settings(names: &[&str], tray: &str) -> AppSettings {
        AppSettings {
            interface_names: names.iter().map(|n| (*n).to_owned()).collect(),
            tray_interface: tray.to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn tray_interface_follows_settings() {
        let state = TrafficState::new(Some(&configuration(&["pppoe-out1", "ether1"])));
        let name = |tray: &str| {
            state
                .tray_interface(&tray_settings(&["pppoe-out1", "ether1"], tray))
                .map(|i| i.name.clone())
        };
        assert_eq!(name("ether1").as_deref(), Some("ether1"));
        assert_eq!(name("").as_deref(), Some("pppoe-out1"));
        assert_eq!(name("wlan9").as_deref(), Some("pppoe-out1"));
        assert!(
            TrafficState::new(None)
                .tray_interface(&AppSettings::default())
                .is_none()
        );
    }

    #[test]
    fn tray_title_follows_connection_state() {
        let now = SystemTime::now();
        let settings = tray_settings(&["pppoe-out1"], "pppoe-out1");
        assert_eq!(
            TrafficState::new(None).tray_title(&settings),
            "Anemobit: Not Set"
        );

        let mut state = TrafficState::new(Some(&configuration(&["pppoe-out1"])));
        assert_eq!(state.tray_title(&settings), "↓ … ↑ …");
        state.apply(PollerUpdate::Status(PollerStatus::Connected));
        assert_eq!(state.tray_title(&settings), "↓ … ↑ …"); // No data yet
        state.apply(snapshot(&[(
            "pppoe-out1",
            sample(0, 12_345_678.0, 1_234_567.0, now),
        )]));
        assert_eq!(state.tray_title(&settings), "↓ 12.3 Mbps ↑ 1.2 Mbps");
        let bytes = AppSettings {
            unit_style: RateUnitStyle::Bytes,
            ..settings.clone()
        };
        assert_eq!(state.tray_title(&bytes), "↓ 1.5 MB/s ↑ 150.7 KB/s");
        state.apply(PollerUpdate::Status(PollerStatus::Failed {
            error: SnmpError::Timeout,
            retry_in: Duration::from_secs(1),
        }));
        assert_eq!(state.tray_title(&settings), "↓ -- ↑ --");
    }

    #[test]
    fn compact_rates_follow_connection_state() {
        let now = SystemTime::now();
        let settings = tray_settings(&["pppoe-out1"], "pppoe-out1");
        assert_eq!(TrafficState::new(None).compact_rates(&settings), None);
        let pair = |a: &str, b: &str| Some((a.to_owned(), b.to_owned()));

        let mut state = TrafficState::new(Some(&configuration(&["pppoe-out1"])));
        assert_eq!(state.compact_rates(&settings), pair("↓ …", "↑ …"));
        state.apply(PollerUpdate::Status(PollerStatus::Connected));
        state.apply(snapshot(&[(
            "pppoe-out1",
            sample(0, 12_345_678.0, 1_234_567.0, now),
        )]));
        assert_eq!(
            state.compact_rates(&settings),
            pair("↓ 12.3 Mbps", "↑ 1.2 Mbps")
        );
        state.apply(PollerUpdate::Status(PollerStatus::Failed {
            error: SnmpError::Timeout,
            retry_in: Duration::from_secs(1),
        }));
        assert_eq!(state.compact_rates(&settings), pair("↓ --", "↑ --"));
    }

    #[test]
    fn titles_depend_on_interface_count() {
        assert_eq!(
            TrafficState::new(None).titles("x"),
            ("Anemobit".to_owned(), String::new())
        );
        let one = TrafficState::new(Some(&configuration(&["pppoe-out1"])));
        assert_eq!(
            one.titles("192.168.88.1"),
            ("pppoe-out1".to_owned(), "192.168.88.1".to_owned())
        );
        let two = TrafficState::new(Some(&configuration(&["pppoe-out1", "ether1"])));
        assert_eq!(
            two.titles("192.168.88.1"),
            ("192.168.88.1".to_owned(), "2 interfaces".to_owned())
        );
    }
}
