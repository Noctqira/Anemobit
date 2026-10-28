use std::collections::HashMap;
use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::ops::ControlFlow;
use std::time::{Duration, Instant, SystemTime};

use crate::oids::{IF_HC_IN_OCTETS, IF_HC_OUT_OCTETS, SYS_UP_TIME};
use crate::transport::{Transport, UdpTransport};
use crate::{Oid, SnmpClient, SnmpError, SnmpSecurity, SnmpValue, VarBind};

use super::rate_calculator::{CounterReading, RateCalculator};
use super::{TrafficSample, TrafficSnapshot};

/// Polling settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollerConfiguration {
    host: String,
    port: u16,
    security: SnmpSecurity,
    interface_names: Vec<String>,
    interval: Duration,
    timeout: Duration,
    retries: u32,
}

impl PollerConfiguration {
    pub const DEFAULT_PORT: u16 = 161;
    pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(1);

    /// `interface_names` are the interface names (ifName) to monitor, resolved to ifIndex on every connection;
    /// duplicate names keep only the first. Port, interval, timeout and retries use the defaults (161, 1 second, 2 seconds, 1).
    pub fn new(
        host: impl Into<String>,
        security: SnmpSecurity,
        interface_names: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut unique: Vec<String> = Vec::new();
        for name in interface_names {
            let name = name.into();
            if !unique.contains(&name) {
                unique.push(name);
            }
        }
        PollerConfiguration {
            host: host.into(),
            port: Self::DEFAULT_PORT,
            security,
            interface_names: unique,
            interval: Self::DEFAULT_INTERVAL,
            timeout: SnmpClient::<UdpTransport>::DEFAULT_TIMEOUT,
            retries: SnmpClient::<UdpTransport>::DEFAULT_RETRIES,
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn interface_names(&self) -> &[String] {
        &self.interface_names
    }
}

/// Polling connection status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollerStatus {
    Connecting,
    Connected,
    /// Failed; retries automatically after `retry_in`.
    Failed {
        error: SnmpError,
        retry_in: Duration,
    },
}

/// Update sent by polling.
#[derive(Debug, Clone, PartialEq)]
pub enum PollerUpdate {
    Status(PollerStatus),
    Snapshot(TrafficSnapshot),
}

/// Time source, so tests can use a virtual clock.
pub trait Clock {
    /// Current time on the monotonic clock.
    fn now(&self) -> Instant;

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()>;
}

/// Local monotonic clock; sleep uses glib timers.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    async fn sleep(&self, duration: Duration) {
        glib::timeout_future(duration).await;
    }
}

/// Creates the transport to the device, so tests can use a double.
pub trait Connector {
    type Transport: Transport;

    fn connect(
        &mut self,
        host: &str,
        port: u16,
    ) -> impl Future<Output = io::Result<Self::Transport>>;
}

/// Connects with [`UdpTransport`].
#[derive(Debug, Clone, Copy, Default)]
pub struct UdpConnector;

impl Connector for UdpConnector {
    type Transport = UdpTransport;

    async fn connect(&mut self, host: &str, port: u16) -> io::Result<UdpTransport> {
        UdpTransport::connect(host, port).await
    }
}

/// Exponential backoff: initial, 2×, 4×…, capped at maximum.
#[derive(Debug, Clone)]
pub(crate) struct Backoff {
    initial: Duration,
    maximum: Duration,
    current: Option<Duration>,
}

impl Backoff {
    pub(crate) fn new(initial: Duration, maximum: Duration) -> Self {
        Backoff {
            initial,
            maximum,
            current: None,
        }
    }

    pub(crate) fn next(&mut self) -> Duration {
        let value = match self.current {
            Some(current) => (current * 2).min(self.maximum),
            None => self.initial,
        };
        self.current = Some(value);
        value
    }

    pub(crate) fn reset(&mut self) {
        self.current = None;
    }
}

/// Polling that reads ifHCInOctets / ifHCOutOctets over SNMP (v2c or v3).
/// All interfaces' counters and sysUpTime are read in one GET, and each interface's rate is calculated separately.
#[derive(Debug)]
pub struct TrafficPoller<C: Connector = UdpConnector, K: Clock = SystemClock> {
    configuration: PollerConfiguration,
    connector: C,
    clock: K,
}

