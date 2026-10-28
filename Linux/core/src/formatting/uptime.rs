use std::time::Duration;

/// Uptime units, largest to smallest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UptimeUnit {
    Days,
    Hours,
    Minutes,
}

/// One uptime unit and its value, e.g. 12 days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UptimePart {
    pub value: u64,
    pub unit: UptimeUnit,
}

/// Splits uptime into at most two units (days, hours, minutes; rounded down; units with a value of 0 are omitted);
/// under a minute it's "0 minutes". The app composes the text ("12 days, 5 hours" / "12天5小時").
pub fn uptime_parts(duration: Duration) -> Vec<UptimePart> {
    let seconds = duration.as_secs();
    let all = [
        UptimePart {
            value: seconds / 86_400,
            unit: UptimeUnit::Days,
        },
        UptimePart {
            value: seconds % 86_400 / 3_600,
            unit: UptimeUnit::Hours,
        },
        UptimePart {
            value: seconds % 3_600 / 60,
            unit: UptimeUnit::Minutes,
        },
    ];
    let parts: Vec<_> = all
        .into_iter()
        .filter(|part| part.value > 0)
        .take(2)
        .collect();
    if parts.is_empty() {
        vec![UptimePart {
            value: 0,
            unit: UptimeUnit::Minutes,
        }]
    } else {
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(seconds: u64) -> Vec<(u64, UptimeUnit)> {
        uptime_parts(Duration::from_secs(seconds))
            .iter()
            .map(|p| (p.value, p.unit))
            .collect()
    }

    #[test]
    fn at_most_two_non_zero_units_rounded_down() {
        use UptimeUnit::*;
        assert_eq!(parts(0), [(0, Minutes)]);
        assert_eq!(parts(59), [(0, Minutes)]);
        assert_eq!(parts(60), [(1, Minutes)]);
        assert_eq!(parts(3_660), [(1, Hours), (1, Minutes)]);
        assert_eq!(parts(86_399), [(23, Hours), (59, Minutes)]); // Never rounds up to 1 day
        assert_eq!(
            parts(86_400 * 12 + 3_600 * 5 + 120),
            [(12, Days), (5, Hours)]
        );
        assert_eq!(parts(86_400 * 3 + 120), [(3, Days), (2, Minutes)]);
        assert_eq!(parts(86_400), [(1, Days)]);
    }

    #[test]
    fn sys_up_time_ticks_are_hundredths_of_a_second() {
        // Polling converts sysUpTime 366000 (hundredths of a second) to 3660 seconds.
        assert_eq!(
            uptime_parts(Duration::from_millis(366_000 * 10)),
            uptime_parts(Duration::from_secs(3_660))
        );
    }
}
