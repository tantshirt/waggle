//! Lazy ACP supervisor — spawn `buzz-acp` when an offline agent is @mentioned.
//!
//! Caps concurrency to avoid Welcome-team process storms. Relies on buzz-acp's own
//! idle timeout to exit; the supervisor reaps children and will respawn on the next
//! mention.
//!
//! Speaks Buzz's WebSocket protocol: NIP-42 AUTH before any REQ, then
//! **channel-scoped** subscriptions (`#h`). Global `#p` filters never receive
//! private channel kind:9 events.

use std::collections::HashMap;
use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use nostr::{EventBuilder, Keys, Kind, Tag};
use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::WebSocket;

use crate::runtime::RuntimeConfig;

pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Default idle timeout passed to buzz-acp (`BUZZ_ACP_IDLE_TIMEOUT`). Matches Buzz Desktop.
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 320;

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("runtime config directory missing: {0}")]
    NoRuntimeDir(PathBuf),

    #[error("no runtime configs in {0} — run waggle runtime emit for each agent first")]
    Empty(PathBuf),

    #[error("invalid relay URL {0}: {1}")]
    BadRelay(String, String),

    #[error("websocket error: {0}")]
    Ws(String),

    #[error("could not spawn buzz-acp: {0}")]
    Spawn(String),

    #[error(
        "supervisor needs an identity for NIP-42 AUTH — set BUZZ_PRIVATE_KEY / WAGGLE_OWNER_NSEC, or pass --auth-role"
    )]
    AuthIdentityMissing,

    #[error("NIP-42 authentication failed: {0}")]
    AuthFailed(String),

    #[error(
        "no channels configured for supervisor — pass --channel (repeatable) or set WAGGLE_SUPERVISOR_CHANNELS"
    )]
    NoChannels,
}

#[derive(Debug, Clone)]
pub struct SupervisorOptions {
    pub project_root: PathBuf,
    pub relay_url: String,
    pub buzz_acp: PathBuf,
    pub agent_command: String,
    pub agent_owner: Option<String>,
    pub max_concurrent: usize,
    pub respond_to: String,
    /// Seconds of inactivity before buzz-acp exits; supervisor reaps and may respawn.
    pub idle_timeout_secs: u64,
    /// Channel ids to subscribe (`#h`). Required — global `#p` never sees private traffic.
    pub channel_ids: Vec<String>,
    /// Optional role whose `.nsec` authenticates the supervisor WS (default: owner env).
    pub auth_role: Option<String>,
}

struct LiveAgent {
    child: Child,
    started: Instant,
    cfg: RuntimeConfig,
}

