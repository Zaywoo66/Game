use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use strategy_app::lan::LanMetrics;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(60);

pub(super) struct LanLog {
    file: File,
    active: Option<ActiveSession>,
}

struct ActiveSession {
    role: &'static str,
    started: Instant,
    next_sample: Instant,
    tick: u64,
    metrics: LanMetrics,
}

impl LanLog {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let mut log = Self { file, active: None };
        log.record("log_open", "")?;
        Ok(log)
    }

    pub(super) fn start(&mut self, role: &'static str, tick: u64) {
        let now = Instant::now();
        self.active = Some(ActiveSession {
            role,
            started: now,
            next_sample: now + SAMPLE_INTERVAL,
            tick,
            metrics: LanMetrics::default(),
        });
        self.report("session_start", &format!("role={role} tick={tick}"));
    }

    pub(super) fn observe(&mut self, now: Instant, tick: u64, metrics: LanMetrics) {
        let Some(session) = &mut self.active else {
            return;
        };
        session.tick = tick;
        session.metrics = metrics;
        if now >= session.next_sample {
            session.next_sample = now + SAMPLE_INTERVAL;
            let fields = session.fields(now);
            self.report("minute", &fields);
        }
    }

    pub(super) fn finish(&mut self, reason: &str) {
        if let Some(session) = self.active.take() {
            let fields = session.fields(Instant::now());
            self.report("session_summary", &format!("reason={reason} {fields}"));
            println!("LAN session summary ({reason})\n{fields}");
        }
    }

    pub(super) fn exit(&mut self, code: i32) {
        self.report("app_exit", &format!("exit_code={code}"));
    }

    fn report(&mut self, event: &str, fields: &str) {
        if let Err(error) = self.record(event, fields) {
            eprintln!("LAN acceptance log write failed: {error}");
        }
    }

    fn record(&mut self, event: &str, fields: &str) -> io::Result<()> {
        let utc_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        writeln!(self.file, "unix_ms={utc_ms} event={event} {fields}")?;
        self.file.flush()
    }
}

impl ActiveSession {
    fn fields(&self, now: Instant) -> String {
        let tick = self.tick;
        let metrics = self.metrics;
        let checksum = metrics.last_checksum.map_or_else(
            || "checksum_tick=none checksum=none".to_owned(),
            |(tick, hash)| {
                let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
                format!("checksum_tick={tick} checksum={hex}")
            },
        );
        let ms = |value: Option<u64>| value.map_or_else(|| "none".to_owned(), |ms| ms.to_string());
        format!(
            "role={} elapsed_seconds={} tick={tick} checksums={} mismatches={} resyncs={} {checksum} ping_samples={} ping_min_ms={} ping_mean_ms={} ping_max_ms={}",
            self.role, now.duration_since(self.started).as_secs(), metrics.checksums,
            metrics.mismatches, metrics.resyncs, metrics.ping_samples, ms(metrics.ping_min_ms),
            ms(metrics.ping_mean_ms), ms(metrics.ping_max_ms),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_records_minute_sample_and_exit_without_overwriting_existing_file() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "strategy-lan-log-{}-{timestamp}",
            std::process::id()
        ));
        let mut log = LanLog::open(&path).unwrap();
        log.start("guest", 0);
        let start = log.active.as_ref().unwrap().started;
        let metrics = LanMetrics {
            checksums: 3,
            resyncs: 1,
            ping_samples: 4,
            ping_mean_ms: Some(5),
            last_checksum: Some((90, [0xab; 32])),
            ..LanMetrics::default()
        };
        log.observe(start + Duration::from_secs(59), 89, metrics);
        log.observe(start + SAMPLE_INTERVAL, 90, metrics);
        log.finish("app_exit");
        log.exit(0);
        drop(log);
        assert!(
            matches!(LanLog::open(&path), Err(error) if error.kind() == io::ErrorKind::AlreadyExists)
        );
        let output = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(output.matches("event=minute ").count(), 1);
        assert!(output.contains("checksum_tick=90 checksum="));
        assert!(output.contains(&"ab".repeat(32)));
        assert!(output.contains("resyncs=1"));
        assert!(output.contains("ping_mean_ms=5"));
        assert!(output.contains("event=session_summary reason=app_exit"));
        assert!(output.contains("event=app_exit exit_code=0"));
    }
}
