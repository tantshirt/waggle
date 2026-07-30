//! Structured stdout / text rendering (AD-20).

use serde::Serialize;

#[derive(Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Text,
    Json,
}

/// One versioned envelope shared by every command (AD-20 consistency convention).
#[derive(Serialize)]
pub struct Envelope<T: Serialize> {
    pub schema: &'static str,
    pub command: &'static str,
    pub ok: bool,
    #[serde(flatten)]
    pub data: T,
}

pub fn emit<T: Serialize>(format: Format, command: &'static str, ok: bool, data: &T) {
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
pub fn print_text<T: Serialize>(data: &T) {
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
