//! The produced architecture (the map read from the code) against the
//! intended one (the human's module rules) and against the committed
//! `docs/map.md`; deterministic, verified.

use serde::Serialize;
use std::collections::BTreeSet;

use crate::domain::architecture::Rules;
use crate::domain::map_doc;
use crate::domain::module_map::ModuleMap;

/// How the current map differs from the human's rules and the committed map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    /// The number of module rules the human declared.
    pub rules: usize,
    /// The dependencies that break a module rule, in the map's dependency
    /// order.
    pub violations: Vec<RuleBreak>,
    /// The edges that differ from the committed `docs/map.md`, when one is
    /// present.
    pub since_map: Option<MapChange>,
}

/// A dependency that breaks one of the human's module rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleBreak {
    pub from: String,
    pub to: String,
    pub imports: usize,
    /// The rule it breaks, as the human wrote it (normalised to no trailing
    /// slash).
    pub rule: String,
}

/// The edges added to or removed from the committed map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapChange {
    pub added: Vec<Edge>,
    pub removed: Vec<Edge>,
}

/// One dependency edge, by module id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub from: String,
    pub to: String,
}

/// Mark the map's violating dependencies and measure its drift.
pub fn measure(map: &mut ModuleMap, rules: &Rules, committed: Option<&str>) -> Drift {
    let module_rules = rules.module_rules();
    let mut violations = Vec::new();

    for dependency in &mut map.dependencies {
        if let Some(rule) = module_rules
            .iter()
            .find(|rule| rule.matches(&dependency.from, &dependency.to))
        {
            dependency.violation = true;
            violations.push(RuleBreak {
                from: dependency.from.clone(),
                to: dependency.to.clone(),
                imports: dependency.imports,
                rule: rule.to_string(),
            });
        }
    }

    Drift {
        rules: module_rules.len(),
        violations,
        since_map: committed.map(|committed| {
            let current: BTreeSet<(String, String)> = map
                .dependencies
                .iter()
                .map(|dependency| (dependency.from.clone(), dependency.to.clone()))
                .collect();
            let committed_edges = map_doc::edges(committed);

            MapChange {
                added: current
                    .difference(&committed_edges)
                    .map(|(from, to)| Edge {
                        from: from.clone(),
                        to: to.clone(),
                    })
                    .collect(),
                removed: committed_edges
                    .difference(&current)
                    .map(|(from, to)| Edge {
                        from: from.clone(),
                        to: to.clone(),
                    })
                    .collect(),
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::map_doc::render;
    use crate::domain::module_map::{Dependency, Module, ModuleMap};

    fn module(id: &str) -> Module {
        Module {
            id: id.into(),
            ..Default::default()
        }
    }

    fn map_with(modules: &[&str], deps: &[(&str, &str, usize)]) -> ModuleMap {
        ModuleMap {
            modules: modules.iter().map(|id| module(id)).collect(),
            dependencies: deps
                .iter()
                .map(|(from, to, imports)| Dependency {
                    from: (*from).into(),
                    to: (*to).into(),
                    imports: *imports,
                    ..Default::default()
                })
                .collect(),
            groups: vec![],
        }
    }

    #[test]
    fn marks_and_lists_a_violation() {
        let mut map = map_with(&["a", "b"], &[("a", "b", 3)]);
        let rules = Rules::parse("a never imports b");

        let drift = measure(&mut map, &rules, None);

        assert_eq!(drift.rules, 1);
        assert_eq!(
            drift.violations,
            vec![RuleBreak {
                from: "a".into(),
                to: "b".into(),
                imports: 3,
                rule: "a never imports b".into(),
            }]
        );
        assert!(map.dependencies[0].violation);
        assert!(drift.since_map.is_none());
    }

    #[test]
    fn catches_a_child_module_with_a_parent_rule() {
        let mut map = map_with(&["a/c", "b/d"], &[("a/c", "b/d", 1)]);
        let rules = Rules::parse("a/ never imports b/");

        let drift = measure(&mut map, &rules, None);

        assert_eq!(drift.rules, 1);
        assert_eq!(drift.violations.len(), 1);
        assert_eq!(drift.violations[0].rule, "a never imports b");
        assert!(map.dependencies[0].violation);
    }

    #[test]
    fn no_rules_mean_no_violations() {
        let mut map = map_with(&["a", "b"], &[("a", "b", 1)]);
        let rules = Rules::parse("# none\n");

        let drift = measure(&mut map, &rules, None);

        assert_eq!(drift.rules, 0);
        assert!(drift.violations.is_empty());
        assert!(!map.dependencies[0].violation);
    }

    #[test]
    fn reports_added_and_removed_edges_against_committed_map() {
        let mut map = map_with(&["a", "b", "c", "d"], &[("a", "b", 1), ("b", "c", 1)]);
        let committed_map = map_with(&["a", "b", "c", "d"], &[("a", "b", 1), ("c", "d", 1)]);
        let committed = render(&committed_map);

        let drift = measure(&mut map, &Rules::parse(""), Some(&committed));

        assert_eq!(
            drift.since_map,
            Some(MapChange {
                added: vec![Edge {
                    from: "b".into(),
                    to: "c".into(),
                }],
                removed: vec![Edge {
                    from: "c".into(),
                    to: "d".into(),
                }],
            })
        );
    }
}
