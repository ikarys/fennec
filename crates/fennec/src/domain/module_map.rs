//! The project map: a project's modules and the dependencies between them,
//! read from the import lines of its source files (Rust and TypeScript or
//! JavaScript). Deterministic: no model involved.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::iter::once;

use super::test_files::is_test_path;

mod layout;
mod notes;
mod routes;
use layout::layout;

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub id: String,
    /// The modules linked by dependencies it is drawn with; `""` when it has none.
    pub group: String,
    pub files: usize,
    pub tests: usize,
    pub entry: bool,
    pub layer: usize,
    pub row: usize,
    /// What its authors say it is: the leading doc comment of its head file
    /// (`a/b.rs`, `mod.rs`, `lib.rs`, `main.rs`, `index.ts`…), first paragraph.
    pub summary: Option<String>,
    pub sources: Vec<SourceNote>,
    /// The modules it imports, and those importing it, the most imports first.
    pub uses: Vec<Link>,
    pub used_by: Vec<Link>,
    /// Top-left corner of its box, in the group's drawing.
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub from: String,
    pub to: String,
    pub imports: usize,
    /// The arrow as an SVG path, ready to draw.
    pub path: String,
    /// Where the import count is shown.
    pub label: Point,
}

/// A source file and the first paragraph of its leading doc comment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceNote {
    pub path: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    pub module: String,
    pub imports: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleMap {
    pub modules: Vec<Module>,
    pub dependencies: Vec<Dependency>,
    /// The groups in the modules' order (the biggest first, `""` last).
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub modules: usize,
}

/// Source files by path, with their content.
type Sources<'a> = BTreeMap<&'a str, &'a str>;

const TS_EXTENSIONS: [&str; 5] = ["ts", "tsx", "js", "jsx", "mjs"];

pub fn build(files: &[SourceFile]) -> ModuleMap {
    let sources: Sources = files
        .iter()
        .filter(|file| is_source_path(&file.path))
        .map(|file| (file.path.as_str(), file.content.as_str()))
        .collect();
    // Every directory holding a source file, directly or deeper.
    let dirs: BTreeSet<&str> = sources
        .keys()
        .flat_map(|path| path.match_indices('/').map(|(at, _)| &path[..at]))
        .collect();
    let owners: BTreeMap<&str, String> = sources
        .keys()
        .map(|path| (*path, module_of(path, &dirs)))
        .collect();
    let mut dependencies = dependencies(&sources, &owners);
    let mut modules = layout(module_stats(&sources, &owners), &dependencies);
    notes::describe(&mut modules, &sources, &owners, &dependencies);
    routes::route(&mut modules, &mut dependencies);
    let groups = modules
        .chunk_by(|a, b| a.group == b.group)
        .map(|chunk| Group {
            name: chunk[0].group.clone(),
            modules: chunk.len(),
        })
        .collect();
    ModuleMap {
        modules,
        dependencies,
        groups,
    }
}

pub fn is_source_path(path: &str) -> bool {
    path.ends_with(".rs") || is_ts_path(path)
}

fn is_ts_path(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| TS_EXTENSIONS.contains(&ext))
}

