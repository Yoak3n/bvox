//! Shuffle playback order bookkeeping.

use super::App;
use super::PlayRequest;

/// Small xorshift PRNG for shuffle order. Avoids a dependency for the one
/// thing the core needs randomness for.
pub(super) struct ShuffleRng(u64);

impl ShuffleRng {
    pub(super) fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self((nanos ^ ((std::process::id() as u64) << 32)).max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, upper: usize) -> usize {
        if upper == 0 {
            0
        } else {
            (self.next_u64() % upper as u64) as usize
        }
    }

    pub(super) fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let other = self.below(index + 1);
            items.swap(index, other);
        }
    }
}
impl App {
    pub(super) fn step_shuffle(&mut self, delta: isize) -> Option<PlayRequest> {
        if delta > 0 {
            let next = self.next_shuffle_index()?;
            self.start_queue_index(next, true)
        } else {
            let Some(previous) = self.previous_shuffle_index() else {
                self.set_status("没有上一首了", false);
                return None;
            };
            self.start_queue_index(previous, false)
        }
    }
}
impl App {
    /// Rebuild the shuffle bag around the current track. Called when the
    /// queue changes or the user switches to shuffle mode.
    pub fn reset_shuffle(&mut self) {
        self.shuffle_history.clear();
        self.shuffle_remaining.clear();
        if self.results.is_empty() {
            return;
        }
        self.shuffle_remaining = (0..self.results.len())
            .filter(|index| Some(*index) != self.current)
            .collect();
        self.shuffle_rng.shuffle(&mut self.shuffle_remaining);
    }

    /// Rebuild the current-pass bag, excluding the track that is playing.
    fn refill_shuffle(&mut self) {
        self.shuffle_remaining = (0..self.results.len())
            .filter(|index| Some(*index) != self.current)
            .collect();
        self.shuffle_rng.shuffle(&mut self.shuffle_remaining);
    }

    pub(super) fn next_shuffle_index(&mut self) -> Option<usize> {
        let len = self.results.len();
        if len == 0 {
            return None;
        }
        if len == 1 {
            return Some(0);
        }
        loop {
            if self.shuffle_remaining.is_empty() {
                self.refill_shuffle();
            }
            let index = self.shuffle_remaining.pop()?;
            if index < len && Some(index) != self.current {
                return Some(index);
            }
        }
    }

    pub(super) fn previous_shuffle_index(&mut self) -> Option<usize> {
        while let Some(index) = self.shuffle_history.pop() {
            if index < self.results.len() && Some(index) != self.current {
                if let Some(current) = self.current {
                    self.shuffle_remaining.push(current);
                }
                return Some(index);
            }
        }
        None
    }
}
