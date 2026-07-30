//! The waggle command line — the only crate that wires the others (AD-1's dependency rule).
//!
//! **AD-20:** every command is machine-first. Structured output on stdout, diagnostics on
//! stderr, no interactive input required, and a fixed exit-code taxonomy.

mod commands;
mod common;
mod exit;
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use serde::Serialize;
use waggle_core::{Compatibility, VersionRange};

use common::{resolve_human_pubkey, roster_pubkeys, uuid_like};
use output::{emit, Format};

#[derive(Parser)]
#[command(
    name = "waggle",
    about = "Agentic method modules, compiled to a Buzz hive.",
    long_about = "waggle compiles a BMAD Method installation into a running Buzz hive.\n\n\
                  Compatible with the BMAD Method. Not affiliated with BMad Code, LLC.\n\n\
                  Exit codes: 0=ok  1=user error  2=upstream contract error  3=system failure",
    version
)]
struct Cli {
    /// Output format. `json` is stable and meant for scripting (AD-20, NFR-9).
    #[arg(long, short, global = true, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Project root. Defaults to the current directory.
    #[arg(long, global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check that the substrate and method versions are ones waggle supports.
    ///
    /// Run this before anything that depends on their contracts. Refuses rather than
    /// degrading when either is out of range (AD-18).
    Preflight {
        /// Proceed even when a version is unsupported. Explicit, and warns loudly.
        #[arg(long)]
        allow_unsupported: bool,

        /// Path to the substrate checkout. Defaults to `<root>/vendor/buzz`.
        #[arg(long)]
        substrate: Option<PathBuf>,
    },

    /// Manage agent identities — one Nostr keypair per method role.
    #[command(subcommand)]
    Identity(IdentityCmd),

    /// Agent runtime configuration and managed-agent records (FR-13).
    #[command(subcommand)]
    Runtime(RuntimeCmd),

    /// Compile a method module into a Buzz persona pack.
    ///
    /// Reads the installation (read-only, AD-3), resolves the agent descriptor across all
    /// three override layers (AD-5), and emits a pack. Deterministic (AD-4).
    Compile {
        /// Module code, e.g. `tea`. Required unless `--all`.
        #[arg(long, required_unless_present = "all")]
        module: Option<String>,

        /// Compile every installed module that registers agents or ships channel templates.
        #[arg(long)]
        all: bool,

        /// Agent id to compile, e.g. `bmad-tea`. Omit to compile every agent the
        /// module registers.
        #[arg(long)]
        agent: Option<String>,

        /// Output directory. Defaults to `<root>/packs`.
        #[arg(long)]
        out: Option<PathBuf>,

        /// Treat missing materialized skills as warnings instead of hard errors.
        /// For local iteration only — CI and `waggle sync` stay strict.
        #[arg(long)]
        allow_missing_skills: bool,
    },

    /// List what the method installation registers: modules, agents, provenance.
    Modules,

    /// Publish a method artifact as a signed, tagged event (FR-15, FR-17, FR-24).
    ///
    /// Publishes directly to the relay rather than through the substrate CLI, which
    /// cannot attach typed tags (UP-07).
    Publish {
        /// Role whose identity signs the event.
        #[arg(long)]
        role: String,

        /// Channel UUID.
        #[arg(long)]
        channel: String,

        /// What this is: artifact, handoff, verdict, or gate-record.
        #[arg(long, default_value = "artifact")]
        marker: String,

        /// Method artifact type, e.g. `prd`, `story`, `test-design`.
        #[arg(long)]
        artifact_type: Option<String>,

        #[arg(long)]
        module: Option<String>,

        #[arg(long)]
        story: Option<String>,

        /// Risk priority: P0, P1, P2, or P3.
        #[arg(long)]
        priority: Option<String>,

        /// Event ids this references. A handoff must name the artifact it transfers.
        #[arg(long = "ref")]
        references: Vec<String>,

        #[arg(long)]
        from_role: Option<String>,

        #[arg(long)]
        to_role: Option<String>,

        /// Body text, or `-` to read stdin.
        #[arg(long)]
        body: String,

        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,
    },

    /// Reconcile a gate and publish the authoritative record (FR-19..FR-23, UP-18).
    ///
    /// Reads the reactions itself and derives the approver from each reaction's
    /// signature-bound pubkey, then publishes the record under waggle's own identity.
    /// The relay's workflow output is NOT the record: it is relay-signed and its
    /// `{{trigger.author}}` is spoofable via an unguarded `actor` tag.
    Gate {
        #[arg(long)]
        role: String,

        #[arg(long)]
        channel: String,

        /// The verdict event being gated.
        #[arg(long)]
        verdict_event: String,

        /// The verdict: PASS, CONCERNS, FAIL, or WAIVED.
        #[arg(long)]
        verdict: String,

        /// Report the outcome without publishing a record.
        #[arg(long)]
        dry_run: bool,

        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,
    },

    /// Query the signed trail by tag, returning verifiable events (FR-22, FR-24).
    Trail {
        #[arg(long)]
        role: String,

        #[arg(long)]
        channel: String,

        /// Filter by priority (P0-P3), or omit for the whole waggle trail.
        #[arg(long)]
        priority: Option<String>,

        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,
    },

    /// Publish developer output as a portable NIP-34 patch, linked to its story channel.
    ///
    /// The patch itself is standard git-over-Nostr, readable by any NIP-34 client. waggle
    /// adds the link FR-18 requires: an event in the story channel referencing the patch
    /// and the artifacts that motivated it.
    Patch {
        #[arg(long)]
        role: String,

        /// Story channel the patch belongs to.
        #[arg(long)]
        channel: String,

        /// Repository identifier from the NIP-34 announcement.
        #[arg(long)]
        repo_id: String,

        /// Repo owner pubkey (hex). Defaults to the signing role's own key.
        #[arg(long)]
        repo_owner: Option<String>,

        /// A `git format-patch` file.
        #[arg(long)]
        patch_file: PathBuf,

        /// Earliest unique commit: `git rev-list --max-parents=0 HEAD | tail -1`.
        #[arg(long)]
        euc: String,

        /// Artifact events that motivated this patch.
        #[arg(long = "ref")]
        references: Vec<String>,

        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,

        #[arg(long)]
        buzz_cli: Option<PathBuf>,
    },

    /// Provision a module's channels and canvases into a running hive.
    ///
    /// Delegates creation to the substrate's own template mechanism, adding the
    /// existence check the substrate does not perform (UP-10).
    Provision {
        /// Module code, e.g. `tea`. Required unless `--all`.
        #[arg(long, required_unless_present = "all")]
        module: Option<String>,

        /// Merge every pack's channel templates (phase map) and provision once.
        #[arg(long)]
        all: bool,

        /// Role whose identity performs the provisioning.
        #[arg(long, default_value = "tea")]
        role: String,

        /// Compiled pack directory. Defaults to `<root>/packs/<module>`.
        #[arg(long)]
        pack: Option<PathBuf>,

        /// Relay base URL.
        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,

        /// Path to the substrate CLI.
        #[arg(long)]
        buzz_cli: Option<PathBuf>,

        /// Human Desktop pubkey (hex) to add to every phase channel.
        /// Defaults to `WAGGLE_HUMAN_PUBKEY` or `BUZZ_ACP_AGENT_OWNER`.
        #[arg(long, env = "WAGGLE_HUMAN_PUBKEY")]
        human_pubkey: Option<String>,

        /// Rewrite description + canvas on channels that already exist (template UX updates).
        #[arg(long)]
        refresh: bool,
    },

