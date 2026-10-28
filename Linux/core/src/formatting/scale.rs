use super::RateUnitStyle;

/// Minimum gauge maximum: 1 Kbps.
pub const MINIMUM_RATE_SCALE: f64 = 1_000.0;

/// Gauge maximum: rounded up to the 1-2-5 series (1, 2, 5, 10, 20, 50…), at least 1 Kbps, in bits.
pub fn nice_ceiling(bps: f64) -> f64 {
    if !bps.is_finite() || bps <= MINIMUM_RATE_SCALE {
        return MINIMUM_RATE_SCALE;
    }
    let (step, magnitude) = nice_step(bps);
    step * magnitude
}

/// Chart vertical axis: maximum and ticks, all in bps.
#[derive(Debug, Clone, PartialEq)]
pub struct RateAxis {
    pub maximum: f64,
    /// From 0 to `maximum`.
    pub ticks: Vec<f64>,
}

/// Axis rounded in the display unit: the maximum and ticks are round numbers in that unit.
/// E.g. a byte-mode peak of 3.7 MB/s → maximum 5 MB/s, a tick every 1 MB/s.
pub fn rate_axis(peak_bps: f64, style: RateUnitStyle) -> RateAxis {
    let (base, bits_per_amount) = match style {
        RateUnitStyle::Bits => (1_000.0, 1.0),
        RateUnitStyle::Bytes => (1_024.0, 8.0),
    };
    // The peak in the display unit (starting at Kbps or KB/s).
    let peak = if peak_bps.is_finite() {
        peak_bps.max(0.0)
    } else {
        0.0
    };
    let mut unit = base;
    let mut amount = peak / bits_per_amount / unit;
    while amount >= 1_000.0 {
        unit *= base;
        amount /= base;
    }
    let (step, magnitude) = nice_step(amount.max(1.0));
    let maximum = step * magnitude;
    // 1 → two steps, 2 → four steps, 5 → five steps, so ticks land on round numbers.
    let divisions: u32 = match step as u32 {
        1 => 2,
        2 => 4,
        _ => 5,
    };
    let ticks = (0..=divisions)
        .map(|i| maximum * f64::from(i) / f64::from(divisions) * unit * bits_per_amount)
        .collect();
    RateAxis {
        maximum: maximum * unit * bits_per_amount,
        ticks,
    }
}

/// Returns (1, 2 or 5, a power of 10) whose product is the smallest 1-2-5 number ≥ `value`.
fn nice_step(value: f64) -> (f64, f64) {
    let magnitude = 10f64.powi(value.log10().floor() as i32);
    let mantissa = value / magnitude;
    [1.0, 2.0, 5.0]
        .into_iter()
        .find(|step| mantissa <= step * (1.0 + 1e-9))
        .map_or((1.0, magnitude * 10.0), |step| (step, magnitude))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatting::format_rate;

    /// 1 MB/s (binary) in bps.
    const MIB: f64 = 1_048_576.0 * 8.0;

    #[test]
    fn nice_ceiling_follows_1_2_5() {
        let cases = [
            (0.0, 1_000.0),
            (-5.0, 1_000.0),
            (500.0, 1_000.0),
            (1_000.0, 1_000.0),
            (1_001.0, 2_000.0),
            (1_200_000.0, 2_000_000.0),
            (2_000_000.0, 2_000_000.0),
            (3_700_000.0, 5_000_000.0),
            (37_000_000.0, 50_000_000.0),
            (51_000_000.0, 100_000_000.0),
            (999_000_000.0, 1_000_000_000.0),
        ];
        for (value, expected) in cases {
            assert_eq!(nice_ceiling(value), expected, "{value}");
        }
    }

    #[test]
    fn non_finite_falls_back_to_minimum() {
        assert_eq!(nice_ceiling(f64::NAN), 1_000.0);
        assert_eq!(nice_ceiling(f64::INFINITY), 1_000.0);
    }

    #[test]
    fn bits_axis_uses_decimal_steps() {
        let axis = rate_axis(37_000_000.0, RateUnitStyle::Bits);
        assert_eq!(axis.maximum, 50_000_000.0);
        assert_eq!(
            axis.ticks,
            [
                0.0,
                10_000_000.0,
                20_000_000.0,
                30_000_000.0,
                40_000_000.0,
                50_000_000.0
            ]
        );
    }

    #[test]
    fn bits_axis_for_two_step() {
        let axis = rate_axis(1_200_000.0, RateUnitStyle::Bits);
        assert_eq!(axis.maximum, 2_000_000.0);
        assert_eq!(
            axis.ticks,
            [0.0, 500_000.0, 1_000_000.0, 1_500_000.0, 2_000_000.0]
        );
    }

    #[test]
    fn bits_axis_for_one_step() {
        let axis = rate_axis(800_000.0, RateUnitStyle::Bits);
        assert_eq!(axis.maximum, 1_000_000.0);
        assert_eq!(axis.ticks, [0.0, 500_000.0, 1_000_000.0]);
    }

    #[test]
    fn bytes_axis_rounds_in_megabytes() {
        // Peak exactly 2.0 MB/s → maximum 2 MB/s, a tick every 0.5 MB/s
        let axis = rate_axis(2.0 * MIB, RateUnitStyle::Bytes);
        assert_eq!(axis.maximum, 2.0 * MIB);
        assert_eq!(
            axis.ticks,
            [0.0, 0.5 * MIB, 1.0 * MIB, 1.5 * MIB, 2.0 * MIB]
        );
        let labels: Vec<_> = axis
            .ticks
            .iter()
            .map(|&t| format_rate(t, RateUnitStyle::Bytes))
            .collect();
        assert_eq!(
            labels,
            ["0 B/s", "512.0 KB/s", "1.0 MB/s", "1.5 MB/s", "2.0 MB/s"]
        );
    }

    #[test]
    fn bytes_axis_labels_are_round() {
        // Peak 3.7 MB/s → maximum 5 MB/s, ticks at 1, 2, 3, 4, 5 MB/s
        let axis = rate_axis(3.7 * MIB, RateUnitStyle::Bytes);
        let labels: Vec<_> = axis
            .ticks
            .iter()
            .map(|&t| format_rate(t, RateUnitStyle::Bytes))
            .collect();
        assert_eq!(
            labels,
            [
                "0 B/s", "1.0 MB/s", "2.0 MB/s", "3.0 MB/s", "4.0 MB/s", "5.0 MB/s"
            ]
        );
    }

    #[test]
    fn minimum_axis() {
        assert_eq!(rate_axis(0.0, RateUnitStyle::Bits).maximum, 1_000.0);
        assert_eq!(rate_axis(0.0, RateUnitStyle::Bytes).maximum, 8_192.0); // 1 KB/s
        assert_eq!(rate_axis(f64::NAN, RateUnitStyle::Bytes).maximum, 8_192.0);
    }
}
