//! Port of openfox `tools/load-skill.ts`. Skills are read again at call
//! time, as in openfox: an edited skill loads fresh while the list in the
//! frozen system prompt stays as it was.
//!
//! Not ported: the enabled check (no enable/disable setting yet).

use super::args;
use crate::domain::event::ToolResult;
use crate::domain::skill::format_skill_prompt;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "load_skill",
            "description": "Load a skill's detailed instructions into context. Use this when you need domain-specific knowledge for a task. Call with the skill ID shown in the AVAILABLE SKILLS section.",
            "parameters": {
                "type": "object",
                "properties": {
                    "skillId": {
                        "type": "string",
                        "description": "ID of the skill to load (e.g. \"playwright-cli\")"
                    }
                },
                "required": ["skillId"]
            }
        }
    })
}

pub(super) fn run(args: &Value, workdir: &Path) -> ToolResult {
    let id = match args::string(args, "skillId") {
        Ok(value) => value,
        Err(error) => return ToolResult::failure(error),
    };

    let skills = crate::skills::discover(workdir);

    if let Some(skill) = skills.iter().find(|skill| skill.id == id) {
        return ToolResult::success(format_skill_prompt(skill), false);
    }

    let available = skills
        .iter()
        .map(|skill| skill.id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    ToolResult::failure(format!(
        "Skill \"{id}\" not found. Available skills: {available}"
    ))
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::domain::id::new_id;
    use serde_json::json;
    use std::path::PathBuf;

    fn workdir_with_skill() -> (PathBuf, String) {
        let id = format!("skill-{}", new_id());
        let dir = std::env::temp_dir().join(format!("fennec-skills-{}", new_id()));
        let skill_dir = dir.join(".agents").join("skills").join(&id);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {id}\ndescription: Test\n---\nRun scripts/check.sh.\n"),
        )
        .unwrap();
        (dir, id)
    }

    #[test]
    fn unknown_id_lists_available_ones() {
        let (dir, skill_id) = workdir_with_skill();
        let missing = format!("missing-{}", new_id());
        let result = run(&json!({ "skillId": missing }), &dir);

        assert!(!result.success);
        let error = result.error.expect("failure has an error");
        assert!(error.starts_with("Skill \"missing-"));
        assert!(error.contains("Available skills: "));
        assert!(error.contains(&skill_id));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_skill_is_found() {
        let (dir, skill_id) = workdir_with_skill();
        let result = run(&json!({ "skillId": skill_id }), &dir);

        assert!(result.success);
        let output = result.output.expect("success has output");
        assert!(output.contains("Skill package directory:"));
        assert!(output.contains("Run scripts/check.sh."));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