    /// Bring the method installation in line with BUZZ_VERSION and regenerate packs.
    ///
    /// Runs the BMAD installer (non-interactive), compiles all modules, provisions
    /// identities/profiles/managed-agents, and merges the hive phase channel map.
    Sync {
        /// Override BMAD_METHOD_VERSION from BUZZ_VERSION.
        #[arg(long)]
        bmad_version: Option<String>,

        /// Comma-separated module codes for the installer. Default: discover / full set.
        #[arg(long)]
        modules: Option<String>,

        /// Skip the BMAD installer; only recompile and provision from the current `_bmad/`.
        #[arg(long)]
        skip_install: bool,

        /// Allow versions outside BMAD_SUPPORTED.
        #[arg(long)]
        allow_unsupported: bool,

        /// Relay base URL for identity/channel provisioning.
        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,

        /// Do not call the relay (compile + identity files only).
        #[arg(long)]
        offline: bool,

        /// Path to the substrate CLI.
        #[arg(long)]
        buzz_cli: Option<PathBuf>,

        /// Human Desktop pubkey (hex) to add to every phase channel.
        #[arg(long, env = "WAGGLE_HUMAN_PUBKEY")]
        human_pubkey: Option<String>,

        /// Do not symlink BMAD skills into ~/.claude/skills (or $CLAUDE_SKILLS_HOME).
        #[arg(long)]
        skip_global_skills: bool,

        /// Rewrite description + canvas on channels that already exist.
        #[arg(long)]
        refresh: bool,
    },
}

#[derive(Subcommand)]
enum IdentityCmd {
    /// Generate a keypair for a role.
    ///
    /// Secret key material is written owner-only to `<root>/keys/` (gitignored) and is
    /// never printed. Existing identities are left alone unless `--force` is given.
    Provision {
        /// Role name, lowercase with dashes — e.g. `tea`, `dev`, `sm`.
        #[arg(long)]
        role: String,

        /// Replace an existing identity. **Destructive**: the old key is gone, and
        /// nothing it signed can be attributed to the new one.
        #[arg(long)]
        force: bool,
    },

    /// List provisioned identities. Public data only.
    List,

    /// Publish an agent's profile to the hive under its own key (FR-14).
    ///
    /// Reads display name and description from a compiled pack, so the hive shows what
    /// the method descriptor says rather than something hand-maintained.
    PublishProfile {
        /// Role whose identity signs the profile.
        #[arg(long)]
        role: String,

        /// Compiled pack directory, e.g. `packs/tea`.
        #[arg(long)]
        pack: PathBuf,

        /// Relay base URL.
        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,

        /// Path to the substrate CLI. Defaults to the vendored release build.
        #[arg(long)]
        buzz_cli: Option<PathBuf>,
    },