fn split_path(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

fn module_of(path: &str, dirs: &BTreeSet<&str>) -> String {
    let (dir, name) = split_path(path);
    // Rust's `a/b.rs` and `a/b/` are one module when the directory exists.
    let own_dir = name
        .strip_suffix(".rs")
        .filter(|stem| !stem.is_empty())
        .map(|stem| format!("{dir}/{stem}").trim_start_matches('/').to_string())
        .filter(|candidate| dirs.contains(candidate.as_str()));
    own_dir.unwrap_or_else(|| {
        if dir.is_empty() {
            ".".into()
        } else {
            dir.into()
        }
    })
}

fn module_stats(sources: &Sources, owners: &BTreeMap<&str, String>) -> BTreeMap<String, Module> {
    let mut stats: BTreeMap<String, Module> = BTreeMap::new();
    for (&path, &content) in sources {
        let id = &owners[path];
        let module = stats.entry(id.clone()).or_insert_with(|| Module {
            id: id.clone(),
            group: String::new(),
            files: 0,
            tests: 0,
            entry: false,
            ..Default::default()
        });
        module.files += 1;
        module.tests += (is_test_path(path)
            || (path.ends_with(".rs")
                && (content.contains("#[cfg(test)]") || content.contains("#[test]"))))
            as usize;
        module.entry |= matches!(
            split_path(path).1,
            "main.rs" | "lib.rs" | "main.ts" | "main.tsx" | "main.js"
        );
    }
    stats
}

/// Modules weighted by the distinct files that import them.
fn dependencies(sources: &Sources, owners: &BTreeMap<&str, String>) -> Vec<Dependency> {
    let roots = crate_roots(sources);
    let mut importers: BTreeMap<(&str, &str), BTreeSet<&str>> = BTreeMap::new();
    for (&path, &content) in sources {
        let from = owners[path].as_str();
        for target in imported_files(path, content, sources, &roots) {
            let to = owners.get(target.as_str()).map(String::as_str);
            if let Some(to) = to.filter(|to| *to != from) {
                importers.entry((from, to)).or_default().insert(path);
            }
        }
    }
    importers
        .into_iter()
        .map(|((from, to), files)| Dependency {
            from: from.into(),
            to: to.into(),
            imports: files.len(),
            ..Default::default()
        })
        .collect()
}

/// The source files a file imports, in either language.
fn imported_files(
    path: &str,
    content: &str,
    sources: &Sources,
    crate_roots: &BTreeMap<String, String>,
) -> Vec<String> {
    if is_ts_path(path) {
        let dir = split_path(path).0;
        return ts_imports(content)
            .iter()
            .filter_map(|spec| resolve_ts(dir, spec, sources))
            .collect();
    }
    // `crate::` is this file's crate; a workspace crate's name is its root.
    let markers = once(("crate", None)).chain(crate_roots.keys().map(|n| (n.as_str(), Some(n))));
    let mut files = Vec::new();
    for (name, workspace_crate) in markers {
        for segments in rust_references(content, &format!("{name}::")) {
            let file = match workspace_crate {
                None => crate_root(path).and_then(|root| resolve_rust(&root, &segments, sources)),
                // An item re-exported at the crate root lives in its lib.rs.
                Some(name) => crate_roots.get(name).map(|root| {
                    resolve_rust(root, &segments, sources)
                        .unwrap_or_else(|| format!("{root}/lib.rs"))
                }),
            };
            files.extend(file);
        }
    }
    files
}

/// `X/src/lib.rs` makes a crate named after the last segment of X.
fn crate_roots(sources: &Sources) -> BTreeMap<String, String> {
    sources
        .keys()
        .filter_map(|path| {
            let dir = path
                .strip_suffix("/src/lib.rs")
                .filter(|dir| !dir.is_empty())?;
            let name = split_path(dir).1.replace('-', "_");
            (!name.is_empty()).then(|| (name, format!("{dir}/src")))
        })
        .collect()
}

/// The directory up to and including the last `src` segment of the path.
fn crate_root(path: &str) -> Option<String> {
    let segments: Vec<&str> = split_path(path).0.split('/').collect();
    let last_src = segments.iter().rposition(|segment| *segment == "src")?;
    Some(segments[..=last_src].join("/"))
}

fn resolve_rust(root: &str, segments: &[String], sources: &Sources) -> Option<String> {
    (1..=segments.len()).rev().find_map(|k| {
        let prefix = format!("{root}/{}", segments[..k].join("/"));
        [format!("{prefix}.rs"), format!("{prefix}/mod.rs")]
            .into_iter()
            .find(|file| sources.contains_key(file.as_str()))
    })
}

fn ts_imports(content: &str) -> Vec<String> {
    ["from ", "import ", "import("]
        .iter()
        .flat_map(|marker| content.match_indices(marker))
        .filter_map(|(at, marker)| quoted_specifier(&content[at + marker.len()..]))
        .filter(|spec| spec.starts_with("./") || spec.starts_with("../"))
        .collect()
}

fn quoted_specifier(text: &str) -> Option<String> {
    let mut chars = text.trim_start().chars();
    let quote = chars.next().filter(|c| matches!(c, '\'' | '"'))?;
    let mut value = String::new();
    loop {
        match chars.next()? {
            '\\' => value.push(chars.next()?),
            c if c == quote => return Some(value),
            c => value.push(c),
        }
    }
}

fn resolve_ts(importer_dir: &str, spec: &str, sources: &Sources) -> Option<String> {
    let mut parts: Vec<&str> = importer_dir.split('/').filter(|p| !p.is_empty()).collect();
    for segment in spec.split('/') {
        match segment {
            "" | "." => {}
            ".." => drop(parts.pop()?),
            other => parts.push(other),
        }
    }
    let base = parts.join("/");
    let candidates = once(base.clone())
        .chain(TS_EXTENSIONS.iter().map(|ext| format!("{base}.{ext}")))
        .chain(TS_EXTENSIONS[..3].iter().map(|ext| {
            format!("{base}/index.{ext}")
                .trim_start_matches('/')
                .to_string()
        }));
    candidates
        .into_iter()
        .find(|file| sources.contains_key(file.as_str()))
}

/// The paths written after each `marker` in the production code only: the
/// test module (by convention at the end of the file) holds fixtures whose
/// strings would read as imports.
fn rust_references(content: &str, marker: &str) -> Vec<Vec<String>> {
    let production = content.split("#[cfg(test)]").next().unwrap_or_default();
    let text: String = production
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n");
    let mut references = Vec::new();
    for (at, _) in text.match_indices(marker) {
        if !text.as_bytes()[..at].last().is_some_and(is_ident) {
            references.extend(path_segments(&text[at + marker.len()..]));
        }
    }
    references
}

/// `a::b` gives one path; `{a::b, c::{d, e}, f as g}` one per top-level item,
/// each cut at its own group (`c`): a module is all the map needs.
fn path_segments(after: &str) -> Vec<Vec<String>> {
    let text = after.trim_start();
    let Some(group) = text.strip_prefix('{') else {
        return vec![leading_path(text)];
    };
    let (mut depth, mut start, mut items) = (0usize, 0, Vec::new());
    for (at, ch) in group.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' if depth == 0 => {
                items.push(&group[start..at]);
                break;
            }
            '}' => depth -= 1,
            ',' if depth == 0 => {
                items.push(&group[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    items
        .into_iter()
        .map(|item| leading_path(item.trim_start()))
        .filter(|path| !path.is_empty())
        .collect()
}

fn is_ident(byte: &u8) -> bool {
    byte.is_ascii_alphanumeric() || *byte == b'_'
}

/// The identifiers joined by `::` at the start of the text.
fn leading_path(text: &str) -> Vec<String> {
    let mut path = Vec::new();
    for piece in text.split("::") {
        let len = piece.bytes().take_while(is_ident).count();
        path.extend((len > 0).then(|| piece[..len].to_string()));
        if len == 0 || len < piece.len() {
            break;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_from(files: &[(&str, &str)]) -> ModuleMap {
        let file = |&(path, content): &(&str, &str)| SourceFile {
            path: path.into(),
            content: content.into(),
        };
        build(&files.iter().map(file).collect::<Vec<_>>())
    }

    /// (id, files, tests, entry, layer, row) of each module.
    fn modules(map: &ModuleMap) -> Vec<(&str, usize, usize, bool, usize, usize)> {
        let modules = map.modules.iter();
        modules
            .map(|m| (m.id.as_str(), m.files, m.tests, m.entry, m.layer, m.row))
            .collect()
    }

    fn edges(map: &ModuleMap) -> Vec<(&str, &str, usize)> {
        let deps = map.dependencies.iter();
        deps.map(|d| (d.from.as_str(), d.to.as_str(), d.imports))
            .collect()
    }

    #[test]
    fn rust_groups_count_every_item() {
        let map = build_from(&[
            ("src/main.rs", "use crate::{web::{a, b}, domain as d};\n"),
            ("src/domain.rs", ""),
            ("src/web.rs", ""),
            ("src/web/a.rs", ""),
            ("src/domain/x.rs", ""),
        ]);
        assert_eq!(
            edges(&map),
            [("src", "src/domain", 1), ("src", "src/web", 1)]
        );
    }

    #[test]
    fn rust_crate_shape() {
        let map = build_from(&[
            (
                "src/main.rs",
                "mod domain;\nuse crate::domain::adr::parse;\n",
            ),
            ("src/domain.rs", ""),
            ("src/domain/adr.rs", "#[cfg(test)]\nmod tests {}\n"),
            (
                "src/web/routes.rs",
                "use crate::{domain::adr, web};\n// crate::adr::load()\n",
            ),
        ]);
        assert_eq!(
            modules(&map),
            [
                ("src", 1, 0, true, 0, 0),
                ("src/web", 1, 0, false, 0, 1),
                ("src/domain", 2, 1, false, 1, 0),
            ]
        );
        assert_eq!(
            edges(&map),
            [("src", "src/domain", 1), ("src/web", "src/domain", 1)]
        );
    }

    #[test]
    fn ts_shape() {
        let map = build_from(&[
            (
                "web/src/App.tsx",
                "import Home from './pages/Home';\nimport 'react';\n",
            ),
            ("web/src/pages/Home.tsx", "import '../lib/api';\n"),
            ("web/src/lib/api.ts", ""),
        ]);
        assert_eq!(
            edges(&map),
            [
                ("web/src", "web/src/pages", 1),
                ("web/src/pages", "web/src/lib", 1)
            ]
        );
        let layers: Vec<_> = modules(&map).iter().map(|m| (m.0, m.4)).collect();
        assert_eq!(
            layers,
            [("web/src", 0), ("web/src/pages", 1), ("web/src/lib", 2)]
        );
    }

    #[test]
    fn workspace_crate_dependency() {
        let map = build_from(&[
            ("crates/llm-providers/src/lib.rs", "pub mod openai;\n"),
            ("crates/llm-providers/src/openai.rs", ""),
            (
                "crates/fennec/src/main.rs",
                "use llm_providers::openai::Client;\n",
            ),
        ]);
        assert_eq!(
            edges(&map),
            [("crates/fennec/src", "crates/llm-providers/src", 1)]
        );
    }

    #[test]
    fn cycle_uses_smallest_root() {
        let map = build_from(&[
            ("a/x.ts", "import '../b/y';\n"),
            ("b/y.ts", "import '../a/x';\n"),
        ]);
        let layers: Vec<_> = modules(&map).iter().map(|m| (m.0, m.4)).collect();
        assert_eq!(layers, [("a", 0), ("b", 1)]);
    }

    #[test]
    fn escaping_root_is_ignored() {
        let map = build_from(&[("a.ts", "import '../../outside';\n")]);
        assert!(map.dependencies.is_empty());
    }
}