/// Run until `stop` is set.
pub fn run(opts: SupervisorOptions, stop: Arc<AtomicBool>) -> Result<(), SupervisorError> {
    if opts.channel_ids.is_empty() {
        return Err(SupervisorError::NoChannels);
    }

    let runtime_dir = opts.project_root.join("keys").join("runtime");
    if !runtime_dir.is_dir() {
        return Err(SupervisorError::NoRuntimeDir(runtime_dir));
    }

    let agents = load_runtime_configs(&runtime_dir)?;
    if agents.is_empty() {
        return Err(SupervisorError::Empty(runtime_dir));
    }

    let by_pubkey: HashMap<String, RuntimeConfig> = agents
        .into_iter()
        .map(|c| (c.public_key_hex.to_ascii_lowercase(), c))
        .collect();

    let auth_keys = load_supervisor_keys(&opts)?;

    eprintln!(
        "supervisor: {} agent(s), {} channel(s), max_concurrent={}, relay={}",
        by_pubkey.len(),
        opts.channel_ids.len(),
        opts.max_concurrent,
        opts.relay_url
    );

    let mut live: HashMap<String, LiveAgent> = HashMap::new();
    let mut backoff = Duration::from_secs(1);

    while !stop.load(Ordering::Relaxed) {
        match event_loop(&opts, &auth_keys, &by_pubkey, &mut live, &stop) {
            Ok(()) => break,
            Err(e) => {
                eprintln!("supervisor: connection lost ({e}); retry in {backoff:?}");
                reap_dead(&mut live);
                thread::sleep(backoff);
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        }
    }

    for (_, mut a) in live.drain() {
        let _ = a.child.kill();
        let _ = a.child.wait();
    }
    Ok(())
}

fn load_supervisor_keys(opts: &SupervisorOptions) -> Result<Keys, SupervisorError> {
    if let Some(role) = &opts.auth_role {
        let path = opts.project_root.join("keys").join(format!("{role}.nsec"));
        let secret = fs::read_to_string(&path).map_err(|e| {
            SupervisorError::AuthFailed(format!("cannot read {}: {e}", path.display()))
        })?;
        return Keys::parse(secret.trim())
            .map_err(|e| SupervisorError::AuthFailed(format!("bad key for role {role}: {e}")));
    }
    let secret = std::env::var("WAGGLE_OWNER_NSEC")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::env::var("BUZZ_PRIVATE_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .ok_or(SupervisorError::AuthIdentityMissing)?;
    Keys::parse(secret.trim()).map_err(|e| SupervisorError::AuthFailed(e.to_string()))
}

fn load_runtime_configs(dir: &Path) -> Result<Vec<RuntimeConfig>, SupervisorError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| SupervisorError::Spawn(e.to_string()))? {
        let entry = entry.map_err(|e| SupervisorError::Spawn(e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| SupervisorError::Spawn(e.to_string()))?;
        let cfg: RuntimeConfig = serde_json::from_str(&raw)
            .map_err(|e| SupervisorError::Spawn(format!("{}: {e}", path.display())))?;
        out.push(cfg);
    }
    Ok(out)
}

fn event_loop(
    opts: &SupervisorOptions,
    auth_keys: &Keys,
    by_pubkey: &HashMap<String, RuntimeConfig>,
    live: &mut HashMap<String, LiveAgent>,
    stop: &AtomicBool,
) -> Result<(), SupervisorError> {
    let ws_url = http_to_ws(&opts.relay_url);
    let (mut socket, _resp): (WebSocket<MaybeTlsStream<TcpStream>>, _) =
        tungstenite::connect(ws_url.as_str()).map_err(|e| SupervisorError::Ws(e.to_string()))?;

    authenticate(&mut socket, auth_keys, &opts.relay_url)?;

    let pubkeys: Vec<String> = by_pubkey.keys().cloned().collect();
    for (i, channel_id) in opts.channel_ids.iter().enumerate() {
        let filter = serde_json::json!({
            "kinds": [9],
            "#h": [channel_id],
            "#p": pubkeys,
            "limit": 0
        });
        let sub_id = format!("waggle-supervisor-{i}");
        let req = serde_json::json!(["REQ", sub_id, filter]);
        socket
            .send(Message::Text(req.to_string().into()))
            .map_err(|e| SupervisorError::Ws(e.to_string()))?;
    }

    eprintln!(
        "supervisor: authenticated; subscribed to {} channel(s)",
        opts.channel_ids.len()
    );

    while !stop.load(Ordering::Relaxed) {
        set_read_timeout(&socket, Some(Duration::from_secs(2)))
            .map_err(|e| SupervisorError::Ws(e.to_string()))?;

        match socket.read() {
            Ok(Message::Text(text)) => {
                handle_frame(&text, opts, by_pubkey, live)?;
            }
            Ok(Message::Ping(p)) => {
                let _ = socket.send(Message::Pong(p));
            }
            Ok(Message::Close(_)) => {
                return Err(SupervisorError::Ws("server closed".into()));
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(SupervisorError::Ws(e.to_string())),
        }
        reap_dead(live);
    }
    Ok(())
}

/// NIP-42: wait for AUTH challenge, sign kind:22242, wait for OK.
fn authenticate(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    keys: &Keys,
    relay_url: &str,
) -> Result<(), SupervisorError> {
    set_read_timeout(socket, Some(Duration::from_secs(15)))
        .map_err(|e| SupervisorError::Ws(e.to_string()))?;

    let challenge = wait_for_auth_challenge(socket)?;
    let auth_event = build_auth_event(keys, &challenge, relay_url)?;
    let auth_id = auth_event.id.to_hex();
    let msg = serde_json::json!(["AUTH", auth_event]);
    socket
        .send(Message::Text(msg.to_string().into()))
        .map_err(|e| SupervisorError::Ws(e.to_string()))?;
    wait_for_ok(socket, &auth_id)
}

fn wait_for_auth_challenge(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
) -> Result<String, SupervisorError> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match socket.read() {
            Ok(Message::Text(text)) => {
                let v: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| SupervisorError::AuthFailed(e.to_string()))?;
                let arr = v
                    .as_array()
                    .ok_or_else(|| SupervisorError::AuthFailed("non-array frame".into()))?;
                if arr.first().and_then(|x| x.as_str()) == Some("AUTH") {
                    let challenge = arr.get(1).and_then(|x| x.as_str()).ok_or_else(|| {
                        SupervisorError::AuthFailed("AUTH missing challenge".into())
                    })?;
                    return Ok(challenge.to_string());
                }
                // Ignore NOTICE / other frames until AUTH arrives.
            }
            Ok(Message::Ping(p)) => {
                let _ = socket.send(Message::Pong(p));
            }
            Ok(Message::Close(_)) => {
                return Err(SupervisorError::AuthFailed(
                    "server closed before AUTH".into(),
                ));
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => return Err(SupervisorError::Ws(e.to_string())),
        }
    }
    Err(SupervisorError::AuthFailed(
        "timed out waiting for AUTH challenge".into(),
    ))
}