    /// Register a provisioned identity on the relay membership list (FR-12).
    ///
    /// Requires `DATABASE_URL`, `RELAY_URL`, and `BUZZ_RELAY_PRIVATE_KEY` in the
    /// environment. Idempotent: already-a-member is success.
    Register {
        /// Role to register.
        #[arg(long)]
        role: String,

        /// Membership role: `member` or `admin`.
        #[arg(long, default_value = "member")]
        member_role: String,

        /// Path to `buzz-admin`. Defaults to the vendored release build.
        #[arg(long)]
        buzz_admin: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RuntimeCmd {
    /// Emit agent runtime configuration for a role (FR-13).
    ///
    /// Writes `keys/runtime/<role>.json`. Does not start a session — that needs
    /// an ACP runtime and LLM credentials (see the config's `required_env`).
    Emit {
        #[arg(long)]
        role: String,

        /// Compiled pack directory, e.g. `packs/tea`.
        #[arg(long)]
        pack: PathBuf,

        /// Persona id inside the pack, e.g. `bmad-tea`.
        #[arg(long)]
        persona: String,

        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,

        /// Session concurrency ceiling (NFR-8).
        #[arg(long, default_value_t = waggle_hive::runtime::DEFAULT_MAX_SESSIONS)]
        max_sessions: u32,
    },

    /// Publish a kind:30177 managed-agent record for the role (headless).
    PublishAgent {
        #[arg(long)]
        role: String,

        #[arg(long)]
        pack: PathBuf,

        /// Persona id inside the pack — required so multi-agent packs cannot
        /// silently pick the wrong display name from an unordered directory listing.
        #[arg(long)]
        persona: String,

        #[arg(long, default_value = "http://localhost:3100")]
        relay: String,

        #[arg(long, default_value_t = waggle_hive::runtime::DEFAULT_MAX_SESSIONS)]
        max_sessions: u32,
    },

    /// Lazy-spawn buzz-acp when an offline agent is @mentioned (hive mirror).
    Supervisor {
        #[arg(long, default_value = "ws://localhost:3100")]
        relay: String,

        /// Path to buzz-acp. Defaults to the vendored debug build.
        #[arg(long)]
        buzz_acp: Option<PathBuf>,

        #[arg(long, default_value = "claude-agent-acp")]
        agent_command: String,

        /// Human owner pubkey (hex) — Desktop identity. Required for OwnerOnly agents.
        #[arg(long, env = "BUZZ_ACP_AGENT_OWNER")]
        agent_owner: Option<String>,

        #[arg(long, default_value_t = waggle_hive::supervisor::DEFAULT_MAX_CONCURRENT)]
        max_concurrent: usize,

        #[arg(long, default_value = "anyone")]
        respond_to: String,

        /// Idle seconds before buzz-acp exits (`BUZZ_ACP_IDLE_TIMEOUT`).
        #[arg(long, default_value_t = waggle_hive::supervisor::DEFAULT_IDLE_TIMEOUT_SECS)]
        idle_timeout: u64,

        /// Channel id to subscribe (`#h`). Repeatable. Also reads `WAGGLE_SUPERVISOR_CHANNELS`
        /// (comma-separated) when none are passed.
        #[arg(long = "channel")]
        channels: Vec<String>,

        /// Role whose `.nsec` authenticates the supervisor WebSocket (NIP-42).
        /// Defaults to `BUZZ_PRIVATE_KEY` / `WAGGLE_OWNER_NSEC`.
        #[arg(long)]
        auth_role: Option<String>,
    },
}

#[derive(Serialize)]
struct PreflightReport {
    substrate: ComponentReport,
    method: ComponentReport,
    /// Populated only when the substrate checkout is present.
    #[serde(skip_serializing_if = "Option::is_none")]
    substrate_integrity: Option<IntegrityReport>,
    overridden: bool,
}

#[derive(Serialize)]
struct ComponentReport {
    name: &'static str,
    /// `None` when detection itself failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    found: Option<String>,
    expected: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<Compatibility>,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct IntegrityReport {
    /// AD-2: no *tracked* file modified. Buzz-gitignored files (`.env`) do not count.
    clean: bool,
    modified: Vec<String>,
}

fn main() -> ExitCode {
    // Parsed manually rather than via `Cli::parse()`: clap exits with 2 on a usage error,
    // which collides with our UPSTREAM code. AD-20's taxonomy has to win, so a bad flag
    // must be a USER error and stay distinguishable from "upstream moved under us".
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = match e.kind() {
                // --help / --version are successful requests, not failures.
                clap::error::ErrorKind::DisplayHelp
                | clap::error::ErrorKind::DisplayVersion
                | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => exit::OK,
                _ => exit::USER,
            };
            let _ = e.print();
            return ExitCode::from(code);
        }
    };

    let root = match cli.root.clone() {
        Some(r) => r,
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: cannot determine the current directory: {e}");
                return ExitCode::from(exit::SYSTEM);
            }
        },
    };

    match cli.command {
        Command::Preflight {
            allow_unsupported,
            ref substrate,
        } => {
            let substrate_path = substrate
                .clone()
                .unwrap_or_else(|| waggle_hive::default_path(&root));
            run_preflight(&root, &substrate_path, allow_unsupported, cli.format)
        }
        Command::Identity(ref cmd) => run_identity(&root, cmd, cli.format),
        Command::Runtime(ref cmd) => run_runtime(&root, cmd, cli.format),
        Command::Compile {
            ref module,
            all,
            ref agent,
            ref out,
            allow_missing_skills,
        } => {
            let out_dir = out.clone().unwrap_or_else(|| root.join("packs"));
            if all {
                run_compile_all(&root, &out_dir, cli.format, allow_missing_skills)
            } else {
                let Some(module) = module.as_deref() else {
                    eprintln!("error: --module is required unless --all");
                    return ExitCode::from(exit::USER);
                };
                run_compile(
                    &root,
                    module,
                    agent.as_deref(),
                    &out_dir,
                    cli.format,
                    allow_missing_skills,
                )
            }
        }
        Command::Modules => run_modules(&root, cli.format),
        Command::Gate {
            ref role,
            ref channel,
            ref verdict_event,
            ref verdict,
            dry_run,
            ref relay,
        } => commands::gate::run_gate(
            &root,
            role,
            channel,
            verdict_event,
            verdict,
            dry_run,
            relay,
            cli.format,
        ),
        Command::Patch {
            ref role,
            ref channel,
            ref repo_id,
            ref repo_owner,
            ref patch_file,
            ref euc,
            ref references,
            ref relay,
            ref buzz_cli,
        } => {
            let cli_path = buzz_cli
                .clone()
                .unwrap_or_else(|| root.join("vendor/buzz/target/release/buzz"));
            run_patch(
                &root, role, channel, repo_id, repo_owner, patch_file, euc, references, relay,
                &cli_path, cli.format,
            )
        }
        Command::Publish {
            ref role,
            ref channel,
            ref marker,
            ref artifact_type,
            ref module,
            ref story,
            ref priority,
            ref references,
            ref from_role,
            ref to_role,
            ref body,
            ref relay,
        } => run_publish(
            &root,
            role,
            channel,
            marker,
            artifact_type,
            module,
            story,
            priority,
            references,
            from_role,
            to_role,
            body,
            relay,
            cli.format,
        ),
        Command::Trail {
            ref role,
            ref channel,
            ref priority,
            ref relay,
        } => run_trail(&root, role, channel, priority.as_deref(), relay, cli.format),
        Command::Provision {
            ref module,
            all,
            ref role,
            ref pack,
            ref relay,
            ref buzz_cli,
            ref human_pubkey,
            refresh,
        } => {
            let cli_path = buzz_cli.clone().unwrap_or_else(|| {
                let debug = root.join("vendor/buzz/target/debug/buzz");
                let release = root.join("vendor/buzz/target/release/buzz");
                if debug.exists() {
                    debug
                } else {
                    release
                }
            });
            let human = resolve_human_pubkey(human_pubkey.as_deref());
            if all {
                run_provision_all(
                    &root,
                    role,
                    relay,
                    &cli_path,
                    human.as_deref(),
                    refresh,
                    cli.format,
                )
            } else {
                let Some(module) = module.as_deref() else {
                    eprintln!("error: --module is required unless --all");
                    return ExitCode::from(exit::USER);
                };
                let pack_dir = pack
                    .clone()
                    .unwrap_or_else(|| root.join("packs").join(module));
                run_provision(
                    &root,
                    module,
                    role,
                    &pack_dir,
                    relay,
                    &cli_path,
                    human.as_deref(),
                    refresh,
                    cli.format,
                )
            }
        }
        Command::Sync {
            ref bmad_version,
            ref modules,
            skip_install,
            allow_unsupported,
            ref relay,
            offline,
            ref buzz_cli,
            ref human_pubkey,
            skip_global_skills,
            refresh,
        } => {
            let cli_path = buzz_cli.clone().unwrap_or_else(|| {
                let debug = root.join("vendor/buzz/target/debug/buzz");
                let release = root.join("vendor/buzz/target/release/buzz");
                if debug.exists() {
                    debug
                } else {
                    release
                }
            });
            commands::sync::run_sync(
                &root,
                bmad_version.as_deref(),
                modules.as_deref(),
                skip_install,
                allow_unsupported,
                relay,
                offline,
                &cli_path,
                resolve_human_pubkey(human_pubkey.as_deref()).as_deref(),
                skip_global_skills,
                refresh,
                cli.format,
            )
        }
    }
}

#[derive(Serialize)]
struct ProvisionedChannel {
    template: String,
    channel: String,
    outcome: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    id: String,
    canvas_applied: bool,
}

#[derive(Serialize)]
struct ProvisionReport {
    module: String,
    channels: Vec<ProvisionedChannel>,
}

/// Modules that register agents or ship `templates/<module>/channels.json`.
fn modules_to_compile(root: &std::path::Path) -> Vec<String> {
    let mut mods = Vec::new();
    if let Ok(registry) = waggle_method::registry::read(root) {
        mods.extend(waggle_method::registry::modules(&registry));
    }
    let templates = root.join("templates");
    if let Ok(rd) = std::fs::read_dir(&templates) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().join("channels.json").is_file() && !mods.contains(&name) {
                mods.push(name);
            }
        }
    }
    mods.sort();
    mods.dedup();
    mods
}

pub(crate) fn run_compile_all(
    root: &std::path::Path,
    out_dir: &std::path::Path,
    format: Format,
    allow_missing_skills: bool,
) -> ExitCode {
    let mods = modules_to_compile(root);
    if mods.is_empty() {
        eprintln!("error: no modules to compile — is BMAD installed?");
        return ExitCode::from(exit::USER);
    }
    let mut failed = false;
    for m in &mods {
        match format {
            Format::Text => println!("--- compile {m} ---"),
            Format::Json => {}
        }
        let code = run_compile(root, m, None, out_dir, format, allow_missing_skills);
        if code != ExitCode::from(exit::OK) {
            failed = true;
        }
    }
    // Merge hive phase store for provision --all.
    if let Err(e) = write_hive_channel_store(root, out_dir) {
        eprintln!("error: hive channel merge: {e}");
        return ExitCode::from(exit::SYSTEM);
    }
    if failed {
        ExitCode::from(exit::UPSTREAM)
    } else {
        match format {
            Format::Text => println!("compiled {} modules -> {}", mods.len(), out_dir.display()),
            Format::Json => emit(
                format,
                "compile.all",
                true,
                &serde_json::json!({ "modules": mods }),
            ),
        }
        ExitCode::from(exit::OK)
    }
}

