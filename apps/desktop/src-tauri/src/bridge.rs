//! WebSocket server on 127.0.0.1 that the browser extension connects to.
//!
//! Only browser extensions may connect: the `Origin` header has to be a
//! `chrome-extension://` or `moz-extension://` URL, so ordinary web pages
//! can't talk to GetLocked. That header is easy to fake for anything else
//! on the PC, so a client also has to prove it knows this install's pairing
//! code (see `getlocked_core::pairing`) before it gets rules or counts as a
//! connected extension.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use getlocked_core::pairing::{self, PairingFile, Side};
use getlocked_core::persist::save_json;
use getlocked_core::protocol::{FromExtension, SiteRulesMsg, ToExtension, PORTS, PROTOCOL_VERSION};
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

/// A client that hasn't sent anything for this long is considered gone.
const STALE_AFTER: Duration = Duration::from_secs(30);
const PING_EVERY: Duration = Duration::from_secs(10);
/// A client that hasn't answered the pairing challenge by now is dropped.
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
/// No real message comes close to this.
const MAX_MESSAGE: usize = 64 * 1024;
/// The longest a DNS name can be.
const MAX_DOMAIN_LEN: usize = 253;

pub enum BridgeEvent {
    Blocked { domain: String },
    ClientsChanged,
}

struct Client {
    browser: Option<String>,
    version: Option<String>,
    incognito_allowed: bool,
    last_seen: Instant,
    tx: UnboundedSender<String>,
}

struct Inner {
    /// Only clients that passed the pairing handshake.
    clients: HashMap<u64, Client>,
    next_id: u64,
    rules: SiteRulesMsg,
    port: Option<u16>,
    code: String,
    pairing_file: PathBuf,
}

/// What the Settings page shows about connected extensions.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub browser: String,
    pub version: String,
    pub incognito_allowed: bool,
}

#[derive(Clone)]
pub struct Bridge {
    inner: Arc<Mutex<Inner>>,
}

fn encode(msg: &ToExtension) -> String {
    serde_json::to_string(msg).expect("protocol messages always serialize")
}

fn origin_allowed(req: &Request) -> bool {
    req.headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|o| o.starts_with("chrome-extension://") || o.starts_with("moz-extension://"))
}

impl Bridge {
    /// Starts listening. The pairing code is read from `pairing_file`, or
    /// created there on first run.
    pub fn start(events: Sender<BridgeEvent>, pairing_file: PathBuf) -> Self {
        let code = pairing::load_or_create(&pairing_file).unwrap_or_else(|e| {
            eprintln!("bridge: couldn't save the pairing code ({e}); using a temporary one");
            pairing::generate_code()
        });
        let inner = Inner {
            clients: HashMap::new(),
            next_id: 0,
            rules: SiteRulesMsg::default(),
            port: None,
            code,
            pairing_file,
        };
        let bridge = Bridge { inner: Arc::new(Mutex::new(inner)) };
        let b = bridge.clone();
        tauri::async_runtime::spawn(async move {
            let mut listener = None;
            for port in PORTS {
                if let Ok(l) = TcpListener::bind(("127.0.0.1", port)).await {
                    b.inner.lock().unwrap().port = Some(port);
                    listener = Some(l);
                    break;
                }
            }
            let Some(listener) = listener else {
                eprintln!("bridge: no free port in {PORTS:?}; the browser extension can't connect");
                return;
            };
            let pinger = b.clone();
            let ping_events = events.clone();
            tauri::async_runtime::spawn(async move { pinger.ping_loop(ping_events).await });
            while let Ok((stream, _)) = listener.accept().await {
                let b = b.clone();
                let events = events.clone();
                tauri::async_runtime::spawn(async move { b.handle(stream, events).await });
            }
        });
        bridge
    }

    async fn ping_loop(self, events: Sender<BridgeEvent>) {
        loop {
            tokio::time::sleep(PING_EVERY).await;
            let mut inner = self.inner.lock().unwrap();
            let before = inner.clients.len();
            inner.clients.retain(|_, c| c.last_seen.elapsed() < STALE_AFTER);
            let ping = encode(&ToExtension::Ping);
            for c in inner.clients.values() {
                let _ = c.tx.send(ping.clone());
            }
            if inner.clients.len() != before {
                let _ = events.send(BridgeEvent::ClientsChanged);
            }
        }
    }

