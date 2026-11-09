//! Mediciones reales de la conexión para mostrar en pantalla.

use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rate {
    pub per_second: u32,
    pub megabits_per_second: f32,
}

/// Cuenta lo recibido y da un resultado una vez por ventana de tiempo.
#[derive(Default)]
pub struct RateMeter {
    started: Option<Instant>,
    packets: u32,
    bytes: u64,
}

impl RateMeter {
    pub fn record(&mut self, now: Instant, bytes: usize) -> Option<Rate> {
        let started = *self.started.get_or_insert(now);
        self.packets += 1;
        self.bytes += bytes as u64;

        let elapsed = now.duration_since(started);
        if elapsed < WINDOW {
            return None;
        }
        let seconds = elapsed.as_secs_f32();
        let rate = Rate {
            per_second: (self.packets as f32 / seconds).round() as u32,
            megabits_per_second: self.bytes as f32 * 8.0 / 1_000_000.0 / seconds,
        };
        self.started = Some(now);
        self.packets = 0;
        self.bytes = 0;
        Some(rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_reported_until_a_full_window_has_passed() {
        let mut meter = RateMeter::default();
        let start = Instant::now();
        assert_eq!(meter.record(start, 100), None);
        assert_eq!(meter.record(start + Duration::from_millis(500), 100), None);
    }

    #[test]
    fn rates_are_computed_from_what_arrived_in_the_window() {
        let mut meter = RateMeter::default();
        let start = Instant::now();
        for frame in 0..60 {
            let at = start + Duration::from_millis(frame * 1000 / 60);
            assert_eq!(meter.record(at, 12_500), None);
        }
        let rate = meter.record(start + WINDOW, 12_500).unwrap();
        assert_eq!(rate.per_second, 61);
        assert!((rate.megabits_per_second - 6.1).abs() < 0.01);
    }

    #[test]
    fn each_window_starts_from_zero() {
        let mut meter = RateMeter::default();
        let start = Instant::now();
        meter.record(start, 1_000_000);
        meter.record(start + WINDOW, 1_000_000).unwrap();
        let second = meter.record(start + WINDOW * 2, 125_000).unwrap();
        assert_eq!(second.per_second, 1);
        assert!((second.megabits_per_second - 1.0).abs() < 0.01);
    }
}