fn write_hive_channel_store(
    root: &std::path::Path,
    out_dir: &std::path::Path,
) -> Result<(), String> {
    let mut stores = Vec::new();
    let packs = out_dir;
    if let Ok(rd) = std::fs::read_dir(packs) {
        for entry in rd.flatten() {
            // Skip the merged hive output itself — re-including it overwrites
            // freshly enriched canvases (e.g. #help from bmad-help.csv) with a
            // stale shorter copy from the previous merge.
            if entry.file_name() == "hive" {
                continue;
            }
            let path = entry.path().join("channel-templates.json");
            if !path.is_file() {
                continue;
            }
            let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let store: Vec<waggle_emit::channels::ChannelTemplateRecord> =
                serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?;
            stores.push(store);
        }
    }
    let merged = waggle_emit::channels::merge_stores(&stores);
    let hive = out_dir.join("hive");
    std::fs::create_dir_all(&hive).map_err(|e| e.to_string())?;
    let json = waggle_emit::channels::render(&merged).map_err(|e| e.to_string())?;
    std::fs::write(hive.join("channel-templates.json"), json).map_err(|e| e.to_string())?;
    let _ = root; // reserved for future sync-state path
    Ok(())
}

fn run_provision_all(
    root: &std::path::Path,
    role: &str,
    relay: &str,
    buzz_cli: &std::path::Path,
    human_pubkey: Option<&str>,
    refresh: bool,
    format: Format,
) -> ExitCode {
    run_provision_all_opts(
        root,
        role,
        relay,
        buzz_cli,
        human_pubkey,
        refresh,
        format,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_provision_all_opts(
    root: &std::path::Path,
    role: &str,
    relay: &str,
    buzz_cli: &std::path::Path,
    human_pubkey: Option<&str>,
    refresh: bool,
    format: Format,
    emit_report: bool,
) -> ExitCode {
    let out_dir = root.join("packs");
    if let Err(e) = write_hive_channel_store(root, &out_dir) {
        eprintln!("error: {e}");
        return ExitCode::from(exit::SYSTEM);
    }
    run_provision_opts(
        root,
        "hive",
        role,
        &out_dir.join("hive"),
        relay,
        buzz_cli,
        human_pubkey,
        refresh,
        format,
        emit_report,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_provision(
    root: &std::path::Path,
    module: &str,
    role: &str,
    pack_dir: &std::path::Path,
    relay: &str,
    buzz_cli: &std::path::Path,
    human_pubkey: Option<&str>,
    refresh: bool,
    format: Format,
) -> ExitCode {
    run_provision_opts(
        root,
        module,
        role,
        pack_dir,
        relay,
        buzz_cli,
        human_pubkey,
        refresh,
        format,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_provision_opts(
    root: &std::path::Path,
    module: &str,
    role: &str,
    pack_dir: &std::path::Path,
    relay: &str,
    buzz_cli: &std::path::Path,
    human_pubkey: Option<&str>,
    refresh: bool,
    format: Format,
    emit_report: bool,
) -> ExitCode {
    let templates_file = pack_dir.join("channel-templates.json");
    if !templates_file.exists() {
        // AD-6: a module with no templates provisions nothing and says so, rather than
        // failing or silently doing nothing.
        println!(
            "module {module:?} ships no channel templates ({} absent) — nothing to provision",
            templates_file.display()
        );
        return ExitCode::from(exit::OK);
    }

    let store: Vec<serde_json::Value> = match std::fs::read_to_string(&templates_file)
        .map_err(|e| e.to_string())
        .and_then(|raw| serde_json::from_str(&raw).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {}: {e}", templates_file.display());
            return ExitCode::from(exit::USER);
        }
    };

    // AD-14: the secret is read inside the adapter boundary and never returned here.
    let secret = match std::fs::read_to_string(root.join("keys").join(format!("{role}.nsec"))) {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            eprintln!(
                "error: no identity for role {role:?} — provision it first: \
                 waggle identity provision --role {role}"
            );
            return ExitCode::from(exit::USER);
        }
    };

    let existing = match waggle_hive::channels::existing_channel_names(buzz_cli, relay, &secret) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };

    let members = roster_pubkeys(root, human_pubkey);
    if human_pubkey.is_none() {
        eprintln!(
            "warning: no --human-pubkey / WAGGLE_HUMAN_PUBKEY / BUZZ_ACP_AGENT_OWNER — \
             Desktop may not see phase channels until you are added as a member"
        );
    }

    let mut out = Vec::new();
    for t in &store {
        let Some(template_name) = t.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        // The channel takes the template's name; the template is already module-prefixed.
        let (outcome, id, canvas_applied) = match waggle_hive::channels::provision_channel(
            buzz_cli,
            relay,
            &secret,
            &templates_file,
            template_name,
            template_name,
            &existing,
        ) {
            Ok(waggle_hive::channels::Provisioned::Created { id, canvas_applied }) => {
                ("created", id, canvas_applied)
            }
            Ok(waggle_hive::channels::Provisioned::AlreadyExists { id }) if refresh => {
                let desc = t.get("description").and_then(|v| v.as_str());
                let canvas = t.get("canvas_template").and_then(|v| v.as_str());
                match waggle_hive::channels::refresh_channel(
                    buzz_cli, relay, &secret, &id, desc, canvas,
                ) {
                    Ok(r) => ("refreshed", id, r.canvas_applied),
                    Err(e) => {
                        eprintln!("error: refresh {template_name}: {e}");
                        return ExitCode::from(exit::UPSTREAM);
                    }
                }
            }
            Ok(waggle_hive::channels::Provisioned::AlreadyExists { id }) => {
                ("already-exists", id, false)
            }
            Ok(waggle_hive::channels::Provisioned::Refreshed {
                id, canvas_applied, ..
            }) => ("refreshed", id, canvas_applied),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(exit::UPSTREAM);
            }
        };

        if !members.is_empty() {
            let failed =
                waggle_hive::channels::ensure_members(buzz_cli, relay, &secret, &id, &members);
            for (pk, err) in failed.into_iter().take(3) {
                eprintln!(
                    "warning: add-member {}… on {template_name}: {err}",
                    &pk[..12.min(pk.len())]
                );
            }
        }

        out.push(ProvisionedChannel {
            template: template_name.to_string(),
            channel: template_name.to_string(),
            outcome,
            id,
            canvas_applied,
        });
    }

    let report = ProvisionReport {
        module: module.to_string(),
        channels: out,
    };

    if emit_report {
        match format {
            Format::Json => emit(format, "provision", true, &report),
            Format::Text => {
                for c in &report.channels {
                    let canvas = if c.canvas_applied { " +canvas" } else { "" };
                    println!("{:<14} {}{}", c.outcome, c.channel, canvas);
                }
                if !members.is_empty() {
                    println!(
                        "roster        {} pubkey(s) ensured on each channel",
                        members.len()
                    );
                }
            }
        }
    }
    ExitCode::from(exit::OK)
}
#[derive(Serialize)]
struct PatchReport {
    patch_event: String,
    link_event: String,
    repo_id: String,
    /// Standard NIP-34 kinds, so third-party clients can read this.
    kinds: Vec<u32>,
}

#[allow(clippy::too_many_arguments)]
fn run_patch(
    root: &std::path::Path,
    role: &str,
    channel: &str,
    repo_id: &str,
    repo_owner: &Option<String>,
    patch_file: &std::path::Path,
    euc: &str,
    references: &[String],
    relay: &str,
    buzz_cli: &std::path::Path,
    format: Format,
) -> ExitCode {
    // AD-14: read the secret here, hand it to the adapter, never hold it in the report.
    let secret = match std::fs::read_to_string(root.join("keys").join(format!("{role}.nsec"))) {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            eprintln!(
                "error: no identity for role {role:?} — provision it first: \
                 waggle identity provision --role {role}"
            );
            return ExitCode::from(exit::USER);
        }
    };

    let owner = match repo_owner.clone() {
        Some(o) => o,
        None => match std::fs::read_to_string(root.join("keys").join(format!("{role}.pub"))) {
            Ok(s) => s.trim().to_string(),
            Err(e) => {
                eprintln!("error: could not read the role's public key: {e}");
                return ExitCode::from(exit::USER);
            }
        },
    };

    let patch_event = match waggle_hive::patches::send_patch(
        buzz_cli, relay, &secret, &owner, repo_id, patch_file, euc, true,
    ) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(match e {
                waggle_hive::patches::PatchError::NoPatchFile(_) => exit::USER,
                _ => exit::UPSTREAM,
            });
        }
    };

    // FR-18's link: NIP-34 alone does not tie a patch to the story that motivated it.
    let mut refs = vec![patch_event.clone()];
    refs.extend(references.iter().cloned());

    let link = waggle_core::ArtifactEvent {
        kind_marker: waggle_core::ArtifactKind::Artifact,
        channel_id: channel.to_string(),
        artifact_type: Some("patch".into()),
        module: None,
        story: None,
        priority: None,
        references: refs,
        from_role: Some(role.to_string()),
        to_role: None,
        body: format!("Patch {patch_event} published to repo {repo_id} (NIP-34 kind:1617)."),
    };

    let link_event =
        match waggle_hive::events::publish_artifact(root, role, relay, &link, &uuid_like()) {
            Ok(p) => p.event_id,
            Err(e) => {
                eprintln!("error: patch published but linking failed: {e}");
                return ExitCode::from(exit::UPSTREAM);
            }
        };

    let report = PatchReport {
        patch_event,
        link_event,
        repo_id: repo_id.to_string(),
        kinds: vec![1617],
    };

    match format {
        Format::Json => emit(format, "patch", true, &report),
        Format::Text => {
            println!("patch  {}  (NIP-34 kind:1617)", report.patch_event);
            println!("link   {}  in channel {channel}", report.link_event);
        }
    }
    ExitCode::from(exit::OK)
}

