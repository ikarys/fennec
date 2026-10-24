//! The project's architecture vision and sensitive zones, and the risk of a
//! change: which zones the paths it touches fall in. Read from the project's
//! HEAD, never from a world, so an agent cannot change what judges it; the
//! classification is code, never a model's opinion.
//!
//! `.fennec-os/zones.json`:
//! `{"zones": [{"name": "auth", "kind": "security", "paths": ["src/auth/**"]}]}`
//! where `kind` is `architecture`, `security` or `sensitive`. The vision and
//! the zones file are themselves always an architecture zone, and so are the
//! decision records (see `knowledge`).

use std::io;
use std::path::Path;

use serde_json::{Value, json};

use crate::rules::matches;
use crate::world::git;

pub const ZONES_FILE: &str = ".fennec-os/zones.json";
pub const VISION_FILE: &str = "ARCHITECTURE.md";

const KINDS: [&str; 3] = ["architecture", "security", "sensitive"];

#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub name: String,
    pub kind: String,
    pub paths: Vec<String>,
}

/// The zones at HEAD, the vision's own zone first. A missing file means no
/// zone beyond it; a malformed one is an error, never "no zones".
pub fn load(project: &Path) -> io::Result<Vec<Zone>> {
    let mut zones = vec![Zone {
        name: "vision".to_string(),
        kind: "architecture".to_string(),
        paths: vec![
            VISION_FILE.to_string(),
            ZONES_FILE.to_string(),
            format!("{}/**", crate::knowledge::ADR_DIR),
        ],
    }];
    let Ok(text) = git(project, &["show", &format!("HEAD:{ZONES_FILE}")]) else {
        return Ok(zones);
    };
    zones.extend(parse(&text)?);
    Ok(zones)
}

pub(crate) fn parse(text: &str) -> io::Result<Vec<Zone>> {
    let bad = |why: &str| io::Error::other(format!("{ZONES_FILE}: {why}"));
    let v: Value = serde_json::from_str(text).map_err(|e| bad(&e.to_string()))?;
    let list = v["zones"]
        .as_array()
        .ok_or_else(|| bad("no `zones` list"))?;
    list.iter()
        .map(|z| {
            let name = z["name"]
                .as_str()
                .ok_or_else(|| bad("a zone has no name"))?;
            let kind = z["kind"].as_str().unwrap_or("sensitive");
            if !KINDS.contains(&kind) {
                return Err(bad(&format!("zone {name}: unknown kind {kind}")));
            }
            let paths: Vec<String> = z["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p.as_str().map(str::to_string))
                .collect();
            if paths.is_empty() {
                return Err(bad(&format!("zone {name} has no paths")));
            }
            Ok(Zone {
                name: name.to_string(),
                kind: kind.to_string(),
                paths,
            })
        })
        .collect()
}

/// The vision at HEAD, if the project has one.
pub fn vision(project: &Path) -> Option<String> {
    git(project, &["show", &format!("HEAD:{VISION_FILE}")])
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Which zones `paths` fall in: `{"sensitive": bool, "zones": [{"name",
/// "kind", "paths": [the paths in it]}]}`. Verified: plain glob matching.
pub fn classify(zones: &[Zone], paths: &[String]) -> Value {
    let hits: Vec<Value> = zones
        .iter()
        .filter_map(|z| {
            let inside: Vec<&String> = paths
                .iter()
                .filter(|p| z.paths.iter().any(|pat| matches(pat, p)))
                .collect();
            (!inside.is_empty()).then(|| json!({"name": z.name, "kind": z.kind, "paths": inside}))
        })
        .collect();
    json!({"sensitive": !hits.is_empty(), "zones": hits})
}

/// Every zone's paths: what a task outside the zones may not touch.
pub fn protected(zones: &[Zone]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for p in zones.iter().flat_map(|z| &z.paths) {
        if !all.contains(p) {
            all.push(p.clone());
        }
    }
    all
}

/// What the agents are told of the zones, so a design names its files with
/// them in mind.
pub fn brief(zones: &[Zone]) -> String {
    let mut out = String::from("\n\nSensitive zones of this project (kind: paths):\n");
    for z in zones {
        out.push_str(&format!(
            "  - {} ({}): {}\n",
            z.name,
            z.kind,
            z.paths.join(", ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zones() -> Vec<Zone> {
        let mut z = vec![Zone {
            name: "vision".to_string(),
            kind: "architecture".to_string(),
            paths: vec![VISION_FILE.to_string(), ZONES_FILE.to_string()],
        }];
        z.extend(
            parse(r#"{"zones": [{"name": "auth", "kind": "security", "paths": ["src/auth/**"]}]}"#)
                .unwrap(),
        );
        z
    }

    #[test]
    fn a_path_in_a_zone_makes_the_change_sensitive() {
        let r = classify(&zones(), &["src/auth/login.rs".into(), "README.md".into()]);
        assert_eq!(r["sensitive"], true);
        assert_eq!(r["zones"][0]["name"], "auth");
        assert_eq!(r["zones"][0]["paths"], json!(["src/auth/login.rs"]));
        let r = classify(&zones(), &["src/main.rs".into()]);
        assert_eq!(r, json!({"sensitive": false, "zones": []}));
    }

    #[test]
    fn the_vision_is_always_an_architecture_zone() {
        let r = classify(&zones(), &[VISION_FILE.to_string()]);
        assert_eq!(r["zones"][0]["kind"], "architecture");
        assert!(protected(&zones()).contains(&ZONES_FILE.to_string()));
    }

    #[test]
    fn a_malformed_zones_file_is_an_error_not_no_zones() {
        assert!(parse("{}").is_err());
        assert!(parse(r#"{"zones": [{"name": "x", "paths": []}]}"#).is_err());
        assert!(parse(r#"{"zones": [{"name": "x", "kind": "fun", "paths": ["a"]}]}"#).is_err());
        let z = parse(r#"{"zones": [{"name": "x", "paths": ["a"]}]}"#).unwrap();
        assert_eq!(z[0].kind, "sensitive");
    }
}
