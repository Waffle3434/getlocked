//! Platform-independent core of GetLocked.
//!
//! Everything in here is pure logic or plain file/SQLite I/O, so it can be
//! unit-tested without Tauri or any OS-specific code. The desktop app wires
//! these pieces to the real clock, the process enforcer and the UI.

pub mod end_policy;
pub mod matcher;
pub mod model;
pub mod pairing;
pub mod persist;
pub mod protocol;
pub mod scheduler;
pub mod session;
pub mod sites;
pub mod stats;
pub mod tasks;
pub mod taunt;
pub mod unlock;