#[derive(Serialize)]
struct PublishReport {
    event_id: String,
    pubkey: String,
    marker: String,
    transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blob_url: Option<String>,
    tags: Vec<Vec<String>>,
}

#[allow(clippy::too_many_arguments)]
fn run_publish(
    root: &std::path::Path,
    role: &str,
    channel: &str,
    marker: &str,
    artifact_type: &Option<String>,
    module: &Option<String>,
    story: &Option<String>,
    priority: &Option<String>,
    references: &[String],
    from_role: &Option<String>,
    to_role: &Option<String>,
    body: &str,
    relay: &str,
    format: Format,
) -> ExitCode {
    let kind_marker = match marker {
        "artifact" => waggle_core::ArtifactKind::Artifact,
        "handoff" => waggle_core::ArtifactKind::Handoff,
        "verdict" => waggle_core::ArtifactKind::Verdict,
        "gate-record" => waggle_core::ArtifactKind::GateRecord,
        other => {
            eprintln!(
                "error: {other:?} is not a marker — expected artifact, handoff, verdict, or gate-record"
            );
            return ExitCode::from(exit::USER);
        }
    };

    let priority = match priority {
        Some(p) => match waggle_core::Priority::parse(p) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(exit::USER);
            }
        },
        None => None,
    };

    let body_text = if body == "-" {
        use std::io::Read as _;
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("error: could not read stdin: {e}");
            return ExitCode::from(exit::SYSTEM);
        }
        buf
    } else {
        body.to_string()
    };

    let artifact = waggle_core::ArtifactEvent {
        kind_marker,
        channel_id: channel.to_string(),
        artifact_type: artifact_type.clone(),
        module: module.clone(),
        story: story.clone(),
        priority,
        references: references.to_vec(),
        from_role: from_role.clone(),
        to_role: to_role.clone(),
        body: body_text,
    };

    // A fresh nonce per request; the relay treats a repeat as a replay otherwise.
    let nonce = uuid_like();

    match waggle_hive::events::publish_artifact(root, role, relay, &artifact, &nonce) {
        Ok(p) => {
            let (transport, sha256, blob_url) = match &p.transport {
                waggle_hive::Transport::Inline => ("inline".to_string(), None, None),
                waggle_hive::Transport::Reference {
                    sha256,
                    url,
                    bytes: _,
                } => (
                    "reference".to_string(),
                    Some(sha256.clone()),
                    Some(url.clone()),
                ),
            };
            let report = PublishReport {
                event_id: p.event_id,
                pubkey: p.pubkey,
                marker: marker.to_string(),
                transport,
                sha256,
                blob_url,
                tags: artifact.tags(),
            };
            match format {
                Format::Json => emit(format, "publish", true, &report),
                Format::Text => {
                    println!("published {} {}", report.marker, report.event_id);
                    println!("  signed by {}", report.pubkey);
                    println!("  transport {}", report.transport);
                    if let (Some(h), Some(u)) = (&report.sha256, &report.blob_url) {
                        println!("  sha256    {h}");
                        println!("  blob      {u}");
                    }
                }
            }
            ExitCode::from(exit::OK)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(match e {
                waggle_hive::events::EventError::Invalid(_)
                | waggle_hive::events::EventError::NotProvisioned { .. } => exit::USER,
                _ => exit::UPSTREAM,
            })
        }
    }
}

#[derive(Serialize)]
struct TrailReport {
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<String>,
    count: usize,
    events: Vec<serde_json::Value>,
}

fn run_trail(
    root: &std::path::Path,
    role: &str,
    channel: &str,
    priority: Option<&str>,
    relay: &str,
    format: Format,
) -> ExitCode {
    // Priority rides a `t` tag because NIP-01 only indexes single-letter tags.
    let (letter, value) = match priority {
        Some(p) => match waggle_core::Priority::parse(p) {
            Ok(v) => ('t', v.tag_value().to_string()),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(exit::USER);
            }
        },
        None => ('t', waggle_core::artifact::TAG_WAGGLE.to_string()),
    };

    match waggle_hive::events::query_by_tag(
        root,
        role,
        relay,
        channel,
        letter,
        &value,
        &uuid_like(),
    ) {
        Ok(events) => {
            let report = TrailReport {
                channel: channel.to_string(),
                priority: priority.map(str::to_string),
                count: events.len(),
                events,
            };
            match format {
                Format::Json => emit(format, "trail", true, &report),
                Format::Text => {
                    println!("{} event(s)", report.count);
                    for e in &report.events {
                        let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                        let signed = e.get("sig").and_then(|v| v.as_str()).is_some();
                        let first = e
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .lines()
                            .next()
                            .unwrap_or("");
                        println!(
                            "  {}  sig:{}  {}",
                            &id[..12.min(id.len())],
                            if signed { "yes" } else { "NO" },
                            first
                        );
                    }
                }
            }
            ExitCode::from(exit::OK)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(exit::UPSTREAM)
        }
    }
}

#[derive(Serialize)]
struct ModuleView {
    module: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<String>,
    agents: Vec<AgentView>,
}

#[derive(Serialize)]
struct AgentView {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
}

#[derive(Serialize)]
struct ModulesReport {
    method_version: String,
    modules: Vec<ModuleView>,
    /// Skills carrying an [agent] block the registry does not list. Reported, never
    /// compiled — they are usually workflow-shaped skills, not personas (AD-6).
    unregistered_agent_blocks: Vec<String>,
}

