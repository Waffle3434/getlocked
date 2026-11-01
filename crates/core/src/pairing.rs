//! Pairing between the app and the browser extension.
//!
//! Both sides share a per-install pairing code. When the extension connects,
//! each side proves it knows the code without sending it: they swap random
//! nonces and each returns an HMAC-SHA256 of the nonces under the code. The
//! proofs are bound to the port the extension dialled, so a program squatting
//! a lower port can't relay a real handshake through to the real app.
//!
//! Mirrored in TypeScript by `extension/src/pairing.ts`; the test vector below
//! is checked on both sides.

use std::io;
use std::path::Path;

use hmac::{Hmac, KeyInit, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::persist::{load_json, save_json};

/// Lowercase letters and digits without look-alikes (no 0/o, 1/l/i), so the
/// code is easy to type by hand. 31 symbols, just under 5 bits each.
const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
/// 24 symbols ≈ 119 bits: far beyond guessing, short enough to type.
pub const CODE_LEN: usize = 24;

/// Which side a proof comes from, so one side's proof can't be replayed as
/// the other's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    App,
    Extension,
}

impl Side {
    fn label(self) -> &'static str {
        match self {
            Side::App => "getlocked-app",
            Side::Extension => "getlocked-ext",
        }
    }
}

/// A new random pairing code, already normalized.
pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    (0..CODE_LEN).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect()
}

/// Lowercases and drops everything that isn't a code symbol (dashes, spaces),
/// so `K7QP-X2RM …` typed by hand matches. Returns `None` if the result
/// isn't a full code.
pub fn normalize_code(input: &str) -> Option<String> {
    let code: String = input.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let valid = code.len() == CODE_LEN && code.bytes().all(|b| ALPHABET.contains(&b));
    valid.then_some(code)
}

/// `abcd-efgh-…`: easier to read and type.
pub fn format_code(code: &str) -> String {
    code.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join("-")
}

/// `pairing.json` in the app data folder. The extension folder the app
/// prepares gets a copy too, so in-app setup pairs without typing.
#[derive(Debug, Serialize, Deserialize)]
pub struct PairingFile {
    pub code: String,
}

/// This install's pairing code, created on first run (or if the file is
/// missing or damaged).
pub fn load_or_create(path: &Path) -> io::Result<String> {
    if let Ok(Some(f)) = load_json::<PairingFile>(path) {
        if let Some(code) = normalize_code(&f.code) {
            return Ok(code);
        }
    }
    let code = generate_code();
    save_json(path, &PairingFile { code: code.clone() })?;
    Ok(code)
}

/// A random hex nonce for one handshake.
pub fn nonce() -> String {
    let bytes: [u8; 16] = rand::thread_rng().gen();
    hex::encode(bytes)
}

fn mac(code: &str, side: Side, app_nonce: &str, ext_nonce: &str, port: u16) -> Hmac<Sha256> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(code.as_bytes()).expect("HMAC accepts any key length");
    mac.update(format!("{}|{app_nonce}|{ext_nonce}|{port}", side.label()).as_bytes());
    mac
}

/// The hex proof `side` sends for this handshake.
pub fn proof(code: &str, side: Side, app_nonce: &str, ext_nonce: &str, port: u16) -> String {
    hex::encode(mac(code, side, app_nonce, ext_nonce, port).finalize().into_bytes())
}

/// Checks a proof in constant time.
pub fn verify(code: &str, side: Side, app_nonce: &str, ext_nonce: &str, port: u16, proof_hex: &str) -> bool {
    let Ok(bytes) = hex::decode(proof_hex) else { return false };
    mac(code, side, app_nonce, ext_nonce, port).verify_slice(&bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CODE: &str = "k7qpx2rm3n4b5c6d7e8f9g2h";

    #[test]
    fn matches_the_shared_test_vector() {
        // Same values as extension/src/pairing.test.ts.
        assert_eq!(
            proof(CODE, Side::Extension, "0011aabb", "ccdd2233", 47613),
            "6d4d1d32b7a25926213559f1ee1913a0a9c037d2873c8e859579e7a864423bcb"
        );
        assert_eq!(
            proof(CODE, Side::App, "0011aabb", "ccdd2233", 47613),
            "2872337fe1d08fc0ac3d73657f6c17354c7ec62a3e38b3b102f88fa4698eeb9d"
        );
    }

    #[test]
    fn verify_rejects_wrong_code_side_port_or_garbage() {
        let p = proof(CODE, Side::Extension, "aa", "bb", 47613);
        assert!(verify(CODE, Side::Extension, "aa", "bb", 47613, &p));
        assert!(!verify("x".repeat(CODE_LEN).as_str(), Side::Extension, "aa", "bb", 47613, &p));
        assert!(!verify(CODE, Side::App, "aa", "bb", 47613, &p));
        // Bound to the dialled port: a relayed handshake fails.
        assert!(!verify(CODE, Side::Extension, "aa", "bb", 47614, &p));
        assert!(!verify(CODE, Side::Extension, "aa", "bc", 47613, &p));
        assert!(!verify(CODE, Side::Extension, "aa", "bb", 47613, "not hex"));
    }

    #[test]
    fn load_or_create_keeps_the_code_and_replaces_a_bad_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pairing.json");
        let code = load_or_create(&path).unwrap();
        assert_eq!(load_or_create(&path).unwrap(), code);
        std::fs::write(&path, r#"{"code":"nope"}"#).unwrap();
        let fresh = load_or_create(&path).unwrap();
        assert_ne!(fresh, code);
        assert_eq!(normalize_code(&fresh).as_deref(), Some(fresh.as_str()));
    }

    #[test]
    fn codes_are_valid_and_normalize_from_typed_form() {
        let code = generate_code();
        assert_eq!(normalize_code(&code).as_deref(), Some(code.as_str()));
        assert_ne!(code, generate_code());
        let typed = format_code(CODE).to_uppercase().replace('-', " - ");
        assert_eq!(normalize_code(&typed).as_deref(), Some(CODE));
        assert_eq!(format_code(CODE), "k7qp-x2rm-3n4b-5c6d-7e8f-9g2h");
        assert_eq!(normalize_code("too-short"), None);
        // '0', 'o', '1', 'l', 'i' never appear in a code.
        assert_eq!(normalize_code(&"0".repeat(CODE_LEN)), None);
    }
}
