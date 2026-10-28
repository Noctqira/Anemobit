use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// One rate sample.
#[derive(Debug, Clone, PartialEq)]
pub struct TrafficSample {
    /// Wall-clock time for the chart's x axis (rate calculation itself uses the monotonic clock).
    pub timestamp: SystemTime,
    pub in_bps: f64,
    pub out_bps: f64,
    /// Device uptime (sysUpTime); `None` when unavailable.
    pub device_uptime: Option<Duration>,
}

/// Result of one sample: all monitored interfaces share the same time point and device uptime.
#[derive(Debug, Clone, PartialEq)]
pub struct TrafficSnapshot {
    pub timestamp: SystemTime,
    pub device_uptime: Option<Duration>,
    /// Interface name (ifName) → that interface's rate sample.
    /// Interfaces with no rate this round (first sample, counter reset) are absent.
    pub samples: HashMap<String, TrafficSample>,
}