fn run_modules(root: &std::path::Path, format: Format) -> ExitCode {
    let installation = match waggle_method::detect(root) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };
    let registry = match waggle_method::registry::read(root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };

    let tool_dirs = waggle_method::descriptors::tool_dirs(&installation.ides);
    let tool_dir = tool_dirs.first().cloned().unwrap_or_default();

    let modules: Vec<ModuleView> = waggle_method::registry::modules(&registry)
        .into_iter()
        .map(|m| {
            let meta = installation.modules.iter().find(|im| im.name == m);
            ModuleView {
                version: meta.map(|x| x.version_raw.clone()).unwrap_or_default(),
                source: meta.and_then(|x| x.source.clone()),
                sha: meta.and_then(|x| x.sha.clone()),
                agents: waggle_method::registry::agents_for_module(&registry, &m)
                    .into_iter()
                    .map(|id| {
                        let a = &registry[&id];
                        AgentView {
                            id,
                            name: a.name.clone(),
                            title: a.title.clone(),
                            icon: a.icon.clone(),
                        }
                    })
                    .collect(),
                module: m,
            }
        })
        .collect();

    let report = ModulesReport {
        method_version: installation.version_raw.clone(),
        unregistered_agent_blocks: waggle_method::registry::unregistered_agent_blocks(
            root, &tool_dir, &registry,
        ),
        modules,
    };

    match format {
        Format::Json => emit(format, "modules", true, &report),
        Format::Text => {
            println!("method {}", report.method_version);
            for m in &report.modules {
                println!("\n{} {}", m.module, m.version);
                if let Some(sha) = &m.sha {
                    println!(
                        "  provenance {} @ {}",
                        m.source.clone().unwrap_or_default(),
                        &sha[..7.min(sha.len())]
                    );
                }
                for a in &m.agents {
                    println!(
                        "  {:<26} {} {}",
                        a.id,
                        a.name,
                        a.icon.clone().unwrap_or_default()
                    );
                }
            }
            if !report.unregistered_agent_blocks.is_empty() {
                println!(
                    "\nnot personas (an [agent] block, but not registered): {}",
                    report.unregistered_agent_blocks.join(", ")
                );
            }
        }
    }
    ExitCode::from(exit::OK)
}

#[derive(Serialize)]
struct CompileOutput {
    module: String,
    agents: Vec<String>,
    pack_dir: String,
    files_written: Vec<String>,
    skills_copied: Vec<String>,
    skills_skipped: Vec<String>,
    channel_templates: Vec<String>,
    reports: Vec<waggle_core::CompileReport>,
    /// Kind-policy findings across every emitted artifact (FR-6, AD-8).
    lint: Vec<waggle_core::Finding>,
}

