//! Port of openfox `chat/prompts.ts` (`buildBasePrompt`), with the
//! portable skills section (`buildSkillsSection`).
//!
//! Not ported: sub-agents and caveman thinking.

use crate::domain::clock;
use crate::domain::skill::Skill;
use std::path::Path;

/// Port of openfox `buildBasePrompt`, without the sections for features
/// not ported yet (sub-agents, caveman thinking).
///
/// `instructions` (openfox `instructionsSection`) is appended at the end
/// when non-empty, then `skills` (openfox `buildSkillsSection`) when
/// non-empty.
pub fn system_prompt(
    workdir: &Path,
    model_name: &str,
    instructions: &str,
    skills: &[Skill],
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

    prompt.push_str(&skills_section(skills));
    prompt
}

fn skills_section(skills: &[Skill]) -> String {
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
        let prompt = system_prompt(Path::new("/tmp"), "m", "", &[]);
        assert!(!prompt.contains("CUSTOM INSTRUCTIONS"));
    }

    #[test]
    fn non_empty_instructions_are_appended_at_the_end() {
        let prompt = system_prompt(Path::new("/tmp"), "m", "Be terse.", &[]);
        assert!(prompt.ends_with("## CUSTOM INSTRUCTIONS\n\nBe terse.\n"));
    }

    #[test]
    fn no_skills_add_no_skills_section() {
        let prompt = system_prompt(Path::new("/tmp"), "m", "", &[]);
        assert!(!prompt.contains("AVAILABLE SKILLS"));
    }

    #[test]
    fn two_skills_add_numbered_listing_after_custom_instructions() {
        let skills = vec![skill("alpha", "A"), skill("beta", "B")];
        let prompt = system_prompt(Path::new("/tmp"), "m", "Custom.", &skills);

        assert!(prompt.contains("## CUSTOM INSTRUCTIONS\n\nCustom.\n\n## AVAILABLE SKILLS"));
        assert!(prompt.contains("1. **alpha** - A\n2. **beta** - B"));
        assert!(
            prompt.ends_with(
                "The skill's detailed instructions will be returned as a tool result.\n"
            )
        );
    }
}
