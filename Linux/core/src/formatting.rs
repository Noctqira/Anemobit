//! Formatting and input handling for display: rate text, chart ticks, uptime, device addresses.
//!
//! Rate units (bps, MB/s) aren't translated per the spec, so their text is produced here; uptime is only split into units and values,
//! and the app composes the text in the UI language.

mod device_address;
mod input_filter;
mod rate;
mod scale;
mod uptime;

pub use device_address::DeviceAddress;
pub use input_filter::{HOST_MAXIMUM_LENGTH, PORT_MAXIMUM_LENGTH, filter_host, filter_port};
pub use rate::{RateParts, RateUnitStyle, format_rate, rate_parts, rate_summary, rate_units};
pub use scale::{MINIMUM_RATE_SCALE, RateAxis, nice_ceiling, rate_axis};
pub use uptime::{UptimePart, UptimeUnit, uptime_parts};
