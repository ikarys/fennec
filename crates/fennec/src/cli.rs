//! Terminal adapter: run a turn in the current directory, list sessions,
//! replay one, compact one.

mod transcript;

use crate::agent::ports::EventLog;
use crate::agent::turn::{Turn, compact_session, run_turn};
use crate::config::Settings;
use crate::domain::event::{ChoiceOption, DoneReason, Event};
use crate::domain::open_calls;
use crate::instructions::all_instructions;
use crate::llm::OpenAiClient;
use crate::store::SqliteStore;
use crate::tools::LocalTools;
use std::io::{BufRead, Write};
use std::sync::atomic::AtomicBool;
use transcript::Transcript;

/// Maps a line the human typed to the answer sent to the model: a number
/// matching an option (1-based) is that option's `value`, anything else is
/// the line as typed.
fn answer_from_line(line: &str, options: &[ChoiceOption]) -> String {
    if let Ok(n) = line.trim().parse::<usize>()
        && (1..=options.len()).contains(&n)
    {
        return options[n - 1].value.clone();
    }
    line.trim().to_string()
}

/// Runs one turn in the current directory, in `session` or a new one.
pub fn run(settings: &Settings, session: Option<String>, prompt: &str) -> Result<(), String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let workdir = std::env::current_dir().map_err(|e| e.to_string())?;
    let project = store.project_for(&workdir.display().to_string())?;
    let session_id = match session {
        Some(id) => id,
        None => store.create_session(&project, &prompt.chars().take(80).collect::<String>())?,
    };
    let reason = with_turn(settings, &store, &session_id, |turn| {
        let mut reason = run_turn(turn, prompt)?;
        // The turn ended waiting for the human: read the answer and run the
        // next turn, whose input is that answer.
        while reason == DoneReason::WaitingForUser {
            let options = pending_options(&store.events(&session_id)?)
                .ok_or_else(|| "turn ended: waiting_for_user".to_string())?;
            let Some(line) = read_answer()? else {
                return Err("turn ended: waiting_for_user".to_string());
            };
            reason = run_turn(turn, &answer_from_line(&line, &options))?;
        }
        Ok(reason)
    })?;
    match reason {
        DoneReason::Complete => Ok(()),
        reason => Err(format!("turn ended: {reason}")),
    }
}

/// Compacts `session` on demand, in the current directory (the CLI has no
/// project lookup by session).
pub fn compact(settings: &Settings, session: &str) -> Result<(), String> {
    let store = SqliteStore::open(&settings.db_path)?;
    if store.session(session)?.is_none() {
        return Err(format!("no session {session}"));
    }
    match with_turn(settings, &store, session, compact_session)? {
        DoneReason::Complete => Ok(()),
        reason => Err(format!("compaction ended: {reason}")),
    }
}

/// Builds a turn on `session` in the current directory, printing its events.
fn with_turn(
    settings: &Settings,
    store: &SqliteStore,
    session_id: &str,
    work: impl FnOnce(&mut Turn) -> Result<DoneReason, String>,
) -> Result<DoneReason, String> {
    let workdir = std::env::current_dir().map_err(|e| e.to_string())?;
    let prefs = store.preferences()?;
    let params = settings.llm.for_turn(&prefs);
    let instructions = all_instructions(prefs.global_instructions.as_deref(), &workdir);
    eprintln!(
        "session {session_id} · {} · {}",
        params.model,
        workdir.display()
    );
    let model = OpenAiClient::new(&settings.llm, &params.model);
    let tools = LocalTools::new(workdir.clone());
    // ponytail: no Ctrl-C handling, the process just dies; a signal handler
    // setting `stop` when an interrupted turn must end cleanly.
    let stop = AtomicBool::new(false);
    let mut transcript = Transcript::default();
    let mut on_event = |e: &Event| transcript.print(e);
    work(&mut Turn {
        model: &model,
        log: store,
        tools: &tools,
        session_id,
        workdir: &workdir,
        instructions: &instructions,
        max_tokens: params.max_tokens,
        context_window: params.context_window,
        compact_at: params.compact_at,
        reasoning_effort: params.reasoning_effort,
        stop: &stop,
        on_event: &mut on_event,
    })
}

