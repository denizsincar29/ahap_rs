//! Musical (BPM + time-signature) timing helpers, so callers can address
//! events by bar/beat instead of computing raw seconds by hand.

/// A time signature as numerator/denominator, e.g. 4/4 or 6/8.
#[derive(Debug, Clone, Copy)]
pub struct TimeSignature {
    pub numerator: u32,
    pub denominator: u32,
}

/// BPM plus a time signature - everything needed to convert between
/// bars/beats and seconds.
#[derive(Debug, Clone, Copy)]
pub struct MusicalContext {
    pub bpm: f64,
    pub time_signature: TimeSignature,
}

impl MusicalContext {
    pub fn new(bpm: f64, numerator: u32, denominator: u32) -> Self {
        Self { bpm, time_signature: TimeSignature { numerator, denominator } }
    }

    /// Seconds for a given number of beats (fractional beats allowed) at this BPM.
    pub fn beat_to_seconds(&self, beat: f64) -> f64 {
        beat * (60.0 / self.bpm)
    }

    /// Seconds for a given number of bars (fractional bars allowed), using
    /// the time signature's numerator as beats-per-bar.
    pub fn bar_to_seconds(&self, bar: f64) -> f64 {
        let beats_per_bar = self.time_signature.numerator as f64;
        bar * beats_per_bar * (60.0 / self.bpm)
    }

    /// Duration of a single beat in seconds.
    pub fn beat_duration(&self) -> f64 {
        self.beat_to_seconds(1.0)
    }

    /// Duration of a single bar in seconds.
    pub fn bar_duration(&self) -> f64 {
        self.bar_to_seconds(1.0)
    }

    /// Beats per bar (the time signature's numerator).
    pub fn beats_per_bar(&self) -> u32 {
        self.time_signature.numerator
    }

    /// Inverse of [`Self::beat_to_seconds`].
    pub fn seconds_to_beats(&self, seconds: f64) -> f64 {
        seconds / (60.0 / self.bpm)
    }

    /// Inverse of [`Self::bar_to_seconds`].
    pub fn seconds_to_bars(&self, seconds: f64) -> f64 {
        let beats_per_bar = self.time_signature.numerator as f64;
        seconds / (beats_per_bar * (60.0 / self.bpm))
    }

    /// Convenience matching the Go builder's `At(bar, beat)`: absolute seconds
    /// for a given (0-indexed) bar and beat.
    pub fn at(&self, bar: i64, beat: i64) -> f64 {
        let total_beats = (bar * self.beats_per_bar() as i64 + beat) as f64;
        self.beat_to_seconds(total_beats)
    }
}
