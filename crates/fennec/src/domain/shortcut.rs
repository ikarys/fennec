//! Keyboard shortcut catalog, default bindings, and validation of human overrides.

use serde::Serialize;
use std::collections::BTreeMap;

/// Each action the UI binds, with its default combo, in the Settings page's order.
pub const ACTIONS: &[(&str, &str)] = &[
    ("newChat", "Alt+Shift+Digit0"),
    ("agentPlan", "Alt+Shift+Digit1"),
    ("agentBuilder", "Alt+Shift+Digit2"),
    ("agentAsk", "Alt+Shift+Digit3"),
    ("newSession", "Alt+Shift+KeyN"),
    ("goHome", "Alt+Shift+KeyH"),
    ("goProjects", "Alt+Shift+KeyP"),
    ("goChats", "Alt+Shift+KeyC"),
    ("goTasks", "Alt+Shift+KeyT"),
    ("goWorkflows", "Alt+Shift+KeyW"),
    ("goInsight", "Alt+Shift+KeyI"),
    ("goSettings", "Alt+Shift+KeyS"),
    ("nextTab", "Alt+Shift+ArrowRight"),
    ("prevTab", "Alt+Shift+ArrowLeft"),
    ("closeTab", "Alt+Shift+KeyX"),
    ("palette", "Mod+KeyK"),
    ("split", "Mod+Backslash"),
];

/// A resolved keyboard shortcut binding shown in Settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    /// The action id, matching `ACTIONS`.
    pub action: &'static str,
    /// The effective combo: the stored override if present, otherwise the default.
    pub combo: String,
    /// The catalog default combo.
    pub default: &'static str,
}

/// Resolve the effective bindings from stored overrides.
pub fn resolve(overrides: &BTreeMap<String, String>) -> Vec<Binding> {
    ACTIONS
        .iter()
        .map(|(action, default)| Binding {
            action,
            combo: overrides
                .get(*action)
                .cloned()
                .unwrap_or_else(|| (*default).to_string()),
            default,
        })
        .collect()
}

/// Validate stored overrides against the catalog.
pub fn check(overrides: &BTreeMap<String, String>) -> Result<(), String> {
    for key in overrides.keys() {
        if !ACTIONS.iter().any(|(action, _)| *action == key.as_str()) {
            return Err(format!("unknown shortcut action {key}"));
        }
    }

    let resolved = resolve(overrides);
    for binding in &resolved {
        if !valid_combo(&binding.combo) {
            return Err(format!(
                "invalid shortcut for {}: {}",
                binding.action, binding.combo
            ));
        }
    }

    for i in 0..resolved.len() {
        for j in (i + 1)..resolved.len() {
            let a = &resolved[i];
            let b = &resolved[j];
            if !a.combo.is_empty() && a.combo == b.combo {
                return Err(format!("{} and {} share {}", a.action, b.action, a.combo));
            }
        }
    }

    Ok(())
}

fn valid_combo(combo: &str) -> bool {
    if combo.is_empty() {
        return true;
    }

    let parts: Vec<&str> = combo.split('+').collect();
    if parts.len() > 4 {
        return false;
    }

    let key = parts.last().unwrap();
    if !key_allowed(key) {
        return false;
    }

    let modifiers = &parts[..parts.len() - 1];
    if modifiers.is_empty() {
        return is_fkey(key);
    }

    if !is_fkey(key) && !modifiers.iter().any(|m| *m == "Mod" || *m == "Alt") {
        return false;
    }

    let allowed = ["Mod", "Alt", "Shift"];
    let mut i = 0;
    for m in modifiers {
        while i < allowed.len() && allowed[i] != *m {
            i += 1;
        }
        if i == allowed.len() {
            return false;
        }
        i += 1;
    }

    true
}

