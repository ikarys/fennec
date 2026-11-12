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
use crate::domain::mcp;
use crate::domain::preferences::{ModelRef, Preferences, ReasoningEffort};
use crate::domain::project::SessionSummary;
use crate::domain::protection::Protection;
use crate::domain::test_files;
use crate::domain::waiver;
use crate::instructions::all_instructions;
use crate::llm::OpenAiClient;
use crate::metering::MeteredLog;
use crate::notifier::{Notifier, NotifyingLog};
use crate::protected;
use crate::sandbox::Sandbox;
use crate::skills;
use crate::store::SqliteStore;
use crate::tools::{CanvasTarget, LocalTools};
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
    /// The session's effort chosen by the human, already checked against the model.
    pub effort_choice: Option<ReasoningEffort>,
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
    settings.require_model(&prefs, options.model_choice.as_ref())?;
    let params = settings.llm.for_turn(&prefs, options.model_choice.as_ref());
    let instructions = all_instructions(prefs.global_instructions.as_deref(), workdir);
    let skills = skills::discover(workdir);
    let mut agents = agents::discover(workdir);
    // The agents are told of (their reminder), and may call, only the tools this turn
    // can run. Delegation also needs the setting on. The definitions stay whole: one
    // prefix for every session, so a new one (a delegate's child included) starts from
    // the server's cache.
    let can_delegate = options.children.is_some() && prefs.delegation.enabled;
    for agent in &mut agents {
        agent
            .allowed_tools
            .retain(|tool| !delegate::withheld(tool, can_delegate, options.delegated));
    }
    let model = OpenAiClient::new(settings.endpoint(&prefs, &params.provider)?, &params.model);
    // The human's rules live in the `rules` crate's own database. They are
    // loaded per turn so a rule changed in the UI applies from the next turn.
    let rule_store = rules::RuleStore::open(&settings.rules_path()).map_err(|e| e.to_string())?;
    let session = store.session(session_id)?;
    let task_sheet = task_sheet(store, session.as_ref())?;
    // The budget is a human decision the kernel enforces before any turn starts:
    // every path that starts a turn (web, CLI, the ideas queue, workflow children)
    // goes through `with_turn`.
    if let Some(session) = &session {
        refuse_over_budget(settings, store, log, session, options)?;
    }
    // The kernel's own rules come with them, CLI and workflow shell nodes included.
    let rules = kernel_rules::with_kernel(match &session {
        Some(session) => rule_store
            .for_turn(&session.project_id, Some(session_id))
            .map_err(|e| e.to_string())?,
        None => rules::Rules::default(),
    });
    let events = log.events(session_id)?;
    let tools = build_tools(
        settings,
        store,
        workdir,
        session.as_ref(),
        &prefs,
        rules,
        &events,
    )?;
    let mut turn = Turn {
        model: &model,
        log,
        tools: &tools,
        session_id,
        workdir,
        task_sheet: task_sheet.as_deref(),
        instructions: &instructions,
        skills: &skills,
        always_on: &prefs.always_on_skills,
        style: prefs.agent_style,
        agents: &agents,
        max_tokens: params.max_tokens,
        context_window: params.context_window,
        compact_at: params.compact_at,
        reasoning_effort: params.reasoning_effort,
        session_effort: options.effort_choice,
        autonomous: options.autonomous,
        lang: prefs.display.language,
        preflight_sent_back: false,
        stop,
        on_event: &mut options.on_event,
        children: options.children,
    };
    body(&mut turn, &tools)
}

/// The task sheet of the idea this session frames, as markdown.
fn task_sheet(
    store: &SqliteStore,
    session: Option<&SessionSummary>,
) -> Result<Option<String>, String> {
    let Some(session) = session else {
        return Ok(None);
    };
    let ideas = store
        .backlog(&session.project_id)
        .map_err(|e| e.to_string())?
        .ideas;
    let Some(idea) = backlog::idea_for_session(&ideas, &session.id) else {
        return Ok(None);
    };
    let Some(sheet) = idea.sheet.as_ref() else {
        return Ok(None);
    };
    Ok(Some(sheet.to_markdown(&backlog::title(&idea.text, None))))
}

