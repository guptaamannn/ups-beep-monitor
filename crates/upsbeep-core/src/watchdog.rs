use std::time::{Duration, Instant};

/// Peak amplitude below which a block counts as digital silence (muted or dead mic).
const SILENT_PEAK: f32 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicEvent {
    /// Audio is flowing but has been silent for a long time.
    Silent,
    /// No audio input could be opened for a long time.
    Down,
    /// A previously reported problem is gone.
    Recovered,
}

/// Notices when the microphone stops working, since a dead mic means no protection.
pub struct MicWatchdog {
    silent_after: Duration,
    down_after: Duration,
    silent_since: Option<Instant>,
    down_since: Option<Instant>,
    reported: bool,
}

impl MicWatchdog {
    pub fn new(silent_after: Duration, down_after: Duration) -> Self {
        Self {
            silent_after,
            down_after,
            silent_since: None,
            down_since: None,
            reported: false,
        }
    }

    /// Call for every audio block with its peak amplitude.
    pub fn block(&mut self, now: Instant, peak: f32) -> Option<MicEvent> {
        self.down_since = None;
        if peak >= SILENT_PEAK {
            self.silent_since = None;
            return self.recover();
        }
        let since = *self.silent_since.get_or_insert(now);
        if !self.reported && now.duration_since(since) >= self.silent_after {
            self.reported = true;
            return Some(MicEvent::Silent);
        }
        None
    }

    /// Call while no audio stream is open.
    pub fn down(&mut self, now: Instant) -> Option<MicEvent> {
        self.silent_since = None;
        let since = *self.down_since.get_or_insert(now);
        if !self.reported && now.duration_since(since) >= self.down_after {
            self.reported = true;
            return Some(MicEvent::Down);
        }
        None
    }

    fn recover(&mut self) -> Option<MicEvent> {
        if self.reported {
            self.reported = false;
            return Some(MicEvent::Recovered);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dog() -> MicWatchdog {
        MicWatchdog::new(Duration::from_secs(600), Duration::from_secs(60))
    }

    #[test]
    fn normal_audio_never_warns() {
        let (mut d, t0) = (dog(), Instant::now());
        for i in 0..100 {
            assert_eq!(d.block(t0 + Duration::from_secs(i * 60), 0.01), None);
        }
    }

    #[test]
    fn long_silence_warns_once_then_recovers() {
        let (mut d, t0) = (dog(), Instant::now());
        assert_eq!(d.block(t0, 0.0), None);
        assert_eq!(
            d.block(t0 + Duration::from_secs(600), 0.0),
            Some(MicEvent::Silent)
        );
        assert_eq!(d.block(t0 + Duration::from_secs(700), 0.0), None);
        assert_eq!(
            d.block(t0 + Duration::from_secs(701), 0.05),
            Some(MicEvent::Recovered)
        );
        assert_eq!(d.block(t0 + Duration::from_secs(702), 0.05), None);
    }

    #[test]
    fn missing_device_warns_after_a_minute() {
        let (mut d, t0) = (dog(), Instant::now());
        assert_eq!(d.down(t0), None);
        assert_eq!(d.down(t0 + Duration::from_secs(59)), None);
        assert_eq!(d.down(t0 + Duration::from_secs(60)), Some(MicEvent::Down));
        assert_eq!(d.down(t0 + Duration::from_secs(90)), None);
        assert_eq!(
            d.block(t0 + Duration::from_secs(91), 0.05),
            Some(MicEvent::Recovered)
        );
    }

    #[test]
    fn brief_noise_resets_the_silence_timer() {
        let (mut d, t0) = (dog(), Instant::now());
        d.block(t0, 0.0);
        d.block(t0 + Duration::from_secs(500), 0.05);
        assert_eq!(d.block(t0 + Duration::from_secs(700), 0.0), None);
    }
}
