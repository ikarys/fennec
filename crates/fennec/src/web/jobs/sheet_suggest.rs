//! One model call proposing one task sheet field (indicative); same shape as `commit_subject.rs`,
//! synchronous, called by the route.

use crate::agent::ports::{Delta, Model, Request};
use crate::config::Settings;
use crate::domain::lang::Lang;
use crate::domain::preferences::ReasoningEffort;
use crate::domain::sheet_suggest::{self, Suggestion};
use crate::llm::OpenAiClient;
use crate::store::SqliteStore;
use backlog::{Idea, SheetField};
use serde_json::json;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub fn suggest(
    settings: &Settings,
    workdir: &Path,
    idea: &Idea,
    field: SheetField,
    lang: Lang,
) -> Result<Suggestion, String> {
    let store = SqliteStore::open(&settings.db_path)?;
    let prefs = store.preferences()?;
    let params = settings.llm.for_turn(&prefs, None);
    let model = OpenAiClient::new(settings.endpoint(&prefs, &params.provider)?, &params.model);
    let stop = AtomicBool::new(false);

    let agents_md = read_agents_md(workdir);
    let text_attachments = super::idea_assets::text_files(&idea.attachments);
    let sheet = idea.sheet.clone().unwrap_or_default();

    let content = sheet_suggest::prompt(
        &idea.text,
        &text_attachments,
        &sheet,
        field,
        agents_md.as_deref(),
        lang,
    );
    let messages = vec![json!({ "role": "user", "content": content })];
    let tools = json!([]);
    let model_request = Request {
        system_prompt: "",
        messages: &messages,
        tools: &tools,
        max_tokens: 4096,
        reasoning_effort: Some(ReasoningEffort::None),
        stop: &stop,
        session: "",
    };

    let mut on_delta = |_: Delta| {};
    let completion = model.stream(&model_request, &mut on_delta)?;

    sheet_suggest::parse_reply(field, &completion.content)
}

fn read_agents_md(workdir: &Path) -> Option<String> {
    let path = workdir.join("AGENTS.md");
    // A symlinked or huge AGENTS.md must not make the server read outside the project or load an unbounded file.
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return None;
    }

    let file = std::fs::File::open(&path).ok()?;
    let mut buf = Vec::new();
    file.take(64 * 1024).read_to_end(&mut buf).ok()?;

    Some(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[cfg(unix)]
    #[test]
    fn agents_md_symlink_is_not_read() {
        use std::os::unix::fs::symlink;

        let target_dir = std::env::temp_dir().join(format!(
            "fennec-sheet-suggest-target-{}",
            crate::domain::id::new_id()
        ));
        let workdir = std::env::temp_dir().join(format!(
            "fennec-sheet-suggest-{}",
            crate::domain::id::new_id()
        ));

        fs::create_dir_all(&target_dir).unwrap();
        fs::create_dir_all(&workdir).unwrap();

        let target = target_dir.join("AGENTS.md");
        fs::write(&target, "secret").unwrap();
        symlink(&target, workdir.join("AGENTS.md")).unwrap();

        assert_eq!(read_agents_md(&workdir), None);

        fs::remove_file(workdir.join("AGENTS.md")).unwrap();
        fs::write(workdir.join("AGENTS.md"), "rules").unwrap();
        assert_eq!(read_agents_md(&workdir), Some("rules".to_string()));

        fs::remove_dir_all(&target_dir).unwrap();
        fs::remove_dir_all(&workdir).unwrap();
    }
}
