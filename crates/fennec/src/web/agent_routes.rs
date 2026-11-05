//! The agent routes (contract: `docs/api.md`): the human manages their own
//! agents here; no agent tool reaches them. `crate::agents` owns discovery and
//! the CRUD, this module only translates HTTP to its calls.
//!
//! The Settings page is global while a project's `.agents/agents` is not, so a
//! request may name a project: its agents are then listed, and shown as the
//! repository's, not the human's.

use std::net::TcpStream;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::App;
use super::http::{Request, error, send_json};
use crate::agents;
use crate::agents::crud;
use crate::domain::agent::{AgentDefinition, CommandAccess, KNOWN_TOOLS};
use crate::domain::agent_file::AgentDraft;

/// `GET /api/agents[?projectId=…]`: every agent, with where it came from and
/// whether the human may edit it, plus the tools an agent may be given.
pub fn list(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let workdir = project_workdir(app, req)?;
    let mut agents = agents::discover(&workdir);
    // The same order as the composer's menu, so the two screens agree.
    agents.sort_by_key(|agent| (agent.phase.is_none(), agent.phase));
    let out: Vec<Value> = agents.iter().map(view).collect();

    send_json(
        stream,
        200,
        &json!({ "agents": out, "tools": KNOWN_TOOLS, "broken": broken() }),
    )
}

/// `GET /api/agents/{id}`: the human's `<id>.agent.md`, both verbatim and
/// parsed into fields. The form fills itself from `fields` rather than
/// re-implementing the frontmatter parser in the browser, and still shows the
/// file as it is when it does not parse (`fields` null then).
pub fn get(stream: &mut TcpStream, id: &str) -> Result<(), String> {
    match crud::read(id) {
        Ok(content) => {
            let fields = crate::domain::agent::parse_agent_md(&content)
                .ok()
                .map(|agent| fields(&agent));
            send_json(
                stream,
                200,
                &json!({ "id": id, "content": content, "fields": fields }),
            )
        }
        Err(e) => refuse(stream, &e),
    }
}

/// The agent as the edit form's inputs, so the browser parses no frontmatter.
fn fields(agent: &AgentDefinition) -> Value {
    json!({
        "id": agent.id,
        "name": agent.name,
        "description": agent.description,
        "phase": agent.phase,
        "color": agent.color,
        "subagent": agent.subagent,
        "readOnly": agent.commands == CommandAccess::ReadOnly,
        "reasoningEffort": agent.reasoning_effort,
        "allowedTools": agent.allowed_tools,
        "prompt": agent.prompt,
    })
}

/// `POST /api/agents`: write the human's `<id>.agent.md`, new or replaced.
/// The form sends fields and the domain renders the file, so the frontmatter
/// format lives in one place.
pub fn save(app: &App, req: &Request, stream: &mut TcpStream) -> Result<(), String> {
    let draft = match serde_json::from_value::<AgentDraft>(req.json()?) {
        Ok(draft) => draft,
        Err(e) => return error(stream, 400, &format!("the agent fields do not read: {e}")),
    };

    let workdir = project_workdir(app, req)?;
    match crud::save(&workdir, &draft.id, &draft.render()) {
        Ok(agent) => send_json(stream, 200, &json!({ "agent": view(&agent) })),
        Err(e) => refuse(stream, &e),
    }
}

/// `POST /api/agents/{id}/delete`: drop the human's `<id>.agent.md`.
pub fn delete(app: &App, req: &Request, stream: &mut TcpStream, id: &str) -> Result<(), String> {
    let workdir = project_workdir(app, req)?;
    match crud::delete(&workdir, id) {
        Ok(()) => send_json(stream, 200, &json!({ "ok": true })),
        Err(e) => refuse(stream, &e),
    }
}

/// The CRUD's refusal, with the field at fault so the form can point at it.
fn refuse(stream: &mut TcpStream, e: &crud::CrudError) -> Result<(), String> {
    let field = e.field();
    super::http::send_json(
        stream,
        e.status(),
        &json!({ "error": e.to_string(), "field": field }),
    )
}

/// The workdir whose `.agents/agents` counts for this request: the named
/// project's, else a path that holds none.
fn project_workdir(app: &App, req: &Request) -> Result<PathBuf, String> {
    let Some(id) = req.query_param("projectId") else {
        return Ok(PathBuf::new());
    };
    let workdir = app
        .store()?
        .project(&id)?
        .map(|project| PathBuf::from(project.workdir))
        .unwrap_or_default();
    Ok(workdir)
}

fn view(agent: &AgentDefinition) -> Value {
    json!({
        "id": agent.id,
        "name": agent.name,
        "description": agent.description,
        "color": agent.color,
        "subagent": agent.subagent,
        "phase": agent.phase,
        "origin": agent.origin,
        "path": agent.path,
        "editable": agent.origin.editable(),
        "readOnly": agent.commands == CommandAccess::ReadOnly,
        "allowedTools": agent.allowed_tools,
    })
}

/// The human's `*.agent.md` files discovery skipped, with the reason: without
/// this a typo makes an agent silently vanish from the menu with no clue why.
fn broken() -> Vec<Value> {
    crud::unparsable()
        .into_iter()
        .map(|(id, error)| json!({ "id": id, "error": error.to_string(), "field": error.field() }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::crud::CrudError;
    use crate::domain::agent::{AgentOrigin, AgentParseError, WriteScope};

    fn agent(origin: AgentOrigin) -> AgentDefinition {
        AgentDefinition {
            id: "mine".into(),
            name: "Mine".into(),
            description: "does things".into(),
            subagent: false,
            composer: true,
            allowed_tools: vec!["read_file".into()],
            commands: CommandAccess::ReadOnly,
            writes: WriteScope::Any,
            reasoning_effort: None,
            color: None,
            phase: None,
            prompt: "Mine.".into(),
            origin,
            path: Some("/home/me/.agents/agents/mine.agent.md".into()),
        }
    }

    #[test]
    fn the_view_says_the_origin_and_who_may_edit_it() {
        let user = view(&agent(AgentOrigin::User));
        assert_eq!(user["origin"], json!("user"));
        assert_eq!(user["editable"], json!(true));
        assert_eq!(user["readOnly"], json!(true));
        assert_eq!(user["allowedTools"], json!(["read_file"]));

        for origin in [AgentOrigin::Default, AgentOrigin::Project] {
            assert_eq!(view(&agent(origin))["editable"], json!(false));
        }
        assert_eq!(
            view(&agent(AgentOrigin::Project))["origin"],
            json!("project")
        );
        assert_eq!(
            view(&agent(AgentOrigin::Default))["origin"],
            json!("default")
        );
    }

    #[test]
    fn a_refusal_names_the_field_at_fault_and_its_status() {
        assert_eq!(CrudError::Invalid(AgentParseError::MissingId).field(), "id");
        assert_eq!(
            CrudError::Invalid(AgentParseError::EmptyBody).field(),
            "prompt"
        );
        assert_eq!(
            CrudError::UnknownTool("rm_rf".into()).field(),
            "allowedTools"
        );
        assert_eq!(CrudError::UnknownTool("rm_rf".into()).status(), 400);
        assert_eq!(
            CrudError::NotEditable(AgentOrigin::Default).status(),
            409,
            "a default is taken, not malformed"
        );
        assert_eq!(CrudError::NotFound.status(), 404);
    }
}
