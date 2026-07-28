//! BMAD's three-layer customization merge (AD-5).
//!
//! Pure: operates on parsed TOML values, never touches the filesystem.
//!
//! **This is the highest-risk correctness surface in waggle.** Choosing Rust over Python
//! meant we could not reuse BMAD's own `resolve_customization.py`, so these rules are a
//! reimplementation. Getting them subtly wrong produces a persona that is *plausible but
//! wrong* — no error, no crash, just an agent missing three principles nobody notices.
//! That is why AD-5 makes the differential test against the real resolver mandatory and
//! non-skippable, and why this module is commented far more heavily than its size warrants.
//!
//! The rules, applied base → team → user:
//!
//! | Kind | Rule |
//! |---|---|
//! | scalar | later layer wins outright |
//! | table | deep-merge, recursing per key |
//! | array of tables keyed by `code` or `id` | replace entries whose key matches; append new ones |
//! | any other array | append |
//!
//! Note how asymmetric that last pair is, and note that **arrays append rather than
//! replace** — the opposite of Buzz's persona-pack merge, which replaces wholesale. waggle
//! sits between two systems whose merge semantics disagree, so it must fully resolve on
//! this side and emit something flat.

use toml::Value;

/// Keys that identify an entry within an array of tables.
///
/// BMAD checks `code` first, then `id`. Order matters when a table carries both.
const IDENTITY_KEYS: [&str; 2] = ["code", "id"];

/// Merge `over` on top of `base`, returning the resolved value.
///
/// Call once per layer, in order: `merge(merge(base, team), user)`.
pub fn merge(base: Value, over: Value) -> Value {
    match (base, over) {
        // Tables deep-merge: keys present only in one side survive; shared keys recurse.
        (Value::Table(mut b), Value::Table(o)) => {
            for (k, ov) in o {
                let merged = match b.remove(&k) {
                    Some(bv) => merge(bv, ov),
                    None => ov,
                };
                b.insert(k, merged);
            }
            Value::Table(b)
        }

        (Value::Array(b), Value::Array(o)) => Value::Array(merge_arrays(b, o)),

        // Scalars, and any type mismatch, take the later layer. A type change between
        // layers is a authoring mistake, but silently keeping the older value would be
        // worse than honouring the override.
        (_, over) => over,
    }
}

fn merge_arrays(base: Vec<Value>, over: Vec<Value>) -> Vec<Value> {
    // "Array of tables keyed by code/id" is decided by the *data*, not by a schema, so we
    // detect it. A keyed array is one where every element is a table and at least one
    // carries an identity key. Anything else is a plain array and simply appends.
    let key = identity_key(&base, &over);

    let Some(key) = key else {
        let mut out = base;
        out.extend(over);
        return out;
    };

    let mut out = base;
    for ov in over {
        let ov_id = ov.get(key).and_then(Value::as_str).map(str::to_string);

        match ov_id {
            // Matching identity replaces the existing entry *in place*, preserving order.
            Some(id) => {
                let existing = out
                    .iter()
                    .position(|bv| bv.get(key).and_then(Value::as_str) == Some(id.as_str()));
                match existing {
                    Some(i) => out[i] = ov,
                    None => out.push(ov),
                }
            }
            // A table in a keyed array with no identity of its own cannot match
            // anything, so it can only append.
            None => out.push(ov),
        }
    }
    out
}

/// Which identity key, if any, governs this pair of arrays.
fn identity_key(base: &[Value], over: &[Value]) -> Option<&'static str> {
    let all_tables = base.iter().chain(over).all(Value::is_table);
    if !all_tables || (base.is_empty() && over.is_empty()) {
        return None;
    }
    IDENTITY_KEYS.into_iter().find(|k| {
        base.iter()
            .chain(over)
            .any(|v| v.get(k).and_then(Value::as_str).is_some())
    })
}

