use std::time::Instant;

/// Raw counters from one SNMP sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CounterReading {
    pub(crate) in_octets: u64,
    pub(crate) out_octets: u64,
    /// sysUpTime (hundredths of a second); helps detect whether the device restarted.
    pub(crate) sys_up_time: Option<u32>,
    /// Sample time on the local monotonic clock.
    pub(crate) instant: Instant,
}

/// Rate (bits per second).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rate {
    pub(crate) in_bps: f64,
    pub(crate) out_bps: f64,
}

/// Calculates rates from two consecutive counter readings.
///
/// - The first reading has no rate.
/// - Counter or sysUpTime decreased (restart / reset) → discard and use the new value as the baseline, with no spike.
/// - Elapsed time ≤ 0 → ignore the reading.
#[derive(Debug, Clone, Default)]
pub(crate) struct RateCalculator {
    previous: Option<CounterReading>,
}

impl RateCalculator {
    pub(crate) fn add(&mut self, reading: CounterReading) -> Option<Rate> {
        let Some(previous) = self.previous else {
            self.previous = Some(reading);
            return None;
        };
        let seconds = reading
            .instant
            .saturating_duration_since(previous.instant)
            .as_secs_f64();
        if seconds <= 0.0 {
            return None;
        }
        self.previous = Some(reading);

        if let (Some(old), Some(new)) = (previous.sys_up_time, reading.sys_up_time)
            && new < old
        {
            return None;
        }
        let delta_in = reading.in_octets.checked_sub(previous.in_octets)?;
        let delta_out = reading.out_octets.checked_sub(previous.out_octets)?;
        Some(Rate {
            in_bps: delta_in as f64 * 8.0 / seconds,
            out_bps: delta_out as f64 * 8.0 / seconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    struct Fixture {
        t0: Instant,
        calculator: RateCalculator,
    }

    impl Fixture {
        fn new() -> Self {
            Fixture {
                t0: Instant::now(),
                calculator: RateCalculator::default(),
            }
        }

        fn add(
            &mut self,
            in_octets: u64,
            out_octets: u64,
            millis: u64,
            up_time: Option<u32>,
        ) -> Option<Rate> {
            let instant = self.t0 + Duration::from_millis(millis);
            self.calculator.add(CounterReading {
                in_octets,
                out_octets,
                sys_up_time: up_time,
                instant,
            })
        }
    }

    fn rate(in_bps: f64, out_bps: f64) -> Option<Rate> {
        Some(Rate { in_bps, out_bps })
    }

    #[test]
    fn first_sample_has_no_rate() {
        assert_eq!(Fixture::new().add(1000, 1000, 0, None), None);
    }

    #[test]
    fn normal_rate_over_one_second() {
        let mut f = Fixture::new();
        f.add(1_000_000, 500_000, 0, None);
        assert_eq!(
            f.add(2_250_000, 625_000, 1000, None),
            rate(10_000_000.0, 1_000_000.0)
        );
    }

    #[test]
    fn uses_actual_elapsed_time() {
        let mut f = Fixture::new();
        f.add(0, 0, 0, None);
        let first = f.add(300_000, 30_000, 1500, None).unwrap();
        assert!((first.in_bps - 1_600_000.0).abs() < 0.001);
        assert!((first.out_bps - 160_000.0).abs() < 0.001);
        // 100 000 bytes × 8 ÷ 0.8 s
        let next = f.add(400_000, 30_000, 2300, None).unwrap();
        assert!((next.in_bps - 1_000_000.0).abs() < 0.001);
        assert_eq!(next.out_bps, 0.0);
    }

    #[test]
    fn static_counters_give_zero() {
        let mut f = Fixture::new();
        f.add(42, 42, 0, None);
        assert_eq!(f.add(42, 42, 1000, None), rate(0.0, 0.0));
    }

    #[test]
    fn counter_reset_is_discarded_and_rebased() {
        let mut f = Fixture::new();
        f.add(10_000_000, 5_000_000, 0, None);
        // Device restarted: counter decreased
        assert_eq!(f.add(1_000, 500, 1000, None), None);
        // The next reading uses the reset value as the baseline
        assert_eq!(f.add(126_000, 500, 2000, None), rate(1_000_000.0, 0.0));
    }

    #[test]
    fn single_direction_reset_is_discarded() {
        let mut f = Fixture::new();
        f.add(10_000, 10_000, 0, None);
        assert_eq!(f.add(20_000, 5_000, 1000, None), None);
    }

    #[test]
    fn sys_up_time_decrease_is_treated_as_reboot() {
        let mut f = Fixture::new();
        f.add(1_000, 1_000, 0, Some(900_000));
        // The counter seems to increase, but sysUpTime decreased → restarted; the difference can't be trusted
        assert_eq!(f.add(5_000_000, 5_000_000, 1000, Some(300)), None);
        assert_eq!(
            f.add(5_125_000, 5_000_000, 2000, Some(400)),
            rate(1_000_000.0, 0.0)
        );
    }

    #[test]
    fn zero_elapsed_time_is_ignored_and_keeps_baseline() {
        let mut f = Fixture::new();
        f.add(0, 0, 0, None);
        assert_eq!(f.add(1_000, 1_000, 0, None), None);
        assert_eq!(f.add(125_000, 0, 1000, None), rate(1_000_000.0, 0.0));
    }

    #[test]
    fn large_counter_values_do_not_overflow() {
        let mut f = Fixture::new();
        f.add(u64::MAX - 1_000, 1 << 63, 0, None);
        assert_eq!(
            f.add(u64::MAX, (1 << 63) + 125, 1000, None),
            rate(8_000.0, 1_000.0)
        );
    }
}