impl TrafficPoller {
    /// Polls over UDP with the local clock.
    pub fn new(configuration: PollerConfiguration) -> Self {
        Self::with_connector(configuration, UdpConnector, SystemClock)
    }
}

/// Why a connection ended.
enum SessionEnd {
    /// The update receiver asked to stop.
    Stopped,
    Failed(SnmpError),
}

impl From<SnmpError> for SessionEnd {
    fn from(error: SnmpError) -> Self {
        SessionEnd::Failed(error)
    }
}

impl<C: Connector, K: Clock> TrafficPoller<C, K> {
    /// Longest retry interval after failures.
    pub const MAXIMUM_BACKOFF: Duration = Duration::from_secs(10);

    pub fn with_connector(configuration: PollerConfiguration, connector: C, clock: K) -> Self {
        TrafficPoller {
            configuration,
            connector,
            clock,
        }
    }

    /// Polls continuously, calling `on_update` on every status change or new rate; reconnects automatically with exponential backoff on failure.
    ///
    /// Stops and returns when `on_update` returns `ControlFlow::Break`. Dropping this future (e.g. aborting the
    /// `glib::spawn_future_local` handle) also stops it; both close the connection.
    /// Must run on a `glib::MainContext`.
    pub async fn run(mut self, mut on_update: impl FnMut(PollerUpdate) -> ControlFlow<()>) {
        let mut backoff = Backoff::new(Duration::from_secs(1), Self::MAXIMUM_BACKOFF);
        loop {
            if on_update(PollerUpdate::Status(PollerStatus::Connecting)).is_break() {
                return;
            }
            let configuration = &self.configuration;
            let error = match self
                .connector
                .connect(&configuration.host, configuration.port)
                .await
            {
                Err(error) => SnmpError::Transport(error.to_string()),
                Ok(transport) => {
                    let mut client = SnmpClient::new(transport, configuration.security.clone())
                        .with_timeout(configuration.timeout)
                        .with_retries(configuration.retries);
                    // The client is dropped at the end of this block, closing the connection.
                    let Err(end) = self
                        .poll_session(&mut client, &mut backoff, &mut on_update)
                        .await;
                    match end {
                        SessionEnd::Stopped => return,
                        SessionEnd::Failed(error) => error,
                    }
                }
            };
            let retry_in = backoff.next();
            if on_update(PollerUpdate::Status(PollerStatus::Failed {
                error,
                retry_in,
            }))
            .is_break()
            {
                return;
            }
            self.clock.sleep(retry_in).await;
        }
    }

    /// Resolves the interfaces, then polls until an error or the receiver asks to stop.
    async fn poll_session<T: Transport>(
        &self,
        client: &mut SnmpClient<T>,
        backoff: &mut Backoff,
        on_update: &mut impl FnMut(PollerUpdate) -> ControlFlow<()>,
    ) -> Result<Infallible, SessionEnd> {
        let names = &self.configuration.interface_names;
        let indexes = client.resolve_interface_indexes(names).await?;
        // Two OIDs per interface (in, out), then sysUpTime.
        let oids: Vec<Oid> = indexes
            .iter()
            .flat_map(|&index| [IF_HC_IN_OCTETS.child(index), IF_HC_OUT_OCTETS.child(index)])
            .chain([SYS_UP_TIME.clone()])
            .collect();

        let mut calculators: HashMap<String, RateCalculator> = HashMap::new();
        let mut connected = false;
        let mut deadline = self.clock.now();
        loop {
            let binds = client.get(&oids).await?;
            let readings = parse(&binds, &oids, names, self.clock.now())?;

            if !connected {
                connected = true;
                backoff.reset();
                if on_update(PollerUpdate::Status(PollerStatus::Connected)).is_break() {
                    return Err(SessionEnd::Stopped);
                }
            }

            let timestamp = SystemTime::now();
            // sysUpTime is in hundredths of a second.
            let device_uptime = readings
                .sys_up_time
                .map(|ticks| Duration::from_millis(u64::from(ticks) * 10));
            let mut samples = HashMap::new();
            for (name, reading) in readings.counters {
                if let Some(rate) = calculators.entry(name.clone()).or_default().add(reading) {
                    let sample = TrafficSample {
                        timestamp,
                        in_bps: rate.in_bps,
                        out_bps: rate.out_bps,
                        device_uptime,
                    };
                    samples.insert(name, sample);
                }
            }
            if !samples.is_empty() {
                let snapshot = TrafficSnapshot {
                    timestamp,
                    device_uptime,
                    samples,
                };
                if on_update(PollerUpdate::Snapshot(snapshot)).is_break() {
                    return Err(SessionEnd::Stopped);
                }
            }

            // Schedule on a fixed beat; if behind, don't catch up, just start from now.
            deadline += self.configuration.interval;
            let now = self.clock.now();
            deadline = deadline.max(now);
            self.clock.sleep(deadline - now).await;
        }
    }
}