fn build_auth_event(
    keys: &Keys,
    challenge: &str,
    relay_url: &str,
) -> Result<nostr::Event, SupervisorError> {
    let relay = http_to_ws(relay_url);
    let event = EventBuilder::new(Kind::Authentication, "")
        .tags(vec![
            Tag::parse(["relay", &relay])
                .map_err(|e| SupervisorError::AuthFailed(e.to_string()))?,
            Tag::parse(["challenge", challenge])
                .map_err(|e| SupervisorError::AuthFailed(e.to_string()))?,
        ])
        .sign_with_keys(keys)
        .map_err(|e| SupervisorError::AuthFailed(e.to_string()))?;
    Ok(event)
}

fn wait_for_ok(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    event_id: &str,
) -> Result<(), SupervisorError> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match socket.read() {
            Ok(Message::Text(text)) => {
                let v: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| SupervisorError::AuthFailed(e.to_string()))?;
                let arr = match v.as_array() {
                    Some(a) => a,
                    None => continue,
                };
                match arr.first().and_then(|x| x.as_str()) {
                    Some("OK") => {
                        let id = arr.get(1).and_then(|x| x.as_str()).unwrap_or("");
                        if id != event_id {
                            continue;
                        }
                        let accepted = arr.get(2).and_then(|x| x.as_bool()).unwrap_or(false);
                        if accepted {
                            return Ok(());
                        }
                        let reason = arr
                            .get(3)
                            .and_then(|x| x.as_str())
                            .unwrap_or("rejected")
                            .to_string();
                        return Err(SupervisorError::AuthFailed(reason));
                    }
                    Some("NOTICE") | Some("AUTH") => continue,
                    _ => continue,
                }
            }
            Ok(Message::Ping(p)) => {
                let _ = socket.send(Message::Pong(p));
            }
            Ok(Message::Close(_)) => {
                return Err(SupervisorError::AuthFailed(
                    "server closed during AUTH OK".into(),
                ));
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => return Err(SupervisorError::Ws(e.to_string())),
        }
    }
    Err(SupervisorError::AuthFailed(
        "timed out waiting for AUTH OK".into(),
    ))
}

fn set_read_timeout(
    socket: &WebSocket<MaybeTlsStream<TcpStream>>,
    timeout: Option<Duration>,
) -> std::io::Result<()> {
    match socket.get_ref() {
        MaybeTlsStream::Plain(t) => t.set_read_timeout(timeout),
        MaybeTlsStream::Rustls(t) => t.get_ref().set_read_timeout(timeout),
        _ => Ok(()),
    }
}

