use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Mains,
    OnBattery { since: Instant },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Shutdown,
}

/// Tracks beeps over time. Feed it a boolean per audio block via [`Detector::update`].
pub struct Detector {
    beep_timeout: Duration,
    shutdown_after: Duration,
    state: State,
    last_beep: Option<Instant>,
}

impl Detector {
    pub fn new(beep_timeout: Duration, shutdown_after: Duration) -> Self {
        Self {
            beep_timeout,
            shutdown_after,
            state: State::Mains,
            last_beep: None,
        }
    }

    pub fn set_durations(&mut self, beep_timeout: Duration, shutdown_after: Duration) {
        self.beep_timeout = beep_timeout;
        self.shutdown_after = shutdown_after;
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn update(&mut self, now: Instant, beep: bool) -> Action {
        if beep {
            self.last_beep = Some(now);
            if self.state == State::Mains {
                self.state = State::OnBattery { since: now };
            }
        }
        if let State::OnBattery { since } = self.state {
            let quiet = self
                .last_beep
                .is_none_or(|t| now.duration_since(t) > self.beep_timeout);
            if quiet {
                self.state = State::Mains;
            } else if now.duration_since(since) >= self.shutdown_after {
                return Action::Shutdown;
            }
        }
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuts_down_after_sustained_beeps() {
        let t0 = Instant::now();
        let mut d = Detector::new(Duration::from_secs(60), Duration::from_secs(120));
        assert_eq!(d.update(t0, true), Action::None);
        assert_eq!(d.update(t0 + Duration::from_secs(30), true), Action::None);
        assert_eq!(
            d.update(t0 + Duration::from_secs(120), true),
            Action::Shutdown
        );
    }

    #[test]
    fn recovers_when_beeps_stop() {
        let t0 = Instant::now();
        let mut d = Detector::new(Duration::from_secs(60), Duration::from_secs(120));
        d.update(t0, true);
        assert_eq!(d.update(t0 + Duration::from_secs(90), false), Action::None);
        assert_eq!(d.state(), State::Mains);
    }
}
