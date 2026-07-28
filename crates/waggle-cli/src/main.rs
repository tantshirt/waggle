//! The waggle command line — the only crate that wires the others (AD-1's dependency rule).
//!
//! **AD-20:** every command is machine-first. Structured output on stdout, diagnostics on
//! stderr, no interactive input required, and a fixed exit-code taxonomy.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use waggle_core::{Compatibility, VersionRange};

/// AD-20's exit-code taxonomy. Mirrors buzz-cli's shape so the two compose in scripts.
mod exit {
    /// Everything succeeded.
    pub const OK: u8 = 0;
    /// The caller asked for something impossible — bad flag, missing input.
    pub const USER: u8 = 1;
    /// An upstream contract was violated: version out of range, schema unrecognized.
    pub const UPSTREAM: u8 = 2;
    /// Something broke that is neither the caller's fault nor upstream's.
    pub const SYSTEM: u8 = 3;
}

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

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
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
}

/// One versioned envelope shared by every command (AD-20 consistency convention).
#[derive(Serialize)]
struct Envelope<T: Serialize> {
    schema: &'static str,
    command: &'static str,
    ok: bool,
    #[serde(flatten)]
    data: T,
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
    }
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

fn emit<T: Serialize>(format: Format, command: &'static str, ok: bool, data: &T) {
    match format {
        Format::Json => {
            let env = Envelope {
                schema: "waggle.v1",
                command,
                ok,
                data,
            };
            match serde_json::to_string_pretty(&env) {
                Ok(s) => println!("{s}"),
                Err(e) => eprintln!("error: could not serialize output: {e}"),
            }
        }
        Format::Text => print_text(data),
    }
}

/// Text rendering goes through JSON so the two formats can never disagree about content.
fn print_text<T: Serialize>(data: &T) {
    let Ok(v) = serde_json::to_value(data) else {
        eprintln!("error: could not render output");
        return;
    };

    for key in ["substrate", "method"] {
        let Some(c) = v.get(key) else { continue };
        let ok = c.get("ok").and_then(|b| b.as_bool()).unwrap_or(false);
        let mark = if ok { "ok  " } else { "FAIL" };
        let name = c.get("name").and_then(|s| s.as_str()).unwrap_or(key);
        let expected = c.get("expected").and_then(|s| s.as_str()).unwrap_or("?");

        match c.get("error").and_then(|s| s.as_str()) {
            Some(err) => println!("{mark} {name:<12} expected {expected}\n     {err}"),
            None => {
                let found = c.get("found").and_then(|s| s.as_str()).unwrap_or("?");
                let why = c
                    .get("compatibility")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                if ok {
                    println!("{mark} {name:<12} {found}  (expected {expected})");
                } else {
                    println!(
                        "{mark} {name:<12} {found}  (expected {expected}) — {}",
                        why.replace('_', " ")
                    );
                }
            }
        }
    }

    if let Some(i) = v.get("substrate_integrity") {
        let clean = i.get("clean").and_then(|b| b.as_bool()).unwrap_or(false);
        if clean {
            println!("ok   integrity    substrate checkout is unmodified (AD-2)");
        } else {
            println!("FAIL integrity    substrate checkout has modified tracked files (AD-2):");
            if let Some(list) = i.get("modified").and_then(|m| m.as_array()) {
                for line in list.iter().filter_map(|l| l.as_str()) {
                    println!("       {line}");
                }
            }
        }
    }
}
