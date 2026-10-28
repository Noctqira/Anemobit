use crate::traffic::TrafficSample;

/// How rates are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum RateUnitStyle {
    /// Bits, decimal: bps, Kbps, Mbps, Gbps (same as RouterOS).
    #[default]
    Bits,
    /// Bytes, binary (1024): B/s, KB/s, MB/s, GB/s.
    Bytes,
}

/// A rate with value and unit separated, e.g. `("12.3", "Mbps")`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateParts {
    pub value: String,
    pub unit: &'static str,
}

/// All units of this style, smallest to largest (for measuring fixed-width layouts).
pub fn rate_units(style: RateUnitStyle) -> [&'static str; 4] {
    match style {
        RateUnitStyle::Bits => ["bps", "Kbps", "Mbps", "Gbps"],
        RateUnitStyle::Bytes => ["B/s", "KB/s", "MB/s", "GB/s"],
    }
}

/// bps → "12.3 Mbps" or "1.5 MB/s".
pub fn format_rate(bps: f64, style: RateUnitStyle) -> String {
    let parts = rate_parts(bps, style);
    format!("{} {}", parts.value, parts.unit)
}

/// The value is at most "999.9" (except in the largest unit); negative and non-finite values count as 0.
pub fn rate_parts(bps: f64, style: RateUnitStyle) -> RateParts {
    let units = rate_units(style);
    let (base, amount) = match style {
        RateUnitStyle::Bits => (1_000.0, bps),
        RateUnitStyle::Bytes => (1_024.0, bps / 8.0),
    };
    if !amount.is_finite() || amount <= 0.0 {
        return RateParts {
            value: "0".into(),
            unit: units[0],
        };
    }
    if amount.round() < 1_000.0 {
        return RateParts {
            value: format!("{}", amount.round()),
            unit: units[0],
        };
    }
    // Round to one decimal first; if that reaches 1000, use the next unit, so the value never exceeds 3 integer digits (except in the largest unit).
    let mut divisor = 1.0;
    for (index, &unit) in units.iter().enumerate().skip(1) {
        divisor *= base;
        let scaled = (amount / divisor * 10.0).round() / 10.0;
        if scaled < 1_000.0 || index == units.len() - 1 {
            return RateParts {
                value: format!("{scaled:.1}"),
                unit,
            };
        }
    }
    unreachable!("the largest unit always matches")
}

/// "↓ 12.3 Mbps ↑ 1.2 Mbps" (for the system tray and the compact window).
pub fn rate_summary(sample: &TrafficSample, style: RateUnitStyle) -> String {
    format!(
        "↓ {} ↑ {}",
        format_rate(sample.in_bps, style),
        format_rate(sample.out_bps, style)
    )
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    #[test]
    fn formats_bits_with_decimal_base() {
        let cases = [
            (0.0, "0 bps"),
            (999.0, "999 bps"),
            (999.6, "1.0 Kbps"),
            (1_000.0, "1.0 Kbps"),
            (12_340.0, "12.3 Kbps"),
            (999_949.0, "999.9 Kbps"),
            (999_950.0, "1.0 Mbps"),
            (12_345_678.0, "12.3 Mbps"),
            (1_200_000.0, "1.2 Mbps"),
            (950_000_000.0, "950.0 Mbps"),
            (1_000_000_000.0, "1.0 Gbps"),
            (2_500_000_000.0, "2.5 Gbps"),
            (12_000_000_000_000.0, "12000.0 Gbps"),
        ];
        for (bps, expected) in cases {
            assert_eq!(format_rate(bps, RateUnitStyle::Bits), expected, "{bps}");
        }
    }

    #[test]
    fn negative_and_non_finite_are_zero() {
        for bps in [-5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(format_rate(bps, RateUnitStyle::Bits), "0 bps");
            assert_eq!(format_rate(bps, RateUnitStyle::Bytes), "0 B/s");
        }
    }

    #[test]
    fn formats_bytes_with_binary_base() {
        let cases = [
            (0.0, "0 B/s"),
            (7_992.0, "999 B/s"),
            (8_000.0, "1.0 KB/s"), // 1000 B/s ≈ 0.98 KB/s
            (8_192.0, "1.0 KB/s"), // 1024 B/s
            (8_191_508.0, "999.9 KB/s"),
            (8_388_528.0, "1.0 MB/s"),      // 1023.99 KB/s rounds up
            (103_179_878.4, "12.3 MB/s"),   // 12.3 × 1024² B/s
            (21_474_836_480.0, "2.5 GB/s"), // 2.5 × 1024³ B/s
        ];
        for (bps, expected) in cases {
            assert_eq!(format_rate(bps, RateUnitStyle::Bytes), expected, "{bps}");
        }
    }

    #[test]
    fn parts_separate_value_and_unit() {
        let cases = [
            (0.0, "0", "bps"),
            (999.0, "999", "bps"),
            (12_345_678.0, "12.3", "Mbps"),
            (999_950.0, "1.0", "Mbps"),
            (2_500_000_000.0, "2.5", "Gbps"),
        ];
        for (bps, value, unit) in cases {
            assert_eq!(
                rate_parts(bps, RateUnitStyle::Bits),
                RateParts {
                    value: value.into(),
                    unit
                }
            );
        }
    }

    #[test]
    fn units_for_each_style() {
        assert_eq!(
            rate_units(RateUnitStyle::Bits),
            ["bps", "Kbps", "Mbps", "Gbps"]
        );
        assert_eq!(
            rate_units(RateUnitStyle::Bytes),
            ["B/s", "KB/s", "MB/s", "GB/s"]
        );
        assert_eq!(RateUnitStyle::default(), RateUnitStyle::Bits);
    }

    #[test]
    fn summary_shows_download_then_upload() {
        let sample = |in_bps, out_bps| TrafficSample {
            timestamp: SystemTime::now(),
            in_bps,
            out_bps,
            device_uptime: None,
        };
        assert_eq!(
            rate_summary(&sample(12_345_678.0, 1_234_567.0), RateUnitStyle::Bits),
            "↓ 12.3 Mbps ↑ 1.2 Mbps"
        );
        assert_eq!(
            rate_summary(&sample(103_179_878.4, 8_192.0), RateUnitStyle::Bytes),
            "↓ 12.3 MB/s ↑ 1.0 KB/s"
        );
    }
}