fn run_compile(
    root: &std::path::Path,
    module: &str,
    agent: Option<&str>,
    out_dir: &std::path::Path,
    format: Format,
    allow_missing_skills: bool,
) -> ExitCode {
    // AD-19: resolve the tool directory from the installation manifest, never hard-coded.
    let installation = match waggle_method::detect(root) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };
    let registry = match waggle_method::registry::read(root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::UPSTREAM);
        }
    };

    let tool_dirs = waggle_method::descriptors::tool_dirs(&installation.ides);
    let Some(tool_dir) = tool_dirs.first() else {
        eprintln!("error: the installation manifest records no tool directories");
        return ExitCode::from(exit::UPSTREAM);
    };

    // One agent, or every agent the module registers. The registry is authoritative:
    // enumerating [agent] blocks from disk would sweep up workflow-shaped skills that
    // are not personas at all.
    let agent_ids: Vec<String> = match agent {
        Some(a) => vec![a.to_string()],
        None => waggle_method::registry::agents_for_module(&registry, module),
    };

    // waggle's own template data, if the module ships any (AD-16: data, not code).
    let templates_path = root.join("templates").join(module).join("channels.json");
    let channel_sources: Option<Vec<waggle_emit::channels::ChannelTemplateSource>> =
        if templates_path.exists() {
            match std::fs::read_to_string(&templates_path)
                .map_err(|e| e.to_string())
                .and_then(|raw| serde_json::from_str(&raw).map_err(|e| e.to_string()))
            {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("error: {}: {e}", templates_path.display());
                    return ExitCode::from(exit::USER);
                }
            }
        } else {
            None
        };

    if agent_ids.is_empty() && channel_sources.is_none() {
        eprintln!(
            "error: module {module:?} registers no agents and ships no channel templates. \
             Known agent modules: {}",
            waggle_method::registry::modules(&registry).join(", ")
        );
        return ExitCode::from(exit::USER);
    }

    let module_version = installation
        .modules
        .iter()
        .find(|m| m.name == module)
        .map(|m| m.version_raw.clone())
        .unwrap_or_else(|| {
            if channel_sources.is_some() {
                "0.0.0".to_string()
            } else {
                eprintln!("warning: module {module:?} is not in the installation manifest");
                "0.0.0".to_string()
            }
        });

    let mut packs = Vec::new();
    let mut reports = Vec::new();

    for id in &agent_ids {
        let descriptor = match waggle_method::descriptors::resolve_agent(root, tool_dir, id) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error: {id}: {e}");
                return ExitCode::from(exit::UPSTREAM);
            }
        };
        // The registry holds the description; customize.toml does not.
        let description = registry
            .get(id)
            .and_then(|a| a.description.clone())
            .unwrap_or_else(|| {
                format!("Compiled from the {module} module of a BMAD Method installation.")
            });

        match waggle_core::compile_persona(id, &descriptor, &description) {
            Ok((p, r)) => {
                packs.push(p);
                reports.push(r);
            }
            Err(e) => {
                eprintln!("error: {id}: {e}");
                return ExitCode::from(exit::UPSTREAM);
            }
        }
    }

    let all_module_agents = waggle_method::registry::agents_for_module(&registry, module);
    let all_agent_ids: Vec<String> = registry.keys().cloned().collect();

    let instructions = include_str!("../assets/instructions.md");
    let help_csv_path = root.join("_bmad/_config/bmad-help.csv");
    // Core surfaces only — keep agent packs menu-faithful (verify-compile counts skills).
    let always_skills: Vec<String> = if module == "core" {
        vec!["bmad-help".to_string(), "bmad-party-mode".to_string()]
    } else {
        Vec::new()
    };
    let meta = waggle_emit::PackMeta {
        module,
        module_version: &module_version,
        skills_source: &root.join(tool_dir),
        instructions,
        channel_templates: channel_sources.as_deref(),
        module_agent_ids: &all_module_agents,
        all_agent_ids: &all_agent_ids,
        help_csv: help_csv_path.exists().then_some(help_csv_path.as_path()),
        always_skills: &always_skills,
    };

    let outcome = match waggle_emit::emit_pack(out_dir, &packs, &meta, allow_missing_skills) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(exit::SYSTEM);
        }
    };

    // FR-6 / AD-8: lint every emitted artifact for kind-policy violations. Errors fail
    // the compile — a pack that would be unreadable by standard clients, or that collides
    // with a substrate-reserved range, is not worth shipping.
    let mut findings: Vec<waggle_core::Finding> = Vec::new();
    for path in &outcome.files_written {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        findings.extend(waggle_core::lint::scan(&name, &text));
    }
    let lint_failed = waggle_core::has_errors(&findings);

    let output = CompileOutput {
        module: module.to_string(),
        agents: agent_ids,
        pack_dir: outcome.pack_dir.display().to_string(),
        files_written: outcome
            .files_written
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        skills_copied: outcome.skills_copied,
        skills_skipped: outcome.skills_skipped,
        channel_templates: outcome.channel_templates,
        reports,
        lint: findings,
    };

    match format {
        Format::Json => emit(format, "compile", !lint_failed, &output),
        Format::Text => {
            println!(
                "compiled {} agent{} -> {}",
                output.agents.len(),
                if output.agents.len() == 1 { "" } else { "s" },
                output.pack_dir
            );
            println!("  skills       {} copied", output.skills_copied.len());
            if !output.skills_skipped.is_empty() {
                println!(
                    "  skills skip  {} (not on disk under tool skills dir)",
                    output.skills_skipped.join(", ")
                );
            }
            if output.channel_templates.is_empty() {
                println!("  channels     none (module ships no templates/{module}/channels.json)");
            } else {
                println!("  channels     {}", output.channel_templates.join(", "));
            }
            for r in &output.reports {
                let mut notes = Vec::new();
                if !r.prompt_only.is_empty() {
                    notes.push(format!("prompt-only: {}", r.prompt_only.join(",")));
                }
                if !r.unknown.is_empty() {
                    notes.push(format!("UNKNOWN: {}", r.unknown.join(",")));
                }
                if !r.dropped.is_empty() {
                    notes.push(format!(
                        "dropped: {}",
                        r.dropped
                            .iter()
                            .map(|d| d.field.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
                for w in &r.warnings {
                    notes.push(format!("warning: {w}"));
                }
                println!(
                    "  {:<26} {}",
                    r.agent_id,
                    if notes.is_empty() {
                        "ok".to_string()
                    } else {
                        notes.join(" | ")
                    }
                );
            }
        }
    }

    // Findings go to stderr in both formats: they are diagnostics, and stdout must stay
    // parseable (AD-20).
    for f in &output.lint {
        let tag = match f.severity {
            waggle_core::Severity::Error => "LINT ERROR",
            waggle_core::Severity::Warning => "lint warn ",
        };
        eprintln!("  {tag}   {}: {}", f.artifact, f.reason);
    }

    if lint_failed {
        eprintln!(
            "\ncompile failed: generated output violates the kind policy (AD-8). \
             Nothing was left half-written — the pack is on disk but must not be shipped."
        );
        return ExitCode::from(exit::USER);
    }

    ExitCode::from(exit::OK)
}

#[derive(Serialize)]
struct IdentityView {
    role: String,
    public_key: String,
    npub: String,
}

impl From<waggle_hive::AgentIdentity> for IdentityView {
    fn from(i: waggle_hive::AgentIdentity) -> Self {
        // Note what is absent: there is no secret field to forget to strip (AD-14).
        Self {
            role: i.role,
            public_key: i.public_key_hex,
            npub: i.npub,
        }
    }
}

#[derive(Serialize)]
struct IdentityListReport {
    key_dir: String,
    identities: Vec<IdentityView>,
}

fn run_identity(root: &std::path::Path, cmd: &IdentityCmd, format: Format) -> ExitCode {
    match cmd {
        IdentityCmd::Provision { role, force } => {
            match waggle_hive::identity::provision(root, role, *force) {
                Ok(id) => {
                    let view = IdentityView::from(id);
                    match format {
                        Format::Json => emit(format, "identity.provision", true, &view),
                        Format::Text => {
                            println!("role   {}", view.role);
                            println!("npub   {}", view.npub);
                            println!("pubkey {}", view.public_key);
                        }
                    }
                    if matches!(format, Format::Text) {
                        eprintln!(
                            "\nsecret key written to {}/{}.nsec (owner-only, gitignored).\n\
                             Never commit it, and never paste it anywhere.",
                            waggle_hive::identity::key_dir(root).display(),
                            role
                        );
                    }
                    ExitCode::from(exit::OK)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    // An existing identity is the caller's decision to make, not a
                    // system fault — USER, so scripts can distinguish it.
                    ExitCode::from(exit::USER)
                }
            }
        }
        IdentityCmd::List => {
            let identities: Vec<IdentityView> = waggle_hive::identity::list(root)
                .into_iter()
                .map(IdentityView::from)
                .collect();

            let report = IdentityListReport {
                key_dir: waggle_hive::identity::key_dir(root).display().to_string(),
                identities,
            };

            match format {
                Format::Json => emit(format, "identity.list", true, &report),
                Format::Text => {
                    if report.identities.is_empty() {
                        println!(
                            "no identities provisioned in {}\n  waggle identity provision --role tea",
                            report.key_dir
                        );
                    } else {
                        for i in &report.identities {
                            println!("{:<10} {}", i.role, i.npub);
                        }
                    }
                }
            }
            ExitCode::from(exit::OK)
        }
        IdentityCmd::PublishProfile {
            role,
            pack,
            relay,
            buzz_cli,
        } => {
            let (display_name, description) = match read_pack_persona(pack) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(exit::USER);
                }
            };
            let cli_path = buzz_cli
                .clone()
                .unwrap_or_else(|| root.join("vendor/buzz/target/release/buzz"));

            match waggle_hive::identity::publish_profile(
                root,
                role,
                &cli_path,
                relay,
                &display_name,
                &description,
            ) {
                Ok(out) => {
                    println!("{out}");
                    ExitCode::from(exit::OK)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(exit::UPSTREAM)
                }
            }
        }
        IdentityCmd::Register {
            role,
            member_role,
            buzz_admin,
        } => {
            let admin = buzz_admin
                .clone()
                .unwrap_or_else(|| root.join("vendor/buzz/target/release/buzz-admin"));
            match waggle_hive::identity::register_member(root, role, &admin, member_role) {
                Ok(waggle_hive::Registered::Added { pubkey }) => {
                    match format {
                        Format::Json => emit(
                            format,
                            "identity.register",
                            true,
                            &serde_json::json!({
                                "role": role,
                                "pubkey": pubkey,
                                "status": "added",
                                "member_role": member_role,
                            }),
                        ),
                        Format::Text => {
                            println!("registered {role} ({pubkey}) as {member_role}");
                        }
                    }
                    ExitCode::from(exit::OK)
                }
                Ok(waggle_hive::Registered::AlreadyMember { pubkey }) => {
                    match format {
                        Format::Json => emit(
                            format,
                            "identity.register",
                            true,
                            &serde_json::json!({
                                "role": role,
                                "pubkey": pubkey,
                                "status": "already-member",
                                "member_role": member_role,
                            }),
                        ),
                        Format::Text => {
                            println!("already a member: {role} ({pubkey})");
                        }
                    }
                    ExitCode::from(exit::OK)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(match e {
                        waggle_hive::IdentityError::RelayKeyMissing
                        | waggle_hive::IdentityError::NotProvisioned { .. } => exit::USER,
                        _ => exit::UPSTREAM,
                    })
                }
            }
        }
    }
}

fn run_runtime(root: &std::path::Path, cmd: &RuntimeCmd, format: Format) -> ExitCode {
    match cmd {
        RuntimeCmd::Emit {
            role,
            pack,
            persona,
            relay,
            max_sessions,
        } => {
            match waggle_hive::runtime::emit_config(root, role, pack, persona, relay, *max_sessions)
            {
                Ok((cfg, path)) => {
                    match format {
                        Format::Json => emit(format, "runtime.emit", true, &cfg),
                        Format::Text => {
                            println!("wrote {}", path.display());
                            println!("role          {}", cfg.role);
                            println!("npub          {}", cfg.npub);
                            println!("pack          {}", cfg.pack_dir);
                            println!("max_sessions  {}", cfg.max_sessions);
                            println!("live turn needs: {}", cfg.required_env.join(", "));
                        }
                    }
                    ExitCode::from(exit::OK)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(exit::USER)
                }
            }
        }
        RuntimeCmd::PublishAgent {
            role,
            pack,
            persona,
            relay,
            max_sessions,
        } => {
            match waggle_hive::runtime::publish_persona_and_agent(
                root,
                role,
                pack,
                persona,
                relay,
                *max_sessions,
                "anyone",
                &uuid_like(),
            ) {
                Ok((def, p)) => {
                    match format {
                        Format::Json => emit(
                            format,
                            "runtime.publish-agent",
                            true,
                            &serde_json::json!({
                                "persona_event_id": def.event_id,
                                "event_id": p.event_id,
                                "pubkey": p.pubkey,
                                "kind": 30177,
                                "persona_kind": 30175,
                                "persona_id": persona,
                            }),
                        ),
                        Format::Text => {
                            println!("published persona definition {}", def.event_id);
                            println!("published managed-agent {}", p.event_id);
                            println!("  persona   {persona}");
                            println!("  signed by {} (owner)", p.pubkey);
                        }
                    }
                    ExitCode::from(exit::OK)
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(exit::UPSTREAM)
                }
            }
        }
        RuntimeCmd::Supervisor {
            relay,
            buzz_acp,
            agent_command,
            agent_owner,
            max_concurrent,
            respond_to,
            idle_timeout,
            channels,
            auth_role,
        } => {
            let buzz_acp = buzz_acp.clone().unwrap_or_else(|| {
                let debug = root.join("vendor/buzz/target/debug/buzz-acp");
                let release = root.join("vendor/buzz/target/release/buzz-acp");
                if debug.exists() {
                    debug
                } else {
                    release
                }
            });
            let mut channel_ids = channels.clone();
            if channel_ids.is_empty() {
                if let Ok(env_channels) = std::env::var("WAGGLE_SUPERVISOR_CHANNELS") {
                    channel_ids = env_channels
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect();
                }
            }
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            {
                let stop = stop.clone();
                let _ = ctrlc_set(stop);
            }
            let opts = waggle_hive::supervisor::SupervisorOptions {
                project_root: root.to_path_buf(),
                relay_url: relay.clone(),
                buzz_acp,
                agent_command: agent_command.clone(),
                agent_owner: agent_owner.clone(),
                max_concurrent: *max_concurrent,
                respond_to: respond_to.clone(),
                idle_timeout_secs: *idle_timeout,
                channel_ids,
                auth_role: auth_role.clone(),
            };
            match waggle_hive::supervisor::run(opts, stop) {
                Ok(()) => ExitCode::from(exit::OK),
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(exit::SYSTEM)
                }
            }
        }
    }
}

