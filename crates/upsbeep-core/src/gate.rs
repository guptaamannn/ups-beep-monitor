use std::time::{Duration, Instant};

/// Turns per-block "tone present" flags into one event per beep.
///
/// A beep fires once the tone has lasted `min_on`. Short gaps (under `min_gap`) inside a
/// burst don't split it, and a burst shorter than `min_on` (a click) never fires.
pub struct BeepGate {
    min_on: Duration,
    min_gap: Duration,
    run_start: Option<Instant>,
    last_active: Option<Instant>,
    fired: bool,
}

impl BeepGate {
    pub fn new(min_on: Duration, min_gap: Duration) -> Self {
        Self {
            min_on,
            min_gap,
            run_start: None,
            last_active: None,
            fired: false,
        }
    }

    /// Returns `true` exactly once per beep, when it has lasted long enough.
    pub fn update(&mut self, now: Instant, active: bool) -> bool {
        if active {
            let start = *self.run_start.get_or_insert(now);
            self.last_active = Some(now);
            if !self.fired && now.duration_since(start) >= self.min_on {
                self.fired = true;
                return true;
            }
        } else if let Some(last) = self.last_active
            && now.duration_since(last) >= self.min_gap
        {
            self.run_start = None;
            self.last_active = None;
            self.fired = false;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: Duration = Duration::from_millis(50);

    /// Feeds a pattern of blocks ('#' = tone, '.' = silence); returns the fired count.
    fn run(gate: &mut BeepGate, t0: Instant, offset: u32, pattern: &str) -> usize {
        pattern
            .chars()
            .enumerate()
            .filter(|(i, c)| gate.update(t0 + BLOCK * (offset + *i as u32), *c == '#'))
            .count()
    }

    fn gate() -> BeepGate {
        BeepGate::new(Duration::from_millis(100), Duration::from_millis(200))
    }

    #[test]
    fn click_is_rejected() {
        assert_eq!(run(&mut gate(), Instant::now(), 0, "#........."), 0);
    }

    #[test]
    fn long_tone_fires_once() {
        assert_eq!(run(&mut gate(), Instant::now(), 0, "##########...."), 1);
    }

    #[test]
    fn two_bursts_fire_twice() {
        assert_eq!(
            run(&mut gate(), Instant::now(), 0, "#####.........#####"),
            2
        );
    }

    #[test]
    fn short_dropout_does_not_split_a_burst() {
        assert_eq!(run(&mut gate(), Instant::now(), 0, "####.####"), 1);
    }
}
