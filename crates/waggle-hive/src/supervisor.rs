//! Lazy ACP supervisor — spawn `buzz-acp` when an offline agent is @mentioned.
//!
//! Caps concurrency to avoid Welcome-team process storms. Relies on buzz-acp's own
//! idle timeout to exit; the supervisor reaps children and will respawn on the next
//! mention.

use std::collections::HashMap;
use std::fs;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tungstenite::protocol::Message;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::WebSocket;

use crate::runtime::RuntimeConfig;

pub const DEFAULT_MAX_CONCURRENT: usize = 4;

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
}

/// Default idle timeout passed to buzz-acp (`BUZZ_ACP_IDLE_TIMEOUT`). Matches Buzz Desktop.
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 320;

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
}

struct LiveAgent {
    child: Child,
    started: Instant,
    cfg: RuntimeConfig,
}

/// Run until `stop` is set.
pub fn run(opts: SupervisorOptions, stop: Arc<AtomicBool>) -> Result<(), SupervisorError> {
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

    eprintln!(
        "supervisor: {} agent(s), max_concurrent={}, relay={}",
        by_pubkey.len(),
        opts.max_concurrent,
        opts.relay_url
    );

    let mut live: HashMap<String, LiveAgent> = HashMap::new();
    let mut backoff = Duration::from_secs(1);

    while !stop.load(Ordering::Relaxed) {
        match event_loop(&opts, &by_pubkey, &mut live, &stop) {
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
    by_pubkey: &HashMap<String, RuntimeConfig>,
    live: &mut HashMap<String, LiveAgent>,
    stop: &AtomicBool,
) -> Result<(), SupervisorError> {
    let ws_url = http_to_ws(&opts.relay_url);
    let (mut socket, _resp): (WebSocket<MaybeTlsStream<TcpStream>>, _) =
        tungstenite::connect(ws_url.as_str()).map_err(|e| SupervisorError::Ws(e.to_string()))?;

    let pubkeys: Vec<String> = by_pubkey.keys().cloned().collect();
    let filter = serde_json::json!({
        "kinds": [9],
        "#p": pubkeys,
        "limit": 0
    });
    let req = serde_json::json!(["REQ", "waggle-supervisor", filter]);
    socket
        .send(Message::Text(req.to_string().into()))
        .map_err(|e| SupervisorError::Ws(e.to_string()))?;

    eprintln!("supervisor: subscribed for mentions");

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

fn set_read_timeout(
    socket: &WebSocket<MaybeTlsStream<TcpStream>>,
    timeout: Option<Duration>,
) -> std::io::Result<()> {
    match socket.get_ref() {
        MaybeTlsStream::Plain(t) => t.set_read_timeout(timeout),
        MaybeTlsStream::Rustls(t) => {
            // StreamOwned<ClientConnection, TcpStream>
            t.get_ref().set_read_timeout(timeout)
        }
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
    if arr[0].as_str() != Some("EVENT") || arr.len() < 3 {
        return Ok(());
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
        eprintln!("supervisor: ensure {} ({})", cfg.role, &pk[..12.min(pk.len())]);
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
        .env(
            "BUZZ_ACP_IDLE_TIMEOUT",
            opts.idle_timeout_secs.to_string(),
        )
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