fn ctrlc_set(stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Result<(), String> {
    ctrlc::set_handler(move || {
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        eprintln!("supervisor: stopping…");
    })
    .map_err(|e| e.to_string())
}

/// Pull `display_name` and `description` out of a compiled pack's persona frontmatter.
fn read_pack_persona(pack: &std::path::Path) -> Result<(String, String), String> {
    let agents = pack.join("agents");
    let entry = std::fs::read_dir(&agents)
        .map_err(|e| format!("cannot read {}: {e}", agents.display()))?
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().ends_with(".persona.md"))
        .ok_or_else(|| format!("no .persona.md found in {}", agents.display()))?;
    let p =
        waggle_hive::runtime::read_pack_persona_file(&entry.path()).map_err(|e| e.to_string())?;
    Ok((p.display_name, p.description))
}

fn run_preflight(
    root: &std::path::Path,
    substrate_path: &std::path::Path,
    allow_unsupported: bool,
    format: Format,
) -> ExitCode {
    // AD-18: the supported range lives in exactly one committed location.
    let pins_path = root.join("BUZZ_VERSION");
    let pins_raw = match std::fs::read_to_string(&pins_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "error: cannot read the pins file at {}: {e}\n\
                 waggle must be run from the project root, or pass --root.",
                pins_path.display()
            );
            return ExitCode::from(exit::USER);
        }
    };
    let pins = waggle_core::parse_pins(&pins_raw);

    let buzz_range = match waggle_core::range(&pins, "BUZZ_SUPPORTED") {
        Some(r) => r,
        None => {
            eprintln!(
                "error: BUZZ_SUPPORTED missing or malformed in {}",
                pins_path.display()
            );
            return ExitCode::from(exit::USER);
        }
    };
    let bmad_range = match waggle_core::range(&pins, "BMAD_SUPPORTED") {
        Some(r) => r,
        None => {
            eprintln!(
                "error: BMAD_SUPPORTED missing or malformed in {}",
                pins_path.display()
            );
            return ExitCode::from(exit::USER);
        }
    };
    let pinned_tag = pins
        .get("BUZZ_VERSION")
        .cloned()
        .unwrap_or_else(|| "the pinned tag".to_string());

    // --- substrate ---
    let (substrate_report, integrity) = match waggle_hive::detect(substrate_path, &pinned_tag) {
        Ok(found) => {
            let compat = buzz_range.check(found.version);
            let integrity = waggle_hive::verify_integrity(substrate_path)
                .ok()
                .map(|modified| IntegrityReport {
                    clean: modified.is_empty(),
                    modified,
                });
            (
                ComponentReport {
                    name: "buzz",
                    found: Some(found.version_raw),
                    expected: buzz_range.to_string(),
                    compatibility: Some(compat),
                    ok: compat.is_supported(),
                    error: None,
                },
                integrity,
            )
        }
        Err(e) => (component_error("buzz", &buzz_range, e.to_string()), None),
    };

    // --- method ---
    let method_report = match waggle_method::detect(root) {
        Ok(found) => {
            let compat = bmad_range.check(found.version);
            ComponentReport {
                name: "bmad-method",
                found: Some(found.version_raw),
                expected: bmad_range.to_string(),
                compatibility: Some(compat),
                ok: compat.is_supported(),
                error: None,
            }
        }
        Err(e) => component_error("bmad-method", &bmad_range, e.to_string()),
    };

    let all_ok = substrate_report.ok && method_report.ok;
    let integrity_ok = integrity.as_ref().map(|i| i.clean).unwrap_or(true);
    let effective_ok = (all_ok && integrity_ok) || allow_unsupported;

    let report = PreflightReport {
        substrate: substrate_report,
        method: method_report,
        substrate_integrity: integrity,
        overridden: allow_unsupported && !(all_ok && integrity_ok),
    };

    emit(format, "preflight", effective_ok, &report);

    if report.overridden {
        eprintln!(
            "\nwarning: --allow-unsupported is set. waggle is running against versions it \
             was not verified with.\n         Generated output may be wrong in ways that do \
             not surface as errors."
        );
    }

    if effective_ok {
        ExitCode::from(exit::OK)
    } else {
        ExitCode::from(exit::UPSTREAM)
    }
}

fn component_error(name: &'static str, range: &VersionRange, error: String) -> ComponentReport {
    ComponentReport {
        name,
        found: None,
        expected: range.to_string(),
        compatibility: None,
        ok: false,
        error: Some(error),
    }
}
