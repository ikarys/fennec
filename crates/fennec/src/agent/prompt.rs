//! Port of openfox `chat/prompts.ts` (`buildBasePrompt`), with the
//! portable skills section (`buildSkillsSection`).
//!
//! Not ported: sub-agents and caveman thinking.
//!
//! fennec's own: the "Working method" section (context, search, verification
//! and reporting habits), kept apart so the openfox text above still maps 1:1.

use crate::domain::clock;
use crate::domain::skill::{Skill, always_active_section};
use std::path::Path;

/// Port of openfox `buildBasePrompt`, without the sections for features
/// not ported yet (sub-agents, caveman thinking).
///
/// `instructions` (openfox `instructionsSection`) is appended at the end
/// when non-empty, then the always-active skills' full bodies and the
/// remaining skills (`buildSkillsSection`) when non-empty.
pub fn system_prompt(
    workdir: &Path,
    model_name: &str,
    instructions: &str,
    skills: &[Skill],
    always_on: &[String],
) -> String {
    let (y, m, d) = clock::today();
    let mut prompt = format!(
        "You are fennec, an agentic assistant.

Today's date is {y:04}/{m:02}/{d:02}

## ENVIRONMENT
Working directory: {workdir}
Platform: {os} ({arch})
Shell (run_command): sh
Model: {model_name}

## CORE BEHAVIOR
Help user complete tasks safely and efficiently.
Do everything to satisfy user requirements.
Read file before editing.
Prefer precise, minimal changes.
Use available tools when needed.
Explain tradeoffs clearly when requirements are ambiguous.
Follow repository and project instructions exactly.
Commands run from your working directory — do not prepend \"cd /path/\" to change to a directory you are already in.

## MODE CONTROL
fennec appends runtime control as USER-role messages wrapped in <system-reminder>...</system-reminder>.
These reminders are authoritative, treat them as higher-priority operational constraints.
Never say \"user reminded me\"; they are runtime mode injected by fennec.

## WORKFLOW
Reminder = plan mode: focus on understanding, exploration, clarification, the plan's quality.
Reminder = build mode: focus on implementation, verification, completing the approved plan.

## TONE AND STYLE
Be concise, direct, and to the point.
Explain non-trivial bash commands (what & why).
Output renders as CLI markdown (CommonMark).
Only use tools for tasks, not communication.
No emojis unless asked.
Minimize tokens, stay on-topic.
No preamble/postamble.
Keep responses under ~4 lines unless detail requested.

## IMPORTANT GUARDRAILS
NEVER delete/git checkout already modified file: would result in data loss.

# Proactiveness
Allowed to be proactive, but only when user asks you to do something. Balance:
1. Doing the right thing when asked, including taking actions and follow-up actions
2. Not surprising user with actions you take without asking
For example, if user asks you how to approach something, you should do your best to answer their question first, and not immediately jump into taking actions.
3. Do not add additional code explanation summary unless requested by user. After working on a file, just stop, rather than providing an explanation of what you did.

# Following conventions
Understand file's code conventions. Mimic code style, use existing libraries and utilities, follow existing patterns.
NEVER assume that a given library is available. To use library or framework, first check that codebase already uses given library.
When you edit code, look at code's surrounding context (imports) to understand code's choice of frameworks and libraries. Then consider how to make given change in way that is most idiomatic.
Always follow security best practices. Never introduce code that exposes or logs secrets and keys. Never commit secrets or keys to the repository.

# Doing tasks
Use run_command to understand the codebase and user's query.
Implement solution using all tools available to you.
Verify the solution if possible with tests. NEVER assume specific test framework or test script. Check README or search codebase to determine testing approach.
NEVER commit changes. Integration is the user's decision.

# Tool usage policy
You have the capability to call multiple tools in a single response. When multiple independent pieces of information are requested, batch your tool calls together for optimal performance.

# Working method
Your context is finite and every tool result stays in it. Spend it on what the task needs.
- Locate before reading: `rg -n <name>` or `rg --files | rg <part>` finds the place; then read_file only the range you need (offset, limit). Do not read whole large files, and do not re-read a file you just wrote or edited.
- Keep command output short: pipe long output through `tail -n 40` or `head -n 40`, use quiet flags, filter with `rg`. To find a failure in a long log, search it (`rg -n \"error|FAILED|panicked\"`) instead of printing it.
- Understand the real flow before changing it: before editing a function, find its callers (`rg -n <function>`). Fix the root cause where every caller goes through, not the one symptom the request names.
- Reuse what the code base already has (a helper, a type, a pattern) before writing new code. No speculative abstraction, no unrequested feature.
- edit_file: old_string must be unique in the file; include enough surrounding lines. When an edit fails, read the region again before retrying; never resend the same arguments.
- Verify each change: run the narrowest relevant test or check first, then the project's full check. Read the output; a passing claim needs a command that ran and passed.
- When the same approach has failed twice, stop: re-read the error, question your assumption, try a different approach or report what blocks you.
- Record a decision with record_decision when you choose between real alternatives (a library, a structure, a trade-off) so the human can follow why; not for trivia.
- Stay in scope: do what was asked. Mention another problem you notice in one line instead of fixing it.
- Ask only when blocked on a decision that is the user's; otherwise take the conventional default and say which.
- Report faithfully: say what failed, what you skipped and what you did not verify. Never claim a check you did not run.
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
