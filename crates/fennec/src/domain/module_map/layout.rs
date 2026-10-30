//! Where the project map places its modules: one graph per group of modules
//! linked by dependencies, layers along the dependencies (cycles broken),
//! rows ordered to cross fewer edges.
use std::collections::{BTreeMap, BTreeSet};

use super::{Dependency, Module};
type Adj = BTreeMap<String, BTreeSet<String>>;

pub fn layout(mut modules: BTreeMap<String, Module>, dependencies: &[Dependency]) -> Vec<Module> {
    let mut members = groups(&modules, dependencies);
    for (group, ids) in members.iter_mut() {
        ids.sort();
        let (layers, kept) = layers(ids, dependencies);
        let rows = rows(ids, &layers, &kept);
        for id in ids {
            if let Some(m) = modules.get_mut(id) {
                m.group = group.clone();
                m.layer = layers.get(id).copied().unwrap_or(0);
                m.row = rows.get(id).copied().unwrap_or(0);
            }
        }
    }
    // The biggest graph first; modules without any dependency last.
    let mut names: Vec<&str> = members
        .keys()
        .map(String::as_str)
        .filter(|g| !g.is_empty())
        .collect();
    names.sort_by(|a, b| {
        members[*b]
            .len()
            .cmp(&members[*a].len())
            .then_with(|| a.cmp(b))
    });
    names.extend(members.keys().map(String::as_str).filter(|g| g.is_empty()));
    let mut out = Vec::new();
    for name in names {
        let mut group: Vec<Module> = members[name]
            .iter()
            .filter_map(|id| modules.remove(id))
            .collect();
        group.sort_by_key(|m| (m.layer, m.row));
        out.extend(group);
    }
    out
}

fn groups(
    modules: &BTreeMap<String, Module>,
    dependencies: &[Dependency],
) -> BTreeMap<String, Vec<String>> {
    let mut adj = Adj::new();
    for d in dependencies {
        if modules.contains_key(&d.from) && modules.contains_key(&d.to) {
            adj.entry(d.from.clone()).or_default().insert(d.to.clone());
            adj.entry(d.to.clone()).or_default().insert(d.from.clone());
        }
    }
    let (mut seen, mut out) = (BTreeSet::new(), BTreeMap::<String, Vec<String>>::new());
    for id in modules.keys() {
        if seen.contains(id) {
            continue;
        }
        let (mut comp, mut stack) = (Vec::new(), vec![id.clone()]);
        seen.insert(id.clone());
        while let Some(c) = stack.pop() {
            comp.push(c.clone());
            if let Some(nbs) = adj.get(&c) {
                for n in nbs {
                    if !seen.contains(n) {
                        seen.insert(n.clone());
                        stack.push(n.clone());
                    }
                }
            }
        }
        let g = if comp.len() == 1 {
            String::new()
        } else {
            common_prefix(&comp)
        };
        out.entry(g).or_default().extend(comp);
    }
    out
}

fn common_prefix(ids: &[String]) -> String {
    let base: Vec<&str> = ids[0].split('/').collect();
    let mut len = base.len();
    for id in &ids[1..] {
        let parts: Vec<&str> = id.split('/').collect();
        let mut common = 0;
        while common < len && common < parts.len() && base[common] == parts[common] {
            common += 1;
        }
        len = common;
        if len == 0 {
            break;
        }
    }
    if len == 0 {
        ".".into()
    } else {
        base[..len].join("/")
    }
}

fn layers(
    ids: &[String],
    dependencies: &[Dependency],
) -> (BTreeMap<String, usize>, BTreeMap<String, BTreeSet<String>>) {
    let set: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut out = Adj::new();
    for d in dependencies {
        if set.contains(d.from.as_str()) && set.contains(d.to.as_str()) {
            out.entry(d.from.clone()).or_default().insert(d.to.clone());
        }
    }
    let (mut seen, mut on, mut post, mut kept) = (
        BTreeSet::new(),
        BTreeSet::new(),
        Vec::new(),
        BTreeMap::new(),
    );
    for id in ids {
        if !seen.contains(id) {
            dfs(id, &out, &mut seen, &mut on, &mut post, &mut kept);
        }
    }
    let mut layer: BTreeMap<String, usize> = BTreeMap::new();
    for id in post.iter().rev() {
        let mut max: Option<usize> = None;
        if let Some(preds) = kept.get(id) {
            for p in preds {
                if let Some(l) = layer.get(p) {
                    max = Some(max.map_or(*l, |m| m.max(*l)));
                }
            }
        }
        layer.insert(id.clone(), max.map_or(0, |m| m + 1));
    }
    (layer, kept)
}