/// The human's answer from stdin; `None` on end of input or an empty line.
fn read_answer() -> Result<Option<String>, String> {
    eprint!("answer> ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok((read > 0 && !line.trim().is_empty()).then_some(line))
}

/// The options of the `chat.ask_user` whose call is still open, if any.
/// A text question has no options, so it is reported as an empty list;
/// `None` means there is no pending question.
fn pending_options(events: &[Event]) -> Option<Vec<ChoiceOption>> {
    let call_id = open_calls::open_calls(events)
        .into_iter()
        .find(|call| call.awaits_answer)?
        .call_id;
    let event = events
        .iter()
        .find(|e| matches!(e, Event::ChatAskUser { call_id: id, .. } if id == &call_id))?;
    match event {
        Event::ChatAskUser { options, .. } => Some(options.clone().unwrap_or_default()),
        _ => None,
    }
}

pub fn list_sessions(store: &SqliteStore) -> Result<(), String> {
    let projects = store.projects()?;
    for s in store.sessions(None)? {
        let workdir = projects
            .iter()
            .find(|p| p.id == s.project_id)
            .map_or("?", |p| p.workdir.as_str());
        println!("{}  {workdir}  {}", s.id, s.title);
    }
    Ok(())
}

pub fn replay(store: &SqliteStore, session_id: &str) -> Result<(), String> {
    let events = store.events(session_id)?;
    if events.is_empty() {
        return Err(format!("no session {session_id}"));
    }
    let mut transcript = Transcript::default();
    events.iter().for_each(|e| transcript.print(e));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{answer_from_line, pending_options};
    use crate::domain::event::{AskType, ChoiceOption, Event, ToolCall, ToolResult};
    use serde_json::json;

    fn opt(value: &str, label: &str) -> ChoiceOption {
        ChoiceOption {
            value: value.into(),
            label: label.into(),
            description: None,
        }
    }

    fn ask_call(id: &str) -> Event {
        Event::ToolCall {
            message_id: "m".into(),
            tool_call: ToolCall {
                id: id.into(),
                name: "ask_user".into(),
                arguments: json!({}),
                parse_error: None,
            },
        }
    }

    fn ask_event(id: &str, options: Option<Vec<ChoiceOption>>) -> Event {
        Event::ChatAskUser {
            call_id: id.into(),
            question: "Which one?".into(),
            ask_type: if options.is_some() {
                AskType::Choice
            } else {
                AskType::Text
            },
            options,
        }
    }

    #[test]
    fn a_number_maps_to_the_option_value() {
        let options = vec![opt("vllm", "vLLM"), opt("ollama", "Ollama")];
        assert_eq!(answer_from_line("1", &options), "vllm");
        assert_eq!(answer_from_line("2", &options), "ollama");
        assert_eq!(answer_from_line(" 2 ", &options), "ollama");
    }

    #[test]
    fn a_number_out_of_range_is_kept_as_typed() {
        let options = vec![opt("vllm", "vLLM")];
        assert_eq!(answer_from_line("3", &options), "3");
        assert_eq!(answer_from_line("0", &options), "0");
    }

    #[test]
    fn free_text_is_kept_as_typed() {
        let options = vec![opt("vllm", "vLLM")];
        assert_eq!(
            answer_from_line("use vllm please", &options),
            "use vllm please"
        );
        assert_eq!(answer_from_line("blue", &[]), "blue");
    }

    #[test]
    fn pending_options_returns_the_options_of_the_open_question() {
        let options = vec![opt("vllm", "vLLM"), opt("ollama", "Ollama")];
        let events = vec![ask_call("c1"), ask_event("c1", Some(options.clone()))];
        assert_eq!(pending_options(&events), Some(options));
    }

    #[test]
    fn pending_options_returns_an_empty_list_for_a_text_question() {
        let events = vec![ask_call("c1"), ask_event("c1", None)];
        assert_eq!(pending_options(&events), Some(Vec::new()));
    }

    #[test]
    fn pending_options_is_none_when_the_question_is_resolved() {
        let events = vec![
            ask_call("c1"),
            ask_event("c1", None),
            Event::ToolResult {
                message_id: "m".into(),
                tool_call_id: "c1".into(),
                result: ToolResult::success("ok".into(), false),
            },
        ];
        assert_eq!(pending_options(&events), None);
    }

    #[test]
    fn pending_options_is_none_without_a_pending_question() {
        assert_eq!(pending_options(&[]), None);
    }
}
