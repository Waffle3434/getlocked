//! Messages between the desktop app and the browser extension.
//!
//! Mirrored in TypeScript by `packages/protocol/src/index.ts`. Keep the two
//! in sync and bump `PROTOCOL_VERSION` on breaking changes.

use serde::{Deserialize, Serialize};

use crate::model::ListMode;

pub const PROTOCOL_VERSION: u32 = 1;
/// The app listens on the first free port in this range on 127.0.0.1.
pub const PORTS: std::ops::RangeInclusive<u16> = 47613..=47617;

/// App → extension.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ToExtension {
    Welcome { protocol: u32 },
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
}

/// Extension → app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum FromExtension {
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
        });
        assert_eq!(
            serde_json::to_string(&m).unwrap(),
            r#"{"type":"rules","active":true,"mode":"block","domains":["youtube.com"],"endsAt":5,"label":null}"#
        );
        let hello: FromExtension = serde_json::from_str(
            r#"{"type":"hello","protocol":1,"browser":"edge","version":"0.1.0","incognitoAllowed":false}"#,
        )
        .unwrap();
        assert!(matches!(hello, FromExtension::Hello { incognito_allowed: false, .. }));
        let blocked: FromExtension = serde_json::from_str(r#"{"type":"blocked","domain":"x.com"}"#).unwrap();
        assert_eq!(blocked, FromExtension::Blocked { domain: "x.com".into() });
    }
}