fn key_allowed(key: &str) -> bool {
    if let Some(rest) = key.strip_prefix("Digit") {
        return rest.chars().count() == 1
            && rest.chars().next().is_some_and(|c| c.is_ascii_digit());
    }

    if let Some(rest) = key.strip_prefix("Key") {
        return rest.chars().count() == 1
            && rest.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    }

    if let Some(rest) = key.strip_prefix('F') {
        return rest.parse::<usize>().is_ok_and(|n| (1..=12).contains(&n));
    }

    matches!(
        key,
        "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "Backslash"
            | "Slash"
            | "Comma"
            | "Period"
            | "Semicolon"
            | "Quote"
            | "BracketLeft"
            | "BracketRight"
            | "Minus"
            | "Equal"
            | "Backquote"
            | "Enter"
            | "Space"
    )
}

fn is_fkey(key: &str) -> bool {
    key.strip_prefix('F')
        .is_some_and(|rest| rest.parse::<usize>().is_ok_and(|n| (1..=12).contains(&n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_pass_check_and_have_no_duplicates() {
        let overrides = BTreeMap::<String, String>::new();
        assert!(check(&overrides).is_ok());

        let resolved = resolve(&overrides);
        for i in 0..resolved.len() {
            for j in (i + 1)..resolved.len() {
                assert_ne!(resolved[i].combo, resolved[j].combo);
            }
        }
    }

    #[test]
    fn override_replaces_default_in_resolve() {
        let mut overrides = BTreeMap::<String, String>::new();
        overrides.insert("palette".to_string(), "Alt+Shift+KeyK".to_string());

        let binding = resolve(&overrides)
            .into_iter()
            .find(|b| b.action == "palette")
            .unwrap();

        assert_eq!(binding.combo, "Alt+Shift+KeyK");
        assert_eq!(binding.default, "Mod+KeyK");
    }

    #[test]
    fn empty_override_disables() {
        let mut overrides = BTreeMap::<String, String>::new();
        overrides.insert("palette".to_string(), String::new());

        assert!(check(&overrides).is_ok());
        let binding = resolve(&overrides)
            .into_iter()
            .find(|b| b.action == "palette")
            .unwrap();
        assert_eq!(binding.combo, "");
    }

    #[test]
    fn unknown_action_rejected() {
        let mut overrides = BTreeMap::<String, String>::new();
        overrides.insert("nope".to_string(), "Mod+KeyK".to_string());

        assert_eq!(
            check(&overrides).unwrap_err(),
            "unknown shortcut action nope"
        );
    }

    #[test]
    fn invalid_combos_rejected() {
        for combo in [
            "KeyA",
            "Shift+KeyA",
            "Alt+Alt+KeyA",
            "Shift+Alt+KeyA",
            "Alt+Escape",
            "Alt+",
            "Ctrl+KeyA",
        ] {
            let mut overrides = BTreeMap::<String, String>::new();
            overrides.insert("palette".to_string(), combo.to_string());
            assert_eq!(
                check(&overrides).unwrap_err(),
                format!("invalid shortcut for palette: {combo}")
            );
        }
    }

    #[test]
    fn bare_fkey_accepted() {
        let mut overrides = BTreeMap::<String, String>::new();
        overrides.insert("palette".to_string(), "F5".to_string());

        assert!(check(&overrides).is_ok());
    }

    #[test]
    fn duplicate_combo_rejected() {
        let mut overrides = BTreeMap::<String, String>::new();
        overrides.insert("palette".to_string(), "Alt+Shift+Digit0".to_string());

        assert_eq!(
            check(&overrides).unwrap_err(),
            "newChat and palette share Alt+Shift+Digit0"
        );
    }

    #[test]
    fn moving_default_frees_combo() {
        let mut overrides = BTreeMap::<String, String>::new();
        overrides.insert("newChat".to_string(), "Alt+Shift+Digit9".to_string());
        overrides.insert("palette".to_string(), "Alt+Shift+Digit0".to_string());

        assert!(check(&overrides).is_ok());
    }
}
