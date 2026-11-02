//! Messages between the desktop app and the browser extension.
//!
//! Mirrored in TypeScript by `packages/protocol/src/index.ts`. Keep the two
//! in sync and bump `PROTOCOL_VERSION` on breaking changes.

use serde::{Deserialize, Serialize};

use crate::model::ListMode;

pub const PROTOCOL_VERSION: u32 = 2;
/// The app listens on the first free port in this range on 127.0.0.1.
pub const PORTS: std::ops::RangeInclusive<u16> = 47613..=47617;

/// App → extension.
///
/// A connection starts with a pairing handshake (see `crate::pairing`): the
/// app sends `Challenge`, the extension answers with `Auth`, and the app
/// replies `Welcome` with its own proof, or `PairingFailed` and hangs up.
/// Nothing else is sent or accepted until then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ToExtension {
    Challenge { protocol: u32, nonce: String },
    Welcome { protocol: u32, proof: String },
    PairingFailed,
    Rules(SiteRulesMsg),
    Ping,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteRulesMsg {
    /// False when no session is blocking (idle or on a break).
    pub active: bool,
    pub mode: ListMode,
    pub domains: Vec<String>,
    /// Unix ms when the current blocking phase ends on its own, if known.
    /// The extension keeps enforcing cached rules until then if the app
    /// disappears.
    pub ends_at: Option<i64>,
    pub label: Option<String>,
    /// Tasks not yet done, for the blocked page.
    #[serde(default)]
    pub open_tasks: Vec<String>,
    /// A taunt with `{site}` where the blocked domain goes. `None` when the
    /// user turned roasting off.
    #[serde(default)]
    pub taunt: Option<String>,
}

/// Extension → app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FromExtension {
    Auth {
        nonce: String,
        proof: String,
    },
    Hello {
        protocol: u32,
        /// `chrome`, `edge`, `brave`, `opera`, `vivaldi`, `firefox` or
        /// `chromium` when the exact browser is unknown.
        browser: String,
        version: String,
        incognito_allowed: bool,
    },
    Heartbeat,
    Blocked {
        domain: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_is_stable() {
        let m = ToExtension::Rules(SiteRulesMsg {
            active: true,
            mode: ListMode::Block,
            domains: vec!["youtube.com".into()],
            ends_at: Some(5),
            label: None,
            open_tasks: vec![],
            taunt: None,
        });
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"type":"rules","active":true,"mode":"block","domains":["youtube.com"],"endsAt":5,"label":null,"openTasks":[],"taunt":null}"#
        );
        let old: SiteRulesMsg =
            serde_json::from_str(r#"{"active":true,"mode":"block","domains":[],"endsAt":null,"label":null}"#).unwrap();
        assert!(old.open_tasks.is_empty() && old.taunt.is_none());
        let hello: FromExtension = serde_json::from_str(
            r#"{"type":"hello","protocol":2,"browser":"edge","version":"0.1.0","incognitoAllowed":false}"#,
        )
        .unwrap();
        assert!(matches!(hello, FromExtension::Hello { incognito_allowed: false, .. }));
        let auth: FromExtension = serde_json::from_str(r#"{"type":"auth","nonce":"ab","proof":"cd"}"#).unwrap();
        assert_eq!(auth, FromExtension::Auth { nonce: "ab".into(), proof: "cd".into() });
        assert_eq!(serde_json::to_string(&ToExtension::PairingFailed).unwrap(), r#"{"type":"pairingFailed"}"#);
        assert_eq!(
            serde_json::to_string(&ToExtension::Challenge { protocol: 2, nonce: "ab".into() }).unwrap(),
            r#"{"type":"challenge","protocol":2,"nonce":"ab"}"#
        );
        let blocked: FromExtension = serde_json::from_str(r#"{"type":"blocked","domain":"x.com"}"#).unwrap();
        assert_eq!(blocked, FromExtension::Blocked { domain: "x.com".into() });
    }
}
