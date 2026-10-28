use super::TrafficSample;

/// Average and peak (bps) over a set of samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrafficStatistics {
    pub average_in: f64,
    pub average_out: f64,
    pub peak_in: f64,
    pub peak_out: f64,
}

impl TrafficStatistics {
    /// Returns `None` when there are no samples.
    pub fn from_samples<'a>(samples: impl IntoIterator<Item = &'a TrafficSample>) -> Option<Self> {
        let mut count = 0usize;
        let mut statistics = TrafficStatistics {
            average_in: 0.0,
            average_out: 0.0,
            peak_in: f64::MIN,
            peak_out: f64::MIN,
        };
        for sample in samples {
            count += 1;
            statistics.average_in += sample.in_bps;
            statistics.average_out += sample.out_bps;
            statistics.peak_in = statistics.peak_in.max(sample.in_bps);
            statistics.peak_out = statistics.peak_out.max(sample.out_bps);
        }
        if count == 0 {
            return None;
        }
        statistics.average_in /= count as f64;
        statistics.average_out /= count as f64;
        Some(statistics)
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    fn sample(in_bps: f64, out_bps: f64) -> TrafficSample {
        TrafficSample {
            timestamp: SystemTime::now(),
            in_bps,
            out_bps,
            device_uptime: None,
        }
    }

    #[test]
    fn empty_has_no_statistics() {
        assert_eq!(TrafficStatistics::from_samples(&[]), None);
    }

    #[test]
    fn average_and_peak() {
        let samples = [sample(10.0, 1.0), sample(30.0, 5.0), sample(20.0, 3.0)];
        assert_eq!(
            TrafficStatistics::from_samples(&samples),
            Some(TrafficStatistics {
                average_in: 20.0,
                average_out: 3.0,
                peak_in: 30.0,
                peak_out: 5.0
            })
        );
    }
}
