//! Unlock codes for ending a session early.
//!
//! The code is long, random and regenerated on every attempt, so it has to be
//! read and typed by hand. The UI also blocks paste, but the check happens
//! here, not in the UI.

use rand::Rng;

/// Seconds the user must wait before the code is shown.
pub const COOLDOWN_SECS: u64 = 30;
const GROUPS: usize = 8;
const GROUP_LEN: usize = 4;
/// No 0/O, 1/l/I: avoids "is that a zero?" frustration.
const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";

pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    (0..GROUPS)
        .map(|_| (0..GROUP_LEN).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// Exact match, ignoring surrounding whitespace. Case matters on purpose.
pub fn code_matches(expected: &str, typed: &str) -> bool {
    expected == typed.trim()
}

/// An in-progress unlock attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockChallenge {
    pub code: String,
    /// Monotonic milliseconds at which the code may be shown and used.
    pub ready_at_ms: u64,
}

impl UnlockChallenge {
    pub fn new(now_ms: u64) -> Self {
        Self { code: generate_code(), ready_at_ms: now_ms + COOLDOWN_SECS * 1000 }
    }

    pub fn secs_left(&self, now_ms: u64) -> u64 {
        self.ready_at_ms.saturating_sub(now_ms).div_ceil(1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_has_expected_shape() {
        let code = generate_code();
        assert_eq!(code.len(), GROUPS * GROUP_LEN + GROUPS - 1);
        assert!(code.split('-').all(|g| g.len() == GROUP_LEN));
        assert!(code.chars().filter(|c| *c != '-').all(|c| ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn codes_differ() {
        assert_ne!(generate_code(), generate_code());
    }

    #[test]
    fn matching_is_exact_but_trims() {
        assert!(code_matches("abCD-2345", "  abCD-2345\n"));
        assert!(!code_matches("abCD-2345", "abcd-2345"));
    }

    #[test]
    fn cooldown_counts_down() {
        let c = UnlockChallenge::new(1_000);
        assert_eq!(c.secs_left(1_000), COOLDOWN_SECS);
        assert_eq!(c.secs_left(1_000 + COOLDOWN_SECS * 1000 - 1), 1);
        assert_eq!(c.secs_left(1_000 + COOLDOWN_SECS * 1000), 0);
    }
}
