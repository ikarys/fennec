//! Port of openfox `skills/registry.ts` discovery (`loadAllSkills`,
//! `loadPortableSkills`): embedded bundled defaults, then `~/.agents/skills`,
//! then the project's `.agents/skills`, which overrides a skill of the same id.
//!
//! Bundled defaults are embedded in the binary, never copied to the user's
//! config, as in openfox.
//!
//! Not ported: openfox's own config/`.openfox/skills` dirs, legacy
//! `.skill.md` files, selected directories, enable/disable settings, name
//! warnings, installer, HTTP routes/UI, edit/delete.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::skill::{Skill, parse_skill_md};

const DEFAULTS: [&str; 1] = [include_str!("../skills/browser/SKILL.md")];

/// openfox `loadAllSkills`: the skills a session in `workdir` can load.
pub fn discover(workdir: &Path) -> Vec<Skill> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    discover_in(home.as_deref(), workdir)
}

fn discover_in(home: Option<&Path>, workdir: &Path) -> Vec<Skill> {
    let mut skills = Vec::new();

    for content in DEFAULTS {
        let (id, description, prompt) =
            parse_skill_md(content).expect("embedded default skill must parse");
        insert_or_override(
            &mut skills,
            Skill {
                id,
                description,
                prompt,
                directory: None,
            },
        );
    }

    if let Some(home) = home {
        let dir = home.join(".agents").join("skills");
        for skill in load_skills_directory(&dir) {
            insert_or_override(&mut skills, skill);
        }
    }

    let dir = workdir.join(".agents").join("skills");
    for skill in load_skills_directory(&dir) {
        insert_or_override(&mut skills, skill);
    }

    skills
}

/// openfox `loadPortableSkills`; missing dir → empty.
fn load_skills_directory(dir: &Path) -> Vec<Skill> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut entries: Vec<_> = entries.filter_map(|entry| entry.ok()).collect();
    entries.sort_by_key(|entry| entry.file_name());

    let mut skills = Vec::new();
    for entry in entries {
        let package = entry.path();
        if !package.is_dir() {
            continue;
        }

        let skill_path = package.join("SKILL.md");
        let content = match fs::read_to_string(&skill_path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let (id, description, prompt) = match parse_skill_md(&content) {
            Some(value) => value,
            None => continue,
        };

        let directory = match package.canonicalize() {
            Ok(value) => value,
            Err(_) => continue,
        };

        skills.push(Skill {
            id,
            description,
            prompt,
            directory: Some(directory),
        });
    }

    skills
}

fn insert_or_override(skills: &mut Vec<Skill>, skill: Skill) {
    if let Some(slot) = skills.iter().position(|existing| existing.id == skill.id) {
        skills[slot] = skill;
    } else {
        skills.push(skill);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("fennec-skills-{}", new_id()));
        let home = root.join("home");
        let workdir = root.join("workdir");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        (root, home, workdir)
    }

    fn skills_dir(base: &Path) -> PathBuf {
        let dir = base.join(".agents").join("skills");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_skill(skills: &Path, package: &str, content: &str) {
        let package_dir = skills.join(package);
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(package_dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn missing_dirs_load_defaults_only() {
        let (root, home, workdir) = fixture();
        assert_eq!(discover_in(Some(&home), &workdir).len(), DEFAULTS.len());
        assert_eq!(discover_in(None, &workdir).len(), DEFAULTS.len());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn embedded_browser_skill_is_discovered_with_no_directory() {
        let (root, _home, workdir) = fixture();
        let found = discover_in(None, &workdir);
        assert_eq!(found.len(), DEFAULTS.len());
        assert_eq!(found[0].id, "browser");
        assert_eq!(found[0].directory, None);
        assert!(found[0].prompt.contains("@playwright/cli"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn package_loads_with_canonical_directory() {
        let (root, home, workdir) = fixture();
        let skills = skills_dir(&home);
        write_skill(
            &skills,
            "pkg",
            "---\nname: pkg\ndescription: d\n---\nbody\n",
        );

        let found = discover_in(Some(&home), &workdir);
        assert_eq!(found.len(), DEFAULTS.len() + 1);
        let pkg = found.iter().find(|skill| skill.id == "pkg").unwrap();
        assert_eq!(
            pkg.directory,
            Some(skills.join("pkg").canonicalize().unwrap())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_packages_and_plain_files_are_skipped() {
        let (root, home, workdir) = fixture();
        let skills = skills_dir(&home);
        std::fs::create_dir_all(skills.join("empty")).unwrap();
        write_skill(&skills, "invalid", "---\nname: invalid\n---\nbody\n");
        std::fs::write(skills.join("plain.txt"), "not a package").unwrap();

        let found = discover_in(Some(&home), &workdir);
        assert_eq!(found.len(), DEFAULTS.len());
        assert!(found.iter().all(|skill| skill.id == "browser"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_overrides_home_skill_and_keeps_slot_order() {
        let (root, home, workdir) = fixture();
        let home_skills = skills_dir(&home);
        write_skill(
            &home_skills,
            "alpha",
            "---\nname: alpha\ndescription: home\n---\nhome body\n",
        );
        write_skill(
            &home_skills,
            "beta",
            "---\nname: beta\ndescription: beta\n---\nbeta body\n",
        );

        let work_skills = skills_dir(&workdir);
        write_skill(
            &work_skills,
            "alpha",
            "---\nname: alpha\ndescription: project\n---\nproject body\n",
        );

        let found = discover_in(Some(&home), &workdir);
        assert_eq!(found.len(), DEFAULTS.len() + 2);
        assert_eq!(found[0].id, "browser");
        assert_eq!(found[1].id, "alpha");
        assert_eq!(found[1].description, "project");
        assert_eq!(found[2].id, "beta");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_skill_overrides_embedded_browser() {
        let (root, _home, workdir) = fixture();
        let skills = skills_dir(&workdir);
        write_skill(
            &skills,
            "browser",
            "---\nname: browser\ndescription: project\n---\nproject body\n",
        );

        let found = discover_in(None, &workdir);
        assert_eq!(found.len(), DEFAULTS.len());
        assert_eq!(found[0].id, "browser");
        assert_eq!(found[0].description, "project");
        assert_eq!(found[0].prompt, "project body");
        assert_eq!(
            found[0].directory,
            Some(skills.join("browser").canonicalize().unwrap())
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skills_are_sorted_by_directory_name() {
        let (root, _home, workdir) = fixture();
        let skills = skills_dir(&workdir);
        write_skill(&skills, "b", "---\nname: b\ndescription: b\n---\nb\n");
        write_skill(&skills, "a", "---\nname: a\ndescription: a\n---\na\n");

        let found = discover_in(None, &workdir);
        let ids: Vec<&str> = found.iter().map(|skill| skill.id.as_str()).collect();
        assert_eq!(ids, vec!["browser", "a", "b"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
