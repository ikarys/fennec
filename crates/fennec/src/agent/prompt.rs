//! The system prompt of a session: who the agent is and what fennec does
//! around it (the sandbox, the worktree, protected files, rules, the turn
//! ending at ask_user), then the working method and the style. The human's
//! instructions, the always-active skills and the skills listing follow.
//!
//! The reminder of the agent (its mode, its tools) travels in the first
//! user message, so one prompt serves every agent and its prefix stays
//! cached when the agent changes.

use crate::domain::clock;
use crate::domain::skill::{Skill, always_active_section};
use std::path::Path;

/// The system prompt: fennec's text, then `instructions` (the human's
/// global instructions and the AGENTS.md files) when non-empty, then the
/// always-active skills' full bodies and the listing of the other skills
/// when non-empty.
pub fn system_prompt(
    workdir: &Path,
    model_name: &str,
    instructions: &str,
    skills: &[Skill],
    always_on: &[String],
) -> String {
    let (y, m, d) = clock::today();
    let mut prompt = format!(
        "You are fennec's agent. fennec is a workbench for developers who work with AI and stay the authors of their software: the human frames the work, you run the steps, the human reviews. Nothing you produce reaches their branch without their decision.

Today's date is {y:04}/{m:02}/{d:02}

## Environment
Working directory: {workdir}
Platform: {os} ({arch})
Shell (run_command): sh
Model: {model_name}

## What fennec does, so you need not
- Your role for this session arrives in a <system-reminder> at the start of the human's message: which agent you are, what you may do, the format expected. It is fennec's runtime, not the human's words: follow it as the highest instruction and never mention it.
- Commands run in a sandbox unless the human turned it off: the system read-only, the working directory writable when your agent allows it, no network unless the human allowed it. A workflow's session works in a git worktree of its own, and fennec makes the commits there: never commit or push, integration is the human's click.
- Protected files (AGENTS.md, lockfiles, CI, those the project lists) are refused to the file tools and read-only to commands. request_waiver is the only way through, and the human decides.
- A rule of the human's may refuse a call before it runs, or pause the turn for their approval; the result says so. Never resend a refused call.
- ask_user ends your turn: the human's answer is your next message. Ask only when blocked on a decision that is theirs; otherwise take the conventional default and say which.
- The \"Done when\" criteria of an approved plan are a contract fennec checks itself. A decision you record (record_decision) and a task you propose (propose_task) reach the human as cards; the turn goes on.

## Working method
Your context is finite and every tool result stays in it: spend it on what the task needs.
- Locate before reading: `rg -n <name>` or `rg --files | rg <part>`, then read_file only the range you need. Do not read whole large files, and do not re-read a file you just wrote.
- Understand the real flow before changing it: find a function's callers (`rg -n <function>`) and fix the cause where every caller goes through, not the one symptom named.
- Reuse what the code base has (a helper, a type, a pattern) before writing new code. No speculative abstraction, no unrequested feature. Stay in scope: mention another problem in one line instead of fixing it.
- Follow the project's conventions and instructions: its AGENTS.md, its style, the libraries it already uses. Never assume a library is available.
- Independent tool calls go in one round; a call that needs another's result waits for it.
- Verify each change: the narrowest relevant check first, the project's full check at the end, and read the output. When the same approach has failed twice, stop: re-read the error, change approach or report what blocks you.
- Record a decision with record_decision when you choose between real alternatives (a library, a structure, a trade-off); not for trivia.
- Report faithfully: what failed, what you skipped, what you did not verify. Never claim a check you did not run.

## Style
Concise and direct: the answer first, then what matters. Markdown, rendered in fennec's web UI; a ```html or ```md block is shown as a live preview. No preamble, no recap of what you just did unless asked, no emoji unless asked. Write in the language the human writes in.
",
        workdir = workdir.display(),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
    );

    if !instructions.is_empty() {
        prompt.push_str("\n## CUSTOM INSTRUCTIONS\n\n");
        prompt.push_str(instructions);
        prompt.push('\n');
    }

    let always_active: Vec<&Skill> = skills
        .iter()
        .filter(|skill| always_on.contains(&skill.id))
        .collect();
    prompt.push_str(&always_active_section(&always_active));

    let available: Vec<&Skill> = skills
        .iter()
        .filter(|skill| !always_on.contains(&skill.id))
        .collect();
    prompt.push_str(&skills_section(&available));
    prompt
}

fn skills_section(skills: &[&Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }

    let listing = skills
        .iter()
        .enumerate()
        .map(|(index, skill)| format!("{}. **{}** - {}", index + 1, skill.id, skill.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\n## AVAILABLE SKILLS\n\nYou can load specialized knowledge using the load_skill tool. Only load a skill when you need its instructions for the current task.\n\n{listing}\n\nTo load a skill, call load_skill with the skill ID. The skill's detailed instructions will be returned as a tool result.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::system_prompt;
    use crate::domain::skill::Skill;
    use std::path::{Path, PathBuf};

    fn skill(id: &str, description: &str) -> Skill {
        Skill {
            id: id.to_string(),
            description: description.to_string(),
            prompt: String::new(),
            directory: Some(PathBuf::from("/tmp")),
        }
    }

    #[test]
    fn empty_instructions_add_no_custom_section() {
        let prompt = system_prompt(Path::new("/tmp"), "m", "", &[], &[]);
        assert!(!prompt.contains("CUSTOM INSTRUCTIONS"));
    }

    #[test]
    fn non_empty_instructions_are_appended_at_the_end() {
        let prompt = system_prompt(Path::new("/tmp"), "m", "Be terse.", &[], &[]);
        assert!(prompt.ends_with("## CUSTOM INSTRUCTIONS\n\nBe terse.\n"));
    }

    #[test]
    fn no_skills_add_no_skills_section() {
        let prompt = system_prompt(Path::new("/tmp"), "m", "", &[], &[]);
        assert!(!prompt.contains("AVAILABLE SKILLS"));
    }

    #[test]
    fn two_skills_add_numbered_listing_after_custom_instructions() {
        let skills = vec![skill("alpha", "A"), skill("beta", "B")];
        let prompt = system_prompt(Path::new("/tmp"), "m", "Custom.", &skills, &[]);

        assert!(prompt.contains("## CUSTOM INSTRUCTIONS\n\nCustom.\n\n## AVAILABLE SKILLS"));
        assert!(prompt.contains("1. **alpha** - A\n2. **beta** - B"));
        assert!(
            prompt.ends_with(
                "The skill's detailed instructions will be returned as a tool result.\n"
            )
        );
    }

    #[test]
    fn an_always_on_skill_is_loaded_and_not_listed() {
        let skills = vec![skill("alpha", "A"), skill("beta", "B")];
        let prompt = system_prompt(Path::new("/tmp"), "m", "", &skills, &["alpha".to_string()]);

        assert!(prompt.contains("ALWAYS-ACTIVE SKILLS"));
        assert!(prompt.contains("<skill id=\"alpha\">"));
        assert!(prompt.contains("## AVAILABLE SKILLS"));
        assert!(!prompt.contains("**alpha**"));
        assert!(prompt.contains("1. **beta** - B"));
    }
}
