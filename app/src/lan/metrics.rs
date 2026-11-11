use std::time::{Duration, Instant};

/// Cumulative observations for the lifetime of a host room or guest session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LanMetrics {
    /// Checksums sent by the host or verified by the guest.
    pub checksums: u64,
    /// Guest comparisons that failed. The host does not observe comparisons.
    pub mismatches: u64,
    /// Recovery snapshots sent or installed after the connection handshake.
    pub resyncs: u64,
    pub ping_samples: u64,
    pub ping_min_ms: Option<u64>,
    pub ping_mean_ms: Option<u64>,
    pub ping_max_ms: Option<u64>,
    /// Tick and locally calculated checksum from the latest observation.
    pub last_checksum: Option<(u64, [u8; 32])>,
}

#[derive(Clone, Default)]
pub(super) struct Metrics {
    totals: LanMetrics,
    ping_total_micros: u128,
}

impl Metrics {
    pub(super) fn snapshot(&self) -> LanMetrics {
        self.totals
    }

    pub(super) fn record_checksum(&mut self, tick: u64, hash: [u8; 32], matched: bool) {
        self.totals.last_checksum = Some((tick, hash));
        if matched {
            self.totals.checksums = self.totals.checksums.saturating_add(1);
        } else {
            self.totals.mismatches = self.totals.mismatches.saturating_add(1);
        }
    }

    pub(super) fn record_resync(&mut self) {
        self.totals.resyncs = self.totals.resyncs.saturating_add(1);
    }

    pub(super) fn record_ping(&mut self, elapsed: Duration) {
        let millis = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        self.totals.ping_samples = self.totals.ping_samples.saturating_add(1);
        self.ping_total_micros = self.ping_total_micros.saturating_add(elapsed.as_micros());
        let mean = self.ping_total_micros / u128::from(self.totals.ping_samples) / 1000;
        self.totals.ping_mean_ms = Some(u64::try_from(mean).unwrap_or(u64::MAX));
        self.totals.ping_min_ms = Some(
            self.totals
                .ping_min_ms
                .map_or(millis, |min| min.min(millis)),
        );
        self.totals.ping_max_ms = Some(
            self.totals
                .ping_max_ms
                .map_or(millis, |max| max.max(millis)),
        );
    }
}

#[derive(Default)]
pub(super) struct PingProbe(Option<(u64, Instant)>);

impl PingProbe {
    pub(super) fn start(&mut self, nonce: u64) {
        self.0 = Some((nonce, Instant::now()));
    }

    pub(super) fn complete(&mut self, nonce: u64) -> Option<Duration> {
        let (_, started) = self.0.filter(|(pending, _)| *pending == nonce)?;
        self.0 = None;
        Some(started.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_verified_checksums_mismatches_resyncs_and_ping() {
        let mut metrics = Metrics::default();
        assert_eq!(metrics.snapshot(), LanMetrics::default());
        metrics.record_checksum(30, [1; 32], true);
        metrics.record_checksum(60, [2; 32], false);
        metrics.record_resync();
        metrics.record_ping(Duration::from_micros(1_900));
        metrics.record_ping(Duration::from_micros(2_100));
        assert_eq!(
            metrics.snapshot(),
            LanMetrics {
                checksums: 1,
                mismatches: 1,
                resyncs: 1,
                ping_samples: 2,
                ping_min_ms: Some(1),
                ping_mean_ms: Some(2),
                ping_max_ms: Some(2),
                last_checksum: Some((60, [2; 32])),
            }
        );
    }

    #[test]
    fn a_ping_only_accepts_one_matching_reply() {
        let mut probe = PingProbe::default();
        assert_eq!(probe.complete(7), None);
        probe.start(7);
        assert_eq!(probe.complete(8), None);
        assert!(probe.complete(7).is_some());
        assert_eq!(probe.complete(7), None);
    }
}
