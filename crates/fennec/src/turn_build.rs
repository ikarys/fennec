//! The shared wiring of a `Turn`: what both adapters (CLI, web) assemble from
//! the same set of adapters. The adapters keep only their orchestration; a new
//! `Turn` field or a tools default is added here, once.

use crate::agent::delegate;
use crate::agent::ports::{ChildSessions, EventLog};
use crate::agent::turn::Turn;
use crate::agents;
use crate::config::Settings;
use crate::domain::clock;
use crate::domain::event::{DoneReason, Event};
use crate::domain::kernel_rules;
use crate::domain::lang::Lang;
use crate::domain::mcp;
use crate::domain::preferences::ModelRef;
use crate::domain::test_files;
use crate::domain::waiver;
use crate::instructions::all_instructions;
use crate::llm::OpenAiClient;
use crate::metering::MeteredLog;
use crate::notifier::{Notifier, NotifyingLog};
use crate::protected;
use crate::skills;
use crate::store::SqliteStore;
use crate::tools::{CanvasTarget, LocalTools, Sandbox};
use crate::worktree;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// The differences between the adapters' turns; everything else is shared.
pub struct Options<'a> {
    /// Wrap the log so an append wakes the event streams (the web only).
    pub notifier: Option<&'a Notifier>,
    /// The model the session runs on (a workflow child inherits its parent's
    /// choice); `None` uses the default.
    pub model_choice: Option<ModelRef>,
    /// The human is away (the ideas queue's night mode).
    pub autonomous: bool,
    /// The turn runs a `delegate` call's child, hidden from the human: its agents lose the tools that leave something for them.
    pub delegated: bool,
    /// Called for every event right after it is stored (the CLI prints them).
    pub on_event: &'a mut dyn FnMut(&Event),
    /// Where `delegate` runs its agents (the web's top-level turns only).
    pub children: Option<&'a dyn ChildSessions>,
}