fn handle_frame(
    text: &str,
    opts: &SupervisorOptions,
    by_pubkey: &HashMap<String, RuntimeConfig>,
    live: &mut HashMap<String, LiveAgent>,
) -> Result<(), SupervisorError> {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    let arr = match v.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return Ok(()),
    };
    match arr[0].as_str() {
        Some("CLOSED") => {
            let reason = arr
                .get(2)
                .and_then(|x| x.as_str())
                .unwrap_or("closed")
                .to_string();
            if reason.contains("auth-required") {
                return Err(SupervisorError::AuthFailed(reason));
            }
            eprintln!("supervisor: subscription closed: {reason}");
            return Ok(());
        }
        Some("NOTICE") => {
            if let Some(msg) = arr.get(1).and_then(|x| x.as_str()) {
                eprintln!("supervisor: notice: {msg}");
            }
            return Ok(());
        }
        Some("EVENT") if arr.len() >= 3 => {}
        _ => return Ok(()),
    }

    let event = &arr[2];
    let tags = event
        .get("tags")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    for tag in tags {
        let Some(t) = tag.as_array() else { continue };
        if t.first().and_then(|x| x.as_str()) != Some("p") {
            continue;
        }
        let Some(pk) = t.get(1).and_then(|x| x.as_str()) else {
            continue;
        };
        let pk = pk.to_ascii_lowercase();
        if live.contains_key(&pk) {
            continue;
        }
        let Some(cfg) = by_pubkey.get(&pk) else {
            continue;
        };
        if live.len() >= opts.max_concurrent {
            eprintln!(
                "supervisor: at concurrency cap ({}) — not starting {}",
                opts.max_concurrent, cfg.role
            );
            continue;
        }
        eprintln!(
            "supervisor: ensure {} ({})",
            cfg.role,
            &pk[..12.min(pk.len())]
        );
        let child = spawn_acp(opts, cfg)?;
        live.insert(
            pk,
            LiveAgent {
                child,
                started: Instant::now(),
                cfg: cfg.clone(),
            },
        );
    }
    Ok(())
}

fn spawn_acp(opts: &SupervisorOptions, cfg: &RuntimeConfig) -> Result<Child, SupervisorError> {
    let secret = fs::read_to_string(&cfg.secret_key_path)
        .map_err(|e| SupervisorError::Spawn(format!("{}: {e}", cfg.secret_key_path)))?;
    let mut cmd = Command::new(&opts.buzz_acp);
    cmd.env("BUZZ_PRIVATE_KEY", secret.trim())
        .env("BUZZ_RELAY_URL", &cfg.relay_url)
        .env("BUZZ_ACP_AGENT_COMMAND", &opts.agent_command)
        .env("BUZZ_ACP_AGENT_ARGS", "")
        .env("BUZZ_ACP_SYSTEM_PROMPT_FILE", &cfg.persona_file)
        .env("BUZZ_ACP_RESPOND_TO", &opts.respond_to)
        .env("BUZZ_ACP_IDLE_TIMEOUT", opts.idle_timeout_secs.to_string())
        .env("RUST_LOG", "buzz_acp=info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(&opts.project_root);
    if let Some(owner) = &opts.agent_owner {
        cmd.env("BUZZ_ACP_AGENT_OWNER", owner);
    }
    cmd.arg("--relay-url").arg(&cfg.relay_url);
    cmd.spawn()
        .map_err(|e| SupervisorError::Spawn(format!("{}: {e}", opts.buzz_acp.display())))
}

fn reap_dead(live: &mut HashMap<String, LiveAgent>) {
    let mut gone = Vec::new();
    for (pk, agent) in live.iter_mut() {
        match agent.child.try_wait() {
            Ok(Some(status)) => {
                eprintln!(
                    "supervisor: {} exited ({status}) after {:?}",
                    agent.cfg.role,
                    agent.started.elapsed()
                );
                gone.push(pk.clone());
            }
            Ok(None) => {}
            Err(_) => gone.push(pk.clone()),
        }
    }
    for pk in gone {
        live.remove(&pk);
    }
}

fn http_to_ws(relay: &str) -> String {
    if let Some(rest) = relay.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = relay.strip_prefix("http://") {
        format!("ws://{rest}")
    } else if relay.starts_with("ws://") || relay.starts_with("wss://") {
        relay.to_string()
    } else {
        format!("ws://{relay}")
    }
}

/// Build a channel-scoped filter used by the supervisor (exported for tests).
pub fn channel_mention_filter(channel_id: &str, pubkeys: &[String]) -> serde_json::Value {
    serde_json::json!({
        "kinds": [9],
        "#h": [channel_id],
        "#p": pubkeys,
        "limit": 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_filter_includes_h_and_not_global_only_p() {
        let f = channel_mention_filter("chan1", &["abc".into()]);
        assert_eq!(f["#h"], serde_json::json!(["chan1"]));
        assert_eq!(f["#p"], serde_json::json!(["abc"]));
        assert_eq!(f["kinds"], serde_json::json!([9]));
    }

    #[test]
    fn http_to_ws_rewrites_schemes() {
        assert_eq!(http_to_ws("http://localhost:3100"), "ws://localhost:3100");
        assert_eq!(http_to_ws("https://r.example"), "wss://r.example");
        assert_eq!(http_to_ws("ws://localhost:3100"), "ws://localhost:3100");
    }
}