/// Refuses the turn when the project's budget is spent: the refusal is logged as the turn's error.
fn refuse_over_budget(
    settings: &Settings,
    store: &SqliteStore,
    log: &dyn EventLog,
    session: &SessionSummary,
    options: &mut Options,
) -> Result<(), String> {
    let Err(refusal) = crate::budget::check(settings, store, &session.project_id) else {
        return Ok(());
    };
    for event in [
        Event::ChatError {
            error: refusal.clone(),
            recoverable: false,
        },
        Event::ChatDone {
            message_id: String::new(),
            reason: DoneReason::Error,
            duration_ms: None,
        },
    ] {
        log.append(&session.id, &event)?;
        (options.on_event)(&event);
    }
    Err(refusal)
}

/// The `Block` path rules that cover writes, rebased to the work folder for the
/// file tools' protection and the sandbox's read-only mounts, and whether one
/// covers the whole work folder. The rest are checked per call by the `rules`
/// crate.
fn blocked_write_paths(
    rules: &rules::Rules,
    workdir: &Path,
    home: Option<&str>,
) -> (Vec<String>, bool) {
    let root = workdir.to_string_lossy();
    let mut patterns = Vec::new();
    let mut whole = false;
    for rule in rules.all() {
        if !matches!(rule.action, rules::Action::Block) {
            continue;
        }
        let rules::Target::Path { pattern, access } = &rule.target else {
            continue;
        };
        if !access.covers(rules::Access::Write) {
            continue;
        }
        let Some(under) = pattern.under(&root, home) else {
            continue;
        };
        match under {
            rules::Under::Root => whole = true,
            rules::Under::Pattern(pattern) => patterns.push(pattern),
        }
    }
    (patterns, whole)
}

/// The sandbox with the protected files read-only, or the whole work folder when a
/// `Block` write rule covers it: commands cannot write what the file tools refuse.
fn guarded(sandbox: Sandbox, workdir: &Path, protection: &Protection, whole: bool) -> Sandbox {
    let sandbox = sandbox.protecting(protected::existing_paths(workdir, protection));
    if whole {
        sandbox.read_only(workdir).unwrap_or(sandbox)
    } else {
        sandbox
    }
}