/// Builds the turn's dependencies and hands the turn (and its tools) to
/// `body`. The store, log, prefs, instructions, skills, agents, model and
/// tools stay locals here.
pub fn with_turn<R>(
    settings: &Settings,
    store: &SqliteStore,
    session_id: &str,
    workdir: &Path,
    stop: &AtomicBool,
    options: &mut Options,
    body: impl FnOnce(&mut Turn, &LocalTools) -> Result<R, String>,
) -> Result<R, String> {
    let ledger = token_usage::Ledger::open(&settings.usage_path()).map_err(|e| e.to_string())?;
    let metered = MeteredLog {
        log: store,
        store,
        ledger: &ledger,
    };
    let notifying = options.notifier.map(|notifier| NotifyingLog {
        log: &metered,
        notifier,
    });
    let log: &dyn EventLog = match &notifying {
        Some(n) => n,
        None => &metered,
    };
    let prefs = store.preferences()?;
    let params = settings.llm.for_turn(&prefs, options.model_choice.as_ref());
    let instructions = all_instructions(prefs.global_instructions.as_deref(), workdir);
    let skills = skills::discover(workdir);
    let mut agents = agents::discover(workdir);
    // The agents are told of (their reminder), and may call, only the tools this turn
    // can run. The definitions stay whole: one prefix for every session, so a new one
    // (a delegate's child included) starts from the server's cache.
    let can_delegate = options.children.is_some();
    for agent in &mut agents {
        agent
            .allowed_tools
            .retain(|tool| !delegate::withheld(tool, can_delegate, options.delegated));
    }
    let model = OpenAiClient::new(settings.endpoint(&prefs, &params.provider)?, &params.model);
    // A session worktree is the agent's own copy: a new dependency's lockfile
    // may change there, and the integration gate shows it to the human. Outside
    // a worktree the mount list is empty, so this is a no-op for the CLI.
    // The human's rules live in the `rules` crate's own database. They are
    // loaded per turn so a rule changed in the UI applies from the next turn.
    let rule_store = rules::RuleStore::open(&settings.rules_path()).map_err(|e| e.to_string())?;
    let session = store.session(session_id)?;
    // The budget is a human decision the kernel enforces before any turn starts:
    // every path that starts a turn (web, CLI, the ideas queue, workflow children)
    // goes through `with_turn`.
    if let Some(session) = &session
        && let Err(refusal) = crate::budget::check(settings, store, &session.project_id)
    {
        let error = Event::ChatError {
            error: refusal.clone(),
            recoverable: false,
        };
        log.append(session_id, &error)?;
        (options.on_event)(&error);
        let done = Event::ChatDone {
            message_id: String::new(),
            reason: DoneReason::Error,
            duration_ms: None,
        };
        log.append(session_id, &done)?;
        (options.on_event)(&done);
        return Err(refusal);
    }
    // The kernel's own rules come with them, CLI and workflow shell nodes included.
    let rules = kernel_rules::with_kernel(match &session {
        Some(session) => rule_store
            .for_turn(&session.project_id, Some(session_id))
            .map_err(|e| e.to_string())?,
        None => rules::Rules::default(),
    });
    // Only project-relative `Block` path rules that cover writes can be enforced
    // by the file tools' protection and the sandbox's read-only mounts; the rest
    // are checked per call by the `rules` crate.
    let blocked_paths: Vec<String> = rules
        .all()
        .iter()
        .filter_map(|rule| {
            if !matches!(rule.action, rules::Action::Block) {
                return None;
            }
            match &rule.target {
                rules::Target::Path { pattern, access }
                    if access.covers(rules::Access::Write) && pattern.is_relative() =>
                {
                    Some(pattern.text().to_string())
                }
                _ => None,
            }
        })
        .collect();
    // ponytail: a waiver's expiry is checked when a turn starts, not mid-turn.
    let events = log.events(session_id)?;
    let waiver_paths = waiver::active(&events, clock::now_ms());
    let protection = (if workdir.starts_with(worktree::root(&settings.db_path)) {
        protected::load_for_worktree(workdir)
    } else {
        protected::load(workdir)
    })
    .with_patterns(&blocked_paths)
    // Validated at the test gate, protected from the next turn.
    .with_patterns(&test_files::approved(&events))
    .exempting(&waiver_paths);
    let shared_target = worktree::shared_target(&worktree::root(&settings.db_path), workdir);
    // The model may still name the human's checkout after the session moved
    // into its worktree: the file tools redirect those paths.
    let project_dir = match &session {
        Some(session) => store
            .project(&session.project_id)
            .map_err(|e| e.to_string())?
            .map(|p| PathBuf::from(p.workdir)),
        None => None,
    };
    // macOS cannot bind-mount the main checkout's dependency directories as bwrap does: link them,
    // and Seatbelt refuses writes through the links (their targets are outside the workdir).
    #[cfg(target_os = "macos")]
    if let Err(e) = worktree::link_dependencies(workdir) {
        eprintln!("fennec: link the dependencies: {e}");
    }
    let tools = LocalTools::new(workdir.to_path_buf())
        .with_search(prefs.search.clone())
        .with_sandbox(
            Sandbox::for_store(&prefs.sandbox, &settings.db_path)
                .protecting(protected::existing_paths(workdir, &protection))
                .with_mounts(worktree::dependency_mounts(workdir))
                .writing(shared_target.iter().cloned().collect()),
        )
        .with_protection(protection)
        .with_env(
            shared_target
                .map(|dir| ("CARGO_TARGET_DIR".to_string(), dir))
                .into_iter()
                .collect(),
        )
        .with_rules(rules)
        .with_mcp(mcp::active_servers(&events, &prefs))
        .with_mcp_vault(crate::mcp::oauth::Vault::for_settings(settings));
    let tools = match project_dir {
        Some(dir) => tools.with_project_dir(dir),
        None => tools,
    };
    let tools = match &session {
        Some(session) => tools.with_canvas(CanvasTarget {
            db: settings.canvas_path(),
            project_id: session.project_id.clone(),
            session_id: session_id.to_string(),
        }),
        None => tools,
    };
    let mut turn = Turn {
        model: &model,
        log,
        tools: &tools,
        session_id,
        workdir,
        instructions: &instructions,
        skills: &skills,
        always_on: &prefs.always_on_skills,
        agents: &agents,
        max_tokens: params.max_tokens,
        context_window: params.context_window,
        compact_at: params.compact_at,
        reasoning_effort: params.reasoning_effort,
        autonomous: options.autonomous,
        lang: Lang::from_code(&prefs.display.language),
        preflight_sent_back: false,
        stop,
        on_event: &mut options.on_event,
        children: options.children,
    };
    body(&mut turn, &tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmSettings;
    use crate::domain::preferences::{LOCAL_PROVIDER, ModelParams};

    fn settings(dir: &Path) -> Settings {
        Settings {
            llm: LlmSettings {
                base_url: "http://127.0.0.1:8000/v1".into(),
                api_key: None,
                defaults: ModelParams {
                    provider: LOCAL_PROVIDER.to_string(),
                    model: "m".into(),
                    max_tokens: 100,
                    context_window: 2048,
                    compact_at: 0.85,
                    reasoning_effort: None,
                },
                listed: vec![],
            },
            db_path: dir.join("fennec.db"),
        }
    }

    #[test]
    fn the_options_land_on_the_turn() {
        let store = SqliteStore::in_memory().unwrap();
        let dir = std::env::temp_dir().join(format!("fennec-turn-build-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let settings = settings(&dir);
        let stop = AtomicBool::new(false);
        let mut on_event = |_: &Event| {};
        let mut options = Options {
            notifier: None,
            model_choice: None,
            autonomous: true,
            delegated: false,
            on_event: &mut on_event,
            children: None,
        };
        with_turn(
            &settings,
            &store,
            "s",
            &dir,
            &stop,
            &mut options,
            |turn, _tools| {
                assert!(turn.autonomous);
                assert_eq!(turn.stop as *const _, &stop as *const _);
                assert_eq!(turn.model.name(), "m");
                assert_eq!(turn.context_window, 2048);
                Ok(())
            },
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