/// Merge an ordered list of layers. Earlier entries are lower precedence.
pub fn merge_layers<I: IntoIterator<Item = Value>>(layers: I) -> Option<Value> {
    layers.into_iter().reduce(merge)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse a TOML *document*. Note `Value::from_str` parses a single value expression,
    /// not a document, which is a genuinely easy mistake to make here.
    fn t(s: &str) -> Value {
        toml::from_str::<Value>(s).expect("test TOML should parse")
    }

    #[test]
    fn scalars_take_the_later_layer() {
        let out = merge(t(r#"icon = "A""#), t(r#"icon = "B""#));
        assert_eq!(out.get("icon").unwrap().as_str(), Some("B"));
    }

    #[test]
    fn keys_absent_from_the_override_survive() {
        let out = merge(
            t(r#"name = "Murat"
icon = "🧪""#),
            t(r#"icon = "X""#),
        );
        assert_eq!(out.get("name").unwrap().as_str(), Some("Murat"));
        assert_eq!(out.get("icon").unwrap().as_str(), Some("X"));
    }

    #[test]
    fn tables_deep_merge_rather_than_replace() {
        let base = t(r#"
[agent]
name = "Murat"
icon = "🧪"
"#);
        let over = t(r#"
[agent]
icon = "X"
"#);
        let out = merge(base, over);
        let agent = out.get("agent").unwrap();
        // The whole [agent] table must not be replaced by the one-key override.
        assert_eq!(agent.get("name").unwrap().as_str(), Some("Murat"));
        assert_eq!(agent.get("icon").unwrap().as_str(), Some("X"));
    }

    #[test]
    fn plain_arrays_append_they_do_not_replace() {
        // This is the rule most likely to be got wrong, and the failure is silent:
        // replacing would drop principles the base layer defined.
        let base = t(r#"principles = ["a", "b"]"#);
        let over = t(r#"principles = ["c"]"#);
        let out = merge(base, over);
        let got: Vec<_> = out["principles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(got, vec!["a", "b", "c"]);
    }

    #[test]
    fn keyed_arrays_replace_matching_and_append_new() {
        let base = t(r#"
[[menu]]
code = "TD"
description = "original"

[[menu]]
code = "GATE"
description = "gate"
"#);
        let over = t(r#"
[[menu]]
code = "TD"
description = "replaced"

[[menu]]
code = "NEW"
description = "added"
"#);
        let out = merge(base, over);
        let menu = out["menu"].as_array().unwrap();
        assert_eq!(menu.len(), 3, "TD replaced in place, NEW appended");
        assert_eq!(menu[0]["code"].as_str(), Some("TD"));
        assert_eq!(
            menu[0]["description"].as_str(),
            Some("replaced"),
            "matching code must replace, not duplicate"
        );
        assert_eq!(menu[1]["code"].as_str(), Some("GATE"), "order preserved");
        assert_eq!(menu[2]["code"].as_str(), Some("NEW"));
    }

    #[test]
    fn id_works_as_an_identity_key_too() {
        let base = t(r#"
[[items]]
id = "one"
v = 1
"#);
        let over = t(r#"
[[items]]
id = "one"
v = 2
"#);
        let out = merge(base, over);
        assert_eq!(out["items"].as_array().unwrap().len(), 1);
        assert_eq!(out["items"][0]["v"].as_integer(), Some(2));
    }

    #[test]
    fn tables_without_an_identity_key_just_append() {
        // Not every array of tables is keyed. Without code/id there is nothing to match on.
        let base = t(r#"
[[rows]]
a = 1
"#);
        let over = t(r#"
[[rows]]
a = 2
"#);
        let out = merge(base, over);
        assert_eq!(out["rows"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn three_layers_apply_in_order() {
        let out = merge_layers([
            t(r#"icon = "base"
principles = ["p1"]"#),
            t(r#"icon = "team"
principles = ["p2"]"#),
            t(r#"icon = "user""#),
        ])
        .unwrap();
        assert_eq!(out["icon"].as_str(), Some("user"));
        let ps: Vec<_> = out["principles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(
            ps,
            vec!["p1", "p2"],
            "array appends accumulate across layers"
        );
    }

    #[test]
    fn a_single_layer_resolves_to_itself() {
        let out = merge_layers([t(r#"icon = "only""#)]).unwrap();
        assert_eq!(out["icon"].as_str(), Some("only"));
        assert!(merge_layers(Vec::<Value>::new()).is_none());
    }
}