/// Result of one GET, split up.
struct Readings {
    sys_up_time: Option<u32>,
    /// In the configured interface order.
    counters: Vec<(String, CounterReading)>,
}

/// Splits a GET result into sysUpTime and each interface's counters.
fn parse(
    binds: &[VarBind],
    oids: &[Oid],
    names: &[String],
    instant: Instant,
) -> Result<Readings, SnmpError> {
    if binds.len() != oids.len() || binds.iter().zip(oids).any(|(bind, oid)| bind.oid != *oid) {
        return Err(SnmpError::UnexpectedResponse(
            "varbind count or OID mismatch".into(),
        ));
    }
    let sys_up_time = match binds.last().map(|bind| &bind.value) {
        Some(SnmpValue::TimeTicks(ticks)) => Some(*ticks),
        _ => None,
    };
    let counters = names
        .iter()
        .zip(binds.as_chunks::<2>().0)
        .map(
            |(name, pair)| match (pair[0].value.as_u64(), pair[1].value.as_u64()) {
                (Some(in_octets), Some(out_octets)) => Ok((
                    name.clone(),
                    CounterReading {
                        in_octets,
                        out_octets,
                        sys_up_time,
                        instant,
                    },
                )),
                // Interface index changed (e.g. renumbered after a device restart): return an error so the outer loop reconnects and resolves again.
                _ => Err(SnmpError::UnexpectedResponse(format!(
                    "interface counters unavailable for {name} ({:?})",
                    pair[0].value
                ))),
            },
        )
        .collect::<Result<_, _>>()?;
    Ok(Readings {
        sys_up_time,
        counters,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;
    use crate::oids::IF_NAME;
    use crate::testing::{FakeAgent, FakeV3Agent, MockTransport, TestClock, run};
    use crate::{AuthProtocol, PrivProtocol, UsmCredentials, UsmPrivacy};

    /// Records the transport created for each connection; `make`'s argument is the connection count (starting at 0).
    #[derive(Clone)]
    struct RecordingConnector {
        make: Rc<dyn Fn(usize) -> io::Result<MockTransport>>,
        transports: Rc<RefCell<Vec<MockTransport>>>,
        attempts: Rc<RefCell<usize>>,
    }

    impl RecordingConnector {
        fn new(make: impl Fn(usize) -> io::Result<MockTransport> + 'static) -> Self {
            RecordingConnector {
                make: Rc::new(make),
                transports: Rc::default(),
                attempts: Rc::default(),
            }
        }

        fn always(make: impl Fn() -> MockTransport + 'static) -> Self {
            Self::new(move |_| Ok(make()))
        }

        fn transports(&self) -> Vec<MockTransport> {
            self.transports.borrow().clone()
        }
    }

    impl Connector for RecordingConnector {
        type Transport = MockTransport;

        async fn connect(&mut self, _host: &str, _port: u16) -> io::Result<MockTransport> {
            let attempt = {
                let mut attempts = self.attempts.borrow_mut();
                *attempts += 1;
                *attempts - 1
            };
            let transport = (self.make)(attempt)?;
            self.transports.borrow_mut().push(transport.clone());
            Ok(transport)
        }
    }

    /// One step: (sysUpTime, [(ifIndex, in, out)]).
    type Step = (u32, Vec<(u32, u64, u64)>);

    /// Each GET switches to the next step's counters (staying on the last step once they run out).
    fn scripted(steps: Vec<Step>) -> MockTransport {
        let agent = FakeAgent::new(FakeAgent::router_os_interface_mib());
        let gets = RefCell::new(0);
        MockTransport::v2c(move |request, _| {
            if request.pdu.pdu_type == crate::snmp::pdu::PduType::Get {
                let mut count = gets.borrow_mut();
                let (up_time, counters) = &steps[(*count).min(steps.len() - 1)];
                *count += 1;
                agent.update([(SYS_UP_TIME.clone(), SnmpValue::TimeTicks(*up_time))]);
                for &(index, in_octets, out_octets) in counters {
                    agent.update([
                        (
                            IF_HC_IN_OCTETS.child(index),
                            SnmpValue::Counter64(in_octets),
                        ),
                        (
                            IF_HC_OUT_OCTETS.child(index),
                            SnmpValue::Counter64(out_octets),
                        ),
                    ]);
                }
            }
            vec![agent.respond(request).encode()]
        })
    }

    /// Sequence for pppoe-out1 (ifIndex 7) only.
    fn scripted_one(sequence: &[(u64, u64, u32)]) -> MockTransport {
        scripted(
            sequence
                .iter()
                .map(|&(i, o, up)| (up, vec![(7, i, o)]))
                .collect(),
        )
    }

    fn broken() -> MockTransport {
        let transport = MockTransport::silent();
        transport.fail_sends("unreachable");
        transport
    }

    fn configuration(names: &[&str]) -> PollerConfiguration {
        PollerConfiguration::new(
            "192.0.2.1",
            SnmpSecurity::Community("public".into()),
            names.iter().copied(),
        )
        .with_timeout(Duration::from_millis(20))
        .with_retries(0)
    }

    fn poller(
        connector: &RecordingConnector,
        clock: &TestClock,
        names: &[&str],
    ) -> TrafficPoller<RecordingConnector, TestClock> {
        TrafficPoller::with_connector(configuration(names), connector.clone(), clock.clone())
    }

    /// Runs until `done` returns true and returns all updates received.
    fn collect<C: Connector, K: Clock>(
        poller: TrafficPoller<C, K>,
        mut done: impl FnMut(&[PollerUpdate]) -> bool,
    ) -> Vec<PollerUpdate> {
        let mut updates = Vec::new();
        run(poller.run(|update| {
            updates.push(update);
            if done(&updates) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }));
        updates
    }

    fn snapshots(updates: &[PollerUpdate]) -> Vec<&TrafficSnapshot> {
        updates
            .iter()
            .filter_map(|u| match u {
                PollerUpdate::Snapshot(snapshot) => Some(snapshot),
                PollerUpdate::Status(_) => None,
            })
            .collect()
    }

    fn statuses(updates: &[PollerUpdate]) -> Vec<&PollerStatus> {
        updates
            .iter()
            .filter_map(|u| match u {
                PollerUpdate::Status(status) => Some(status),
                PollerUpdate::Snapshot(_) => None,
            })
            .collect()
    }

    fn until_snapshots(count: usize) -> impl FnMut(&[PollerUpdate]) -> bool {
        move |updates| snapshots(updates).len() == count
    }

    fn until_failures(count: usize) -> impl FnMut(&[PollerUpdate]) -> bool {
        move |updates| {
            statuses(updates)
                .iter()
                .filter(|s| matches!(s, PollerStatus::Failed { .. }))
                .count()
                == count
        }
    }

    fn first_failure(updates: &[PollerUpdate]) -> (SnmpError, Duration) {
        statuses(updates)
            .into_iter()
            .find_map(|s| match s {
                PollerStatus::Failed { error, retry_in } => Some((error.clone(), *retry_in)),
                _ => None,
            })
            .expect("a failure")
    }

    fn in_bps(snapshot: &TrafficSnapshot, name: &str) -> Option<f64> {
        snapshot.samples.get(name).map(|s| s.in_bps)
    }

    #[test]
    fn emits_samples_from_counter_sequence() {
        let connector = RecordingConnector::always(|| {
            scripted_one(&[
                (1_000_000, 100_000, 100),
                (2_250_000, 225_000, 200), // +1.25 MB / +125 kB → 10 Mb/s / 1 Mb/s
                (2_250_000, 225_000, 300), // Idle → 0
                (2_500_000, 237_500, 400), // 2 Mb/s / 100 kb/s
            ])
        });
        let clock = TestClock::default();
        let updates = collect(
            poller(&connector, &clock, &["pppoe-out1"]),
            until_snapshots(3),
        );

        let samples: Vec<_> = snapshots(&updates)
            .iter()
            .map(|s| s.samples["pppoe-out1"].clone())
            .collect();
        let rates: Vec<_> = samples.iter().map(|s| (s.in_bps, s.out_bps)).collect();
        assert_eq!(
            rates,
            [
                (10_000_000.0, 1_000_000.0),
                (0.0, 0.0),
                (2_000_000.0, 100_000.0)
            ]
        );
        // sysUpTime (hundredths of a second) comes along too
        let uptimes: Vec<_> = samples.iter().map(|s| s.device_uptime).collect();
        assert_eq!(uptimes, [2, 3, 4].map(|s| Some(Duration::from_secs(s))));
        assert!(samples.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));

        // One GET per round, querying the resolved index 7
        let gets: Vec<_> = connector.transports()[0]
            .sent()
            .into_iter()
            .filter(|m| m.pdu.pdu_type == crate::snmp::pdu::PduType::Get)
            .collect();
        let oids: Vec<_> = gets[0]
            .pdu
            .var_binds
            .iter()
            .map(|b| b.oid.clone())
            .collect();
        assert_eq!(
            oids,
            [
                IF_HC_IN_OCTETS.child(7),
                IF_HC_OUT_OCTETS.child(7),
                SYS_UP_TIME.clone()
            ]
        );
        // Polling interval is 1 second
        assert!(
            clock.sleeps()[..3]
                .iter()
                .all(|&d| d == Duration::from_secs(1))
        );
    }

    #[test]
    fn counter_reset_does_not_produce_spike() {
        let connector = RecordingConnector::always(|| {
            scripted_one(&[
                (9_000_000_000, 9_000_000_000, 1_000_000),
                (9_000_125_000, 9_000_000_000, 1_000_100), // 1 Mb/s
                (500, 500, 50),                            // Restarted → discarded
                (125_500, 500, 150),                       // 1 Mb/s
            ])
        });
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1"]),
            until_snapshots(2),
        );
        let rates: Vec<_> = snapshots(&updates)
            .iter()
            .map(|s| in_bps(s, "pppoe-out1"))
            .collect();
        assert_eq!(rates, [Some(1_000_000.0), Some(1_000_000.0)]);
    }

    #[test]
    fn statuses_report_connecting_then_connected() {
        let connector = RecordingConnector::always(|| scripted_one(&[(0, 0, 1)]));
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1"]),
            |u| u.len() == 2,
        );
        assert_eq!(
            updates,
            [
                PollerUpdate::Status(PollerStatus::Connecting),
                PollerUpdate::Status(PollerStatus::Connected)
            ]
        );
    }

    #[test]
    fn failure_emits_error_status_closes_connection_and_recovers() {
        let connector = RecordingConnector::new(|attempt| {
            Ok(if attempt == 0 {
                broken()
            } else {
                scripted_one(&[(0, 0, 1)])
            })
        });
        let clock = TestClock::default();
        let updates = collect(poller(&connector, &clock, &["pppoe-out1"]), |u| {
            matches!(
                u.last(),
                Some(PollerUpdate::Status(PollerStatus::Connected))
            )
        });
        let statuses = statuses(&updates);
        assert_eq!(statuses.len(), 4);
        assert_eq!(statuses[0], &PollerStatus::Connecting);
        assert!(
            matches!(statuses[1], PollerStatus::Failed { error: SnmpError::Transport(_), retry_in } if *retry_in == Duration::from_secs(1))
        );
        assert_eq!(statuses[2], &PollerStatus::Connecting);
        assert_eq!(statuses[3], &PollerStatus::Connected);
        assert_eq!(clock.sleeps().first(), Some(&Duration::from_secs(1)));
        // The failed connection is closed: only the test's own handle remains.
        assert_eq!(connector.transports()[0].handle_count(), 2);
    }

    #[test]
    fn backoff_doubles_up_to_ten_seconds() {
        let connector = RecordingConnector::always(broken);
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1"]),
            until_failures(6),
        );
        let delays: Vec<_> = statuses(&updates)
            .into_iter()
            .filter_map(|s| match s {
                PollerStatus::Failed { retry_in, .. } => Some(retry_in.as_secs()),
                _ => None,
            })
            .collect();
        assert_eq!(delays, [1, 2, 4, 8, 10, 10]);
    }

    #[test]
    fn connect_failure_is_reported_and_retried() {
        let connector = RecordingConnector::new(|attempt| {
            if attempt == 0 {
                Err(io::Error::other("name resolution failed"))
            } else {
                Ok(scripted_one(&[(0, 0, 1)]))
            }
        });
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1"]),
            |u| {
                matches!(
                    u.last(),
                    Some(PollerUpdate::Status(PollerStatus::Connected))
                )
            },
        );
        assert!(
            matches!(first_failure(&updates).0, SnmpError::Transport(message) if message.contains("name resolution"))
        );
    }

    #[test]
    fn missing_interface_is_reported_as_failure() {
        let connector = RecordingConnector::always(|| {
            MockTransport::with_agent(FakeAgent::new(FakeAgent::router_os_interface_mib()))
        });
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out9"]),
            until_failures(1),
        );
        assert_eq!(
            first_failure(&updates).0,
            SnmpError::InterfaceNotFound("pppoe-out9".into())
        );
    }

    #[test]
    fn missing_one_of_several_interfaces_is_reported_as_failure() {
        let connector = RecordingConnector::always(|| {
            MockTransport::with_agent(FakeAgent::new(FakeAgent::router_os_interface_mib()))
        });
        let updates = collect(
            poller(
                &connector,
                &TestClock::default(),
                &["pppoe-out1", "pppoe-out2"],
            ),
            until_failures(1),
        );
        assert_eq!(
            first_failure(&updates).0,
            SnmpError::InterfaceNotFound("pppoe-out2".into())
        );
    }

    #[test]
    fn missing_counters_are_reported_as_failure() {
        // The interface still exists but its counter can't be read (e.g. renumbered after a device restart): return an error so the outer loop reconnects and resolves again.
        let agent = FakeAgent::new(
            [(
                IF_NAME.child(7),
                SnmpValue::OctetString(b"pppoe-out1".to_vec()),
            )]
            .into(),
        );
        let connector =
            RecordingConnector::always(move || MockTransport::with_agent(agent.clone()));
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1"]),
            until_failures(1),
        );
        assert!(
            matches!(first_failure(&updates).0, SnmpError::UnexpectedResponse(m) if m.contains("pppoe-out1"))
        );
    }

    #[test]
    fn timeout_is_reported_as_failure() {
        let connector = RecordingConnector::always(MockTransport::silent);
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1"]),
            until_failures(1),
        );
        assert_eq!(first_failure(&updates).0, SnmpError::Timeout);
    }

    #[test]
    fn stopping_ends_the_run_and_closes_the_connection() {
        let connector = RecordingConnector::always(|| scripted_one(&[(0, 0, 1), (125, 0, 2)]));
        collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1"]),
            until_snapshots(1),
        );
        let transports = connector.transports();
        assert_eq!(transports.len(), 1);
        assert_eq!(transports[0].handle_count(), 2);
    }

    #[test]
    fn reads_all_interfaces_in_one_get_and_computes_rates_separately() {
        // pppoe-out1 = ifIndex 7, ether1 = ifIndex 1
        let connector = RecordingConnector::always(|| {
            scripted(vec![
                (100, vec![(7, 1_000_000, 100_000), (1, 0, 0)]),
                (200, vec![(7, 2_250_000, 225_000), (1, 125_000, 250_000)]), // 10 / 1 Mb/s; 1 / 2 Mb/s
            ])
        });
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1", "ether1"]),
            until_snapshots(1),
        );
        let snapshot = snapshots(&updates)[0];
        let rates = |name: &str| snapshot.samples.get(name).map(|s| (s.in_bps, s.out_bps));
        assert_eq!(rates("pppoe-out1"), Some((10_000_000.0, 1_000_000.0)));
        assert_eq!(rates("ether1"), Some((1_000_000.0, 2_000_000.0)));
        assert_eq!(snapshot.device_uptime, Some(Duration::from_secs(2)));
        assert!(
            snapshot
                .samples
                .values()
                .all(|s| s.timestamp == snapshot.timestamp)
        );

        // One GET reads all interfaces, in settings order, followed by sysUpTime
        let get = connector.transports()[0]
            .sent()
            .into_iter()
            .find(|m| m.pdu.pdu_type == crate::snmp::pdu::PduType::Get)
            .unwrap();
        let oids: Vec<_> = get.pdu.var_binds.iter().map(|b| b.oid.clone()).collect();
        assert_eq!(
            oids,
            [
                IF_HC_IN_OCTETS.child(7),
                IF_HC_OUT_OCTETS.child(7),
                IF_HC_IN_OCTETS.child(1),
                IF_HC_OUT_OCTETS.child(1),
                SYS_UP_TIME.clone()
            ]
        );
    }

    #[test]
    fn counter_reset_on_one_interface_keeps_the_others() {
        let connector = RecordingConnector::always(|| {
            scripted(vec![
                (100, vec![(7, 1_000_000, 0), (1, 1_000_000, 0)]),
                (200, vec![(7, 1_125_000, 0), (1, 1_125_000, 0)]), // Both are 1 Mb/s
                (300, vec![(7, 500, 0), (1, 1_250_000, 0)]), // ifIndex 7 counter reset → only it is discarded
                (400, vec![(7, 125_500, 0), (1, 1_375_000, 0)]),
            ])
        });
        let updates = collect(
            poller(&connector, &TestClock::default(), &["pppoe-out1", "ether1"]),
            until_snapshots(3),
        );
        let snapshots = snapshots(&updates);
        let ether: Vec<_> = snapshots.iter().map(|s| in_bps(s, "ether1")).collect();
        let pppoe: Vec<_> = snapshots.iter().map(|s| in_bps(s, "pppoe-out1")).collect();
        assert_eq!(ether, [Some(1_000_000.0); 3]);
        assert_eq!(pppoe, [Some(1_000_000.0), None, Some(1_000_000.0)]);
    }

    #[test]
    fn polling_works_over_v3() {
        let credentials = UsmCredentials {
            user_name: "anemobit".into(),
            auth_protocol: AuthProtocol::Sha1,
            auth_password: "authpass12".into(),
            privacy: Some(UsmPrivacy {
                protocol: PrivProtocol::Aes,
                password: "privpass12".into(),
            }),
        };
        let agent = FakeV3Agent::new(credentials.clone());
        let connector =
            RecordingConnector::always(move || MockTransport::with_v3_agent(agent.clone()));
        let configuration =
            PollerConfiguration::new("192.0.2.1", SnmpSecurity::Usm(credentials), ["pppoe-out1"])
                .with_timeout(Duration::from_millis(50))
                .with_retries(0);
        let poller = TrafficPoller::with_connector(configuration, connector, TestClock::default());
        let updates = collect(poller, until_snapshots(1));
        assert_eq!(
            statuses(&updates),
            [&PollerStatus::Connecting, &PollerStatus::Connected]
        );
        assert_eq!(in_bps(snapshots(&updates)[0], "pppoe-out1"), Some(0.0));
    }

    #[test]
    fn configuration_defaults_and_duplicate_names() {
        let configuration = PollerConfiguration::new(
            "192.0.2.1",
            SnmpSecurity::Community("public".into()),
            ["ether1", "pppoe-out1", "ether1"],
        );
        assert_eq!(configuration.interface_names(), ["ether1", "pppoe-out1"]);
        assert_eq!(configuration.port(), 161);
        assert_eq!(configuration.interval, Duration::from_secs(1));
        assert_eq!(configuration.timeout, Duration::from_secs(2));
        assert_eq!(configuration.retries, 1);
    }

    #[test]
    fn backoff_sequence_and_reset() {
        let mut backoff = Backoff::new(Duration::from_secs(1), Duration::from_secs(10));
        let delays: Vec<_> = (0..6).map(|_| backoff.next().as_secs()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 10, 10]);
        backoff.reset();
        assert_eq!(backoff.next(), Duration::from_secs(1));
    }
}