    // The handshake callback's error type is fixed by tungstenite.
    #[allow(clippy::result_large_err)]
    async fn handle(self, stream: TcpStream, events: Sender<BridgeEvent>) {
        let check = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
            if origin_allowed(req) {
                Ok(resp)
            } else {
                let mut err = ErrorResponse::new(Some("only the GetLocked extension may connect".into()));
                *err.status_mut() = StatusCode::FORBIDDEN;
                Err(err)
            }
        };
        let config = WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE)).max_frame_size(Some(MAX_MESSAGE));
        let Ok(mut ws) = tokio_tungstenite::accept_hdr_async_with_config(stream, check, Some(config)).await else {
            return;
        };
        if !self.authenticate(&mut ws).await {
            let _ = ws.close(None).await;
            return;
        }
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = unbounded_channel::<String>();

        let id = {
            let mut inner = self.inner.lock().unwrap();
            let id = inner.next_id;
            inner.next_id += 1;
            let _ = tx.send(encode(&ToExtension::Rules(inner.rules.clone())));
            inner.clients.insert(
                id,
                Client { browser: None, version: None, incognito_allowed: false, last_seen: Instant::now(), tx },
            );
            id
        };

        let writer = tauri::async_runtime::spawn(async move {
            while let Some(text) = rx.recv().await {
                if sink.send(Message::Text(text.into())).await.is_err() {
                    return;
                }
            }
            // The client was dropped (e.g. the pairing code changed): hang up.
            let _ = sink.send(Message::Close(None)).await;
        });

        while let Some(Ok(msg)) = stream.next().await {
            let Message::Text(text) = msg else { continue };
            let Ok(parsed) = serde_json::from_str::<FromExtension>(&text) else { continue };
            let mut inner = self.inner.lock().unwrap();
            let Some(client) = inner.clients.get_mut(&id) else { break };
            client.last_seen = Instant::now();
            match parsed {
                FromExtension::Hello { browser, version, incognito_allowed, .. } => {
                    client.browser = Some(browser.to_lowercase());
                    client.version = Some(version);
                    client.incognito_allowed = incognito_allowed;
                    let _ = events.send(BridgeEvent::ClientsChanged);
                }
                FromExtension::Auth { .. } | FromExtension::Heartbeat => {}
                FromExtension::Blocked { domain } => {
                    if domain.len() <= MAX_DOMAIN_LEN {
                        let _ = events.send(BridgeEvent::Blocked { domain });
                    }
                }
            }
        }

        writer.abort();
        self.inner.lock().unwrap().clients.remove(&id);
        let _ = events.send(BridgeEvent::ClientsChanged);
    }

    /// The pairing handshake. True if the client proved it knows the code;
    /// it has then also been sent our proof, bound to the port it dialled.
    async fn authenticate(&self, ws: &mut WebSocketStream<TcpStream>) -> bool {
        let Some(port) = self.port() else { return false };
        let app_nonce = pairing::nonce();
        let challenge = ToExtension::Challenge { protocol: PROTOCOL_VERSION, nonce: app_nonce.clone() };
        if ws.send(Message::Text(encode(&challenge).into())).await.is_err() {
            return false;
        }
        let reply = tokio::time::timeout(AUTH_TIMEOUT, async {
            while let Some(Ok(msg)) = ws.next().await {
                if let Message::Text(text) = msg {
                    return serde_json::from_str::<FromExtension>(&text).ok();
                }
            }
            None
        })
        .await;
        let Ok(Some(FromExtension::Auth { nonce: ext_nonce, proof })) = reply else { return false };
        let code = self.pairing_code();
        if !pairing::verify(&code, Side::Extension, &app_nonce, &ext_nonce, port, &proof) {
            let _ = ws.send(Message::Text(encode(&ToExtension::PairingFailed).into())).await;
            return false;
        }
        let welcome = ToExtension::Welcome {
            protocol: PROTOCOL_VERSION,
            proof: pairing::proof(&code, Side::App, &app_nonce, &ext_nonce, port),
        };
        ws.send(Message::Text(encode(&welcome).into())).await.is_ok()
    }

    pub fn pairing_code(&self) -> String {
        self.inner.lock().unwrap().code.clone()
    }

    /// Switch to a new pairing code. Every connected extension is dropped
    /// and has to be paired again.
    pub fn reset_pairing(&self) -> Result<String, String> {
        let code = pairing::generate_code();
        let mut inner = self.inner.lock().unwrap();
        save_json(&inner.pairing_file, &PairingFile { code: code.clone() })
            .map_err(|e| format!("couldn't save the new pairing code: {e}"))?;
        inner.code = code.clone();
        inner.clients.clear();
        Ok(code)
    }

    /// Push new rules to every connected extension (only if they changed).
    pub fn set_rules(&self, rules: SiteRulesMsg) {
        let mut inner = self.inner.lock().unwrap();
        if inner.rules == rules {
            return;
        }
        let msg = encode(&ToExtension::Rules(rules.clone()));
        inner.rules = rules;
        for c in inner.clients.values() {
            let _ = c.tx.send(msg.clone());
        }
    }

    pub fn port(&self) -> Option<u16> {
        self.inner.lock().unwrap().port
    }

    pub fn clients(&self) -> Vec<ClientInfo> {
        self.inner
            .lock()
            .unwrap()
            .clients
            .values()
            .filter_map(|c| {
                Some(ClientInfo {
                    browser: c.browser.clone()?,
                    version: c.version.clone().unwrap_or_default(),
                    incognito_allowed: c.incognito_allowed,
                })
            })
            .collect()
    }

    /// Browser families with a live extension. With `need_incognito`, only
    /// those allowed to run in private windows count.
    pub fn connected_families(&self, need_incognito: bool) -> HashSet<String> {
        self.clients().into_iter().filter(|c| !need_incognito || c.incognito_allowed).map(|c| c.browser).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tokio_tungstenite::MaybeTlsStream;

    type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

    async fn connect(port: u16, origin: Option<&str>) -> Option<Ws> {
        let mut req = format!("ws://127.0.0.1:{port}").into_client_request().unwrap();
        if let Some(o) = origin {
            req.headers_mut().insert("origin", o.parse().unwrap());
        }
        tokio_tungstenite::connect_async(req).await.ok().map(|(ws, _)| ws)
    }

    /// The next app message, or `None` once the app hangs up.
    async fn recv(ws: &mut Ws) -> Option<ToExtension> {
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Text(text) = msg {
                return Some(serde_json::from_str(&text).unwrap());
            }
        }
        None
    }

    async fn send(ws: &mut Ws, msg: &FromExtension) {
        ws.send(Message::Text(serde_json::to_string(msg).unwrap().into())).await.unwrap();
    }

    /// Answers the challenge with `code`, signing for `port`.
    async fn auth(ws: &mut Ws, code: &str, port: u16) -> (String, Option<ToExtension>) {
        let Some(ToExtension::Challenge { nonce: app_nonce, .. }) = recv(ws).await else { panic!("no challenge") };
        let ext_nonce = pairing::nonce();
        let proof = pairing::proof(code, Side::Extension, &app_nonce, &ext_nonce, port);
        send(ws, &FromExtension::Auth { nonce: ext_nonce.clone(), proof }).await;
        let reply = recv(ws).await;
        if let Some(ToExtension::Welcome { proof, .. }) = &reply {
            assert!(pairing::verify(code, Side::App, &app_nonce, &ext_nonce, port, proof));
        }
        (ext_nonce, reply)
    }

    #[tokio::test]
    async fn only_paired_clients_get_rules_and_count() {
        let file = std::env::temp_dir().join(format!("getlocked-bridge-test-{}.json", uuid::Uuid::new_v4()));
        let (tx, _rx) = std::sync::mpsc::channel();
        let bridge = Bridge::start(tx, file.clone());
        let mut port = None;
        for _ in 0..100 {
            port = bridge.port();
            if port.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let port = port.expect("bridge didn't bind a port");
        let code = bridge.pairing_code();
        let origin = Some("chrome-extension://abcdefghijklmnop");

        // No extension origin: refused before the handshake.
        assert!(connect(port, None).await.is_none());

        // Wrong code: told so, then hung up on, and never sent rules.
        let mut ws = connect(port, origin).await.unwrap();
        let (_, reply) = auth(&mut ws, &"x".repeat(pairing::CODE_LEN), port).await;
        assert_eq!(reply, Some(ToExtension::PairingFailed));
        assert_eq!(recv(&mut ws).await, None);

        // Right code but signed for another port (a relayed handshake): refused.
        let mut ws = connect(port, origin).await.unwrap();
        let (_, reply) = auth(&mut ws, &code, port + 1).await;
        assert_eq!(reply, Some(ToExtension::PairingFailed));

        // Right code: welcome with a valid proof, then the rules.
        let mut ws = connect(port, origin).await.unwrap();
        let (_, reply) = auth(&mut ws, &code, port).await;
        assert!(matches!(reply, Some(ToExtension::Welcome { .. })));
        assert!(matches!(recv(&mut ws).await, Some(ToExtension::Rules(_))));
        send(
            &mut ws,
            &FromExtension::Hello {
                protocol: PROTOCOL_VERSION,
                browser: "Edge".into(),
                version: "0.1.0".into(),
                incognito_allowed: true,
            },
        )
        .await;
        for _ in 0..100 {
            if !bridge.clients().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(bridge.connected_families(true), HashSet::from(["edge".to_string()]));

        // A new code drops the paired client.
        let new_code = bridge.reset_pairing().unwrap();
        assert_ne!(new_code, code);
        assert_eq!(recv(&mut ws).await, None);
        assert!(bridge.clients().is_empty());
        assert_eq!(pairing::load_or_create(&file).unwrap(), new_code);

        let _ = std::fs::remove_file(file);
    }
}