/// The turn's toolbox: the sandbox, the protected files, the rules, MCP and the canvas.
fn build_tools(
    settings: &Settings,
    store: &SqliteStore,
    workdir: &Path,
    session: Option<&SessionSummary>,
    prefs: &Preferences,
    rules: rules::Rules,
    events: &[Event],
) -> Result<LocalTools, String> {
    let home = std::env::var("HOME").ok();
    let (blocked_paths, whole) = blocked_write_paths(&rules, workdir, home.as_deref());
    // ponytail: a waiver's expiry is checked when a turn starts, not mid-turn.
    let waiver_paths = waiver::active(events, clock::now_ms());
    let protection = (if workdir.starts_with(worktree::root(&settings.db_path)) {
        protected::load_for_worktree(workdir)
    } else {
        protected::load(workdir)
    })
    .with_patterns(&blocked_paths)
    // Validated at the test gate, protected from the next turn.
    .with_patterns(&test_files::approved(events))
    .exempting(&waiver_paths);
    let shared_target = worktree::shared_target(&worktree::root(&settings.db_path), workdir);
    // The model may still name the human's checkout after the session moved
    // into its worktree: the file tools redirect those paths.
    let project_dir = match session {
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
    let sandbox = guarded(
        Sandbox::for_store(&prefs.sandbox, &settings.db_path),
        workdir,
        &protection,
        whole,
    )
    .with_mounts(
        worktree::main_view(workdir)
            .into_iter()
            .chain(worktree::dependency_mounts(workdir))
            .collect(),
    )
    .writing(shared_target.iter().cloned().collect());
    let tools = LocalTools::new(workdir.to_path_buf())
        .with_search(prefs.search.clone())
        .with_sandbox(sandbox)
        .with_protection(protection)
        .with_env(
            shared_target
                .map(|dir| ("CARGO_TARGET_DIR".to_string(), dir))
                .into_iter()
                .collect(),
        )
        .with_rules(rules)
        .with_mcp(mcp::active_servers(events, prefs))
        .with_mcp_vault(crate::mcp::oauth::Vault::for_settings(settings));
    let tools = match project_dir {
        Some(dir) => tools.with_project_dir(dir),
        None => tools,
    };
    let tools = match session {
        Some(session) => tools.with_canvas(CanvasTarget {
            db: settings.canvas_path(),
            project_id: session.project_id.clone(),
            session_id: session.id.clone(),
        }),
        None => tools,
    };
    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmSettings;
    use crate::domain::preferences::{LOCAL_PROVIDER, ModelParams};

    fn settings(dir: &Path) -> Settings {
        Settings {
            llm: LlmSettings {
                base_url: Some("http://127.0.0.1:8000/v1".into()),
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
            effort_choice: None,
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

    #[test]
    fn blocked_write_paths_rebases_path_rules() {
        fn rule(
            id: i64,
            pattern: &str,
            access: rules::Access,
            action: rules::Action,
        ) -> rules::Rule {
            rules::Rule {
                id,
                scope: rules::Scope::Global,
                target: rules::Target::parse(rules::TargetKind::Path, pattern, Some(access))
                    .unwrap(),
                action,
                note: None,
                origin: rules::Origin::Human,
            }
        }

        let workdir = std::path::Path::new("/tmp/wt/s1");
        let rules = rules::Rules::new(vec![
            rule(1, "todo/cli.py", rules::Access::Write, rules::Action::Block),
            rule(
                2,
                "/tmp/wt/s1/todo/cli.py",
                rules::Access::Write,
                rules::Action::Block,
            ),
            rule(3, "/etc/passwd", rules::Access::Write, rules::Action::Block),
            rule(
                4,
                "/tmp/wt/s1/todo/cli.py",
                rules::Access::Write,
                rules::Action::Allow,
            ),
            rule(
                5,
                "/tmp/wt/s1/todo/cli.py",
                rules::Access::Read,
                rules::Action::Block,
            ),
        ]);
        let (patterns, whole) = blocked_write_paths(&rules, workdir, None);
        assert_eq!(
            patterns,
            vec!["todo/cli.py".to_string(), "/todo/cli.py".to_string()]
        );
        assert!(!whole);

        let whole_rules = rules::Rules::new(vec![rule(
            6,
            "/tmp/",
            rules::Access::Write,
            rules::Action::Block,
        )]);
        let (patterns, whole) = blocked_write_paths(&whole_rules, workdir, None);
        assert!(patterns.is_empty());
        assert!(whole);
    }

    #[test]
    fn e2e_an_absolute_block_write_rule_stops_a_command() {
        if !crate::sandbox::tests::bwrap_available() {
            return;
        }
        fn block(pattern: &str) -> rules::Rules {
            rules::Rules::new(vec![rules::Rule {
                id: 1,
                scope: rules::Scope::Global,
                target: rules::Target::parse(
                    rules::TargetKind::Path,
                    pattern,
                    Some(rules::Access::Write),
                )
                .unwrap(),
                action: rules::Action::Block,
                note: None,
                origin: rules::Origin::Human,
            }])
        }
        let run = |sandbox: &Sandbox, workdir: &Path, command: &str| {
            sandbox
                .spawn("sh", &["-c".into(), command.into()], workdir, workdir)
                .output()
                .unwrap()
                .status
                .success()
        };
        let base = std::env::temp_dir().join(format!("fennec-rule-e2e-{}", std::process::id()));
        let workdir = base.join("work");
        std::fs::create_dir_all(&workdir).unwrap();
        let file = workdir.join("cli.py");
        std::fs::write(&file, "original").unwrap();
        let prefs = crate::domain::preferences::SandboxPrefs {
            enabled: true,
            network: false,
        };
        let sandbox_for = |rules: &rules::Rules| {
            let (patterns, whole) = blocked_write_paths(rules, &workdir, None);
            let protection = Protection::default().with_patterns(&patterns);
            guarded(
                Sandbox::from_prefs(&prefs, None, &base),
                &workdir,
                &protection,
                whole,
            )
        };

        // The file itself, named by its absolute path.
        let sandbox = sandbox_for(&block(&file.to_string_lossy()));
        assert!(!run(&sandbox, &workdir, "echo changed > cli.py"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");
        assert!(run(&sandbox, &workdir, "echo ok > sibling.py"));

        // A folder above the work folder: nothing in it is writable.
        let sandbox = sandbox_for(&block(&format!("{}/", base.display())));
        assert!(!run(&sandbox, &workdir, "echo changed > cli.py"));
        assert!(!run(&sandbox, &workdir, "echo new > new.py"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");

        let _ = std::fs::remove_dir_all(&base);
    }
}
