//! What the project map says about a module, read from the code: the leading
//! doc comment of its files (its authors' words) and the modules it imports or
//! that import it.

use std::collections::BTreeMap;

use super::{Dependency, Link, Module, SourceNote, split_path};

pub fn describe(
    modules: &mut [Module],
    sources: &BTreeMap<&str, &str>,
    owners: &BTreeMap<&str, String>,
    dependencies: &[Dependency],
) {
    let mut files: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (path, owner) in owners {
        files.entry(owner.as_str()).or_default().push(path);
    }

    let mut uses: BTreeMap<&str, Vec<&Dependency>> = BTreeMap::new();
    let mut used_by: BTreeMap<&str, Vec<&Dependency>> = BTreeMap::new();
    for dep in dependencies {
        uses.entry(&dep.from).or_default().push(dep);
        used_by.entry(&dep.to).or_default().push(dep);
    }

    for module in modules.iter_mut() {
        let files = files.get(module.id.as_str());
        module.sources = files
            .map(|paths| {
                paths
                    .iter()
                    .map(|&path| SourceNote {
                        path: path.into(),
                        summary: sources.get(path).copied().and_then(summary),
                    })
                    .collect()
            })
            .unwrap_or_default();

        module.summary = files.and_then(|paths| {
            head_path(&module.id, paths)
                .and_then(|path| sources.get(path).copied().and_then(summary))
        });

        module.uses = links(uses.get(module.id.as_str()), |dep| &dep.to);
        module.used_by = links(used_by.get(module.id.as_str()), |dep| &dep.from);
    }
}

fn links<'a>(
    deps: Option<&'a Vec<&'a Dependency>>,
    target: impl Fn(&'a Dependency) -> &'a str,
) -> Vec<Link> {
    let mut links = Vec::new();
    if let Some(deps) = deps {
        for dep in deps {
            links.push(Link {
                module: target(dep).into(),
                imports: dep.imports,
            });
        }
        links.sort_by(|a, b| {
            b.imports
                .cmp(&a.imports)
                .then_with(|| a.module.cmp(&b.module))
        });
    }
    links
}

fn head_path<'a>(id: &str, files: &[&'a str]) -> Option<&'a str> {
    let rs = format!("{id}.rs");
    files.iter().copied().find(|path| {
        *path == rs || {
            let (dir, name) = split_path(path);
            (dir == id || (id == "." && dir.is_empty()))
                && matches!(
                    name,
                    "mod.rs" | "lib.rs" | "main.rs" | "index.ts" | "index.tsx" | "index.js"
                )
        }
    })
}

fn summary(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|line| !line.trim().is_empty())?;
    let first = lines[start].trim_start();

    if first.starts_with("//!") {
        return line_doc(&lines[start..], "//!");
    }
    if first.starts_with("/**") || first.starts_with("/*") {
        return block_doc(&lines[start..]);
    }
    if first.starts_with("//") {
        return line_doc(&lines[start..], "//");
    }
    None
}

fn line_doc(lines: &[&str], prefix: &str) -> Option<String> {
    let mut text = Vec::new();
    for line in lines {
        let line = line.trim_start();
        let Some(body) = line.strip_prefix(prefix) else {
            break;
        };
        let line = clean(body);
        if line.is_empty() {
            break;
        }
        text.push(line);
    }
    join(&text)
}

fn block_doc(lines: &[&str]) -> Option<String> {
    let mut text = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim_start();
        let (body, done) = if i == 0 {
            let body = line
                .strip_prefix("/**")
                .or_else(|| line.strip_prefix("/*"))
                .unwrap_or(line);
            match body.find("*/") {
                Some(end) => (&body[..end], true),
                None => (body, false),
            }
        } else {
            match line.find("*/") {
                Some(end) => (&line[..end], true),
                None => (line, false),
            }
        };

        let line = clean_block(body);
        let empty = line.is_empty();
        if !empty {
            text.push(line);
        }
        if done || (i > 0 && empty) {
            return join(&text);
        }
    }
    join(&text)
}

fn clean(body: &str) -> String {
    body.strip_prefix(' ').unwrap_or(body).trim().into()
}

fn clean_block(line: &str) -> String {
    clean(line.trim().strip_prefix('*').unwrap_or(line))
}

fn join(lines: &[String]) -> Option<String> {
    let text = lines.join(" ").trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn rust_inner_doc() {
        assert_eq!(
            summary(
                "//! The project map.\n//! Read from imports.\n//!\n//! Second paragraph.\nuse x;"
            ),
            Some("The project map. Read from imports.".into())
        );
    }

    #[test]
    fn ts_block_doc() {
        assert_eq!(
            summary("/**\n * Typed fetch\n * for the API.\n */\nimport x from 'y';"),
            Some("Typed fetch for the API.".into())
        );
    }

    #[test]
    fn ts_line_doc() {
        assert_eq!(
            summary("\n// The theme tokens.\nexport const a = 1;"),
            Some("The theme tokens.".into())
        );
    }

    #[test]
    fn no_doc() {
        assert_eq!(summary("import x from 'y';\n// late comment"), None);
        assert_eq!(summary(""), None);
    }

    #[test]
    fn describe_fills_module() {
        let mut modules = vec![
            Module {
                id: "a".into(),
                ..Default::default()
            },
            Module {
                id: "a/b".into(),
                ..Default::default()
            },
        ];
        let sources: BTreeMap<&str, &str> = vec![
            ("a.rs", "//! Module a.\nmod b;"),
            ("a/x.rs", "fn f() {}"),
            ("a/b/mod.rs", "//! Module b."),
        ]
        .into_iter()
        .collect();
        let owners: BTreeMap<&str, String> = vec![
            ("a.rs", "a".into()),
            ("a/x.rs", "a".into()),
            ("a/b/mod.rs", "a/b".into()),
        ]
        .into_iter()
        .collect();
        let dependencies = vec![Dependency {
            from: "a".into(),
            to: "a/b".into(),
            imports: 2,
            ..Default::default()
        }];

        describe(&mut modules, &sources, &owners, &dependencies);

        let a = &modules[0];
        assert_eq!(a.summary.as_deref(), Some("Module a."));
        assert_eq!(
            a.sources
                .iter()
                .map(|s| s.path.as_str())
                .collect::<Vec<_>>(),
            ["a.rs", "a/x.rs"]
        );
        assert_eq!(
            a.uses,
            vec![Link {
                module: "a/b".into(),
                imports: 2
            }]
        );
        assert!(a.used_by.is_empty());

        let b = &modules[1];
        assert_eq!(b.summary.as_deref(), Some("Module b."));
        assert_eq!(
            b.used_by,
            vec![Link {
                module: "a".into(),
                imports: 2
            }]
        );
    }
}
