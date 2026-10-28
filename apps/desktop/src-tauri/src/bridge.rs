//! WebSocket server on 127.0.0.1 that the browser extension connects to.
//!
//! Only browser extensions may connect: the `Origin` header has to be a
//! `chrome-extension://` or `moz-extension://` URL, so ordinary web pages
//! can't talk to GetLocked.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use getlocked_core::protocol::{FromExtension, SiteRulesMsg, ToExtension, PORTS, PROTOCOL_VERSION};
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;

/// A client that hasn't sent anything for this long is considered gone.
const STALE_AFTER: Duration = Duration::from_secs(30);
const PING_EVERY: Duration = Duration::from_secs(10);

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

#[derive(Default)]
struct Inner {
    clients: HashMap<u64, Client>,
    next_id: u64,
    rules: SiteRulesMsg,
    port: Option<u16>,
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
    pub fn start(events: Sender<BridgeEvent>) -> Self {
        let bridge = Bridge { inner: Arc::new(Mutex::new(Inner::default())) };
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
        let Ok(ws) = tokio_tungstenite::accept_hdr_async(stream, check).await else { return };
        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = unbounded_channel::<String>();

        let id = {
            let mut inner = self.inner.lock().unwrap();
            let id = inner.next_id;
            inner.next_id += 1;
            let _ = tx.send(encode(&ToExtension::Welcome { protocol: PROTOCOL_VERSION }));
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
                    break;
                }
            }
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
                FromExtension::Heartbeat => {}
                FromExtension::Blocked { domain } => {
                    let _ = events.send(BridgeEvent::Blocked { domain });
                }
            }
        }

        writer.abort();
        self.inner.lock().unwrap().clients.remove(&id);
        let _ = events.send(BridgeEvent::ClientsChanged);
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