fn dfs(
    id: &str,
    out: &Adj,
    seen: &mut BTreeSet<String>,
    on: &mut BTreeSet<String>,
    post: &mut Vec<String>,
    kept: &mut BTreeMap<String, BTreeSet<String>>,
) {
    seen.insert(id.to_string());
    on.insert(id.to_string());
    if let Some(ts) = out.get(id) {
        for t in ts {
            if on.contains(t) {
                continue;
            }
            if !seen.contains(t) {
                dfs(t, out, seen, on, post, kept);
            }
            kept.entry(t.clone()).or_default().insert(id.to_string());
        }
    }
    on.remove(id);
    post.push(id.to_string());
}

fn rows(
    ids: &[String],
    layers: &BTreeMap<String, usize>,
    kept: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeMap<String, usize> {
    let mut by_layer: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for id in ids {
        if let Some(l) = layers.get(id) {
            by_layer.entry(*l).or_default().push(id.clone());
        }
    }
    for list in by_layer.values_mut() {
        list.sort();
    }
    let mut row = BTreeMap::new();
    for (l, list) in &by_layer {
        if *l == 0 {
            for (i, id) in list.iter().enumerate() {
                row.insert(id.clone(), i);
            }
        } else {
            let mut scored: Vec<(String, f64)> = Vec::new();
            for id in list {
                let (sum, count) = kept
                    .get(id)
                    .map(|preds| {
                        let (mut s, mut c) = (0.0_f64, 0);
                        for p in preds {
                            if let Some(r) = row.get(p) {
                                s += *r as f64;
                                c += 1;
                            }
                        }
                        (s, c)
                    })
                    .unwrap_or((0.0, 0));
                scored.push((
                    id.clone(),
                    if count == 0 { 0.0 } else { sum / count as f64 },
                ));
            }
            scored.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            for (i, (id, _)) in scored.into_iter().enumerate() {
                row.insert(id, i);
            }
        }
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn module(id: &str) -> Module {
        Module {
            id: id.into(),
            group: String::new(),
            files: 0,
            tests: 0,
            entry: false,
            ..Default::default()
        }
    }

    fn dep(from: &str, to: &str) -> Dependency {
        Dependency {
            from: from.into(),
            to: to.into(),
            imports: 1,
            ..Default::default()
        }
    }

    /// Each module as `id|group|layer|row`, in output order.
    fn place(ids: &[&str], deps: &[(&str, &str)]) -> Vec<String> {
        let modules = ids
            .iter()
            .map(|id| (id.to_string(), module(id)))
            .collect::<BTreeMap<_, _>>();
        let dependencies = deps.iter().map(|(f, t)| dep(f, t)).collect::<Vec<_>>();
        layout(modules, &dependencies)
            .into_iter()
            .map(|m| format!("{}|{}|{}|{}", m.id, m.group, m.layer, m.row))
            .collect()
    }

    #[test]
    fn chain() {
        assert_eq!(
            place(&["a/x", "a/y", "a/z"], &[("a/x", "a/y"), ("a/y", "a/z")]),
            vec!["a/x|a|0|0", "a/y|a|1|0", "a/z|a|2|0"]
        );
    }

    #[test]
    fn longest_path() {
        assert_eq!(
            place(
                &["a/p", "a/q", "a/r"],
                &[("a/p", "a/q"), ("a/q", "a/r"), ("a/p", "a/r")]
            ),
            vec!["a/p|a|0|0", "a/q|a|1|0", "a/r|a|2|0"]
        );
    }

    #[test]
    fn cycle() {
        assert_eq!(
            place(&["a/x", "a/y"], &[("a/x", "a/y"), ("a/y", "a/x")]),
            vec!["a/x|a|0|0", "a/y|a|1|0"]
        );
    }

    #[test]
    fn barycenter() {
        assert_eq!(
            place(
                &["a/a", "a/b", "a/y", "a/z"],
                &[("a/a", "a/z"), ("a/b", "a/y")]
            ),
            vec!["a/a|a|0|0", "a/b|a|0|1", "a/z|a|1|0", "a/y|a|1|1",]
        );
    }

    #[test]
    fn groups() {
        let ids = [
            "web/src",
            "web/src/lib",
            "crates/f/src",
            "crates/f/src/d",
            "crates/g/src",
            "tools",
        ];
        let deps = [
            ("web/src", "web/src/lib"),
            ("crates/f/src", "crates/f/src/d"),
            ("crates/f/src", "crates/g/src"),
        ];
        assert_eq!(
            place(&ids, &deps),
            vec![
                "crates/f/src|crates|0|0",
                "crates/f/src/d|crates|1|0",
                "crates/g/src|crates|1|1",
                "web/src|web/src|0|0",
                "web/src/lib|web/src|1|0",
                "tools||0|0",
            ]
        );
    }
}
