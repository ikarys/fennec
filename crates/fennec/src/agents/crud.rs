//! The human's agents: reading, writing and deleting the `*.agent.md`
//! files of `~/.agents/agents`.
//!
//! Only the human reaches this module (the Settings page, through
//! `web::agent_routes`): it is kernel code, and no agent tool calls it. The
//! defaults are embedded and a project's agents belong to the repository, so
//! neither is written here; the UI duplicates a default under a new id
//! instead.

use std::fs;
use std::path::{Path, PathBuf};

use super::{DEFAULTS, agent_files, home, load_agents_directory, user_dir};
use crate::domain::agent::{AgentDefinition, AgentOrigin, AgentParseError, parse_agent_md};

/// Why the human's agent CRUD could not do what was asked.
#[derive(Debug, Clone, PartialEq)]
pub enum CrudError {
    /// The id is not a usable file name (kebab-case, 1-64 chars).
    BadId,
    /// The content does not parse as an agent.
    Invalid(AgentParseError),
    /// The frontmatter `id` is not the id of the request.
    IdMismatch,
    /// A tool named in `allowedTools` is not a tool fennec has.
    UnknownTool(String),
    /// No user agent of that id.
    NotFound,
    /// That id belongs to a default or to the project: not the human's to
    /// write here.
    NotEditable(AgentOrigin),
    /// `HOME` is unset, so there is no `~/.agents/agents` to write to.
    NoHome,
    Io(String),
}

impl std::fmt::Display for CrudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadId => f.write_str(
                "an agent id is lower-case letters, digits and dashes, at most 64 characters",
            ),
            Self::Invalid(error) => write!(f, "{error}"),
            Self::IdMismatch => f.write_str("the frontmatter `id` must match the agent's id"),
            Self::UnknownTool(tool) => write!(f, "unknown tool: {tool}"),
            Self::NotFound => f.write_str("no agent of yours with that id"),
            Self::NotEditable(AgentOrigin::Project) => f.write_str(
                "this agent belongs to the project (.agents/agents): edit it in the repository",
            ),
            Self::NotEditable(_) => f.write_str(
                "this id is one of fennec's default agents: duplicate it under another id",
            ),
            Self::NoHome => f.write_str("HOME is not set: there is no ~/.agents/agents to write"),
            Self::Io(message) => write!(f, "{message}"),
        }
    }
}

impl CrudError {
    /// The field at fault, so the UI can point at the right input.
    pub fn field(&self) -> &'static str {
        match self {
            Self::BadId | Self::IdMismatch | Self::NotEditable(_) | Self::NotFound => "id",
            Self::Invalid(error) => error.field(),
            Self::UnknownTool(_) => "allowedTools",
            Self::NoHome | Self::Io(_) => "",
        }
    }

    /// The HTTP status the routes answer with.
    pub fn status(&self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::NotEditable(_) => 409,
            Self::NoHome | Self::Io(_) => 500,
            _ => 400,
        }
    }
}

/// The content of the human's `<id>.agent.md`, as it is on disk. Err tells
/// them why a file of theirs may not be picked up by discovery.
pub fn read(id: &str) -> Result<String, CrudError> {
    read_in(&user_home()?, id)
}

/// Writes `content` as the human's `<id>.agent.md`, creating or replacing it.
pub fn save(workdir: &Path, id: &str, content: &str) -> Result<AgentDefinition, CrudError> {
    save_in(&user_home()?, workdir, id, content)
}

/// Deletes the human's `<id>.agent.md`.
pub fn delete(workdir: &Path, id: &str) -> Result<(), CrudError> {
    delete_in(&user_home()?, workdir, id)
}

fn user_home() -> Result<PathBuf, CrudError> {
    home().ok_or(CrudError::NoHome)
}

/// The human's `*.agent.md` files discovery had to skip, with the reason.
/// Discovery stays silent so a bad file never fails a session; the human sees
/// them listed in the settings, where they can fix or delete them.
pub fn unparsable() -> Vec<(String, AgentParseError)> {
    home().map(|home| unparsable_in(&home)).unwrap_or_default()
}

fn unparsable_in(home: &Path) -> Vec<(String, AgentParseError)> {
    agent_files(&user_dir(home))
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let error = parse_agent_md(&content).err()?;
            let name = path.file_name()?.to_string_lossy();
            let id = name.trim_end_matches(".agent.md").to_string();
            Some((id, error))
        })
        .collect()
}

/// An id usable both as an agent id and as a file name: kebab-case, so a
/// request can never reach outside `~/.agents/agents`.
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

fn read_in(home: &Path, id: &str) -> Result<String, CrudError> {
    if !valid_id(id) {
        return Err(CrudError::BadId);
    }
    let path = user_dir(home).join(format!("{id}.agent.md"));
    fs::read_to_string(&path).map_err(|_| CrudError::NotFound)
}

fn save_in(
    home: &Path,
    workdir: &Path,
    id: &str,
    content: &str,
) -> Result<AgentDefinition, CrudError> {
    if !valid_id(id) {
        return Err(CrudError::BadId);
    }

    let mut agent = parse_agent_md(content).map_err(CrudError::Invalid)?;
    if agent.id != id {
        return Err(CrudError::IdMismatch);
    }
    if let Some(unknown) = agent
        .allowed_tools
        .iter()
        // ponytail: MCP tools (`{server}_{tool}`) are not in KNOWN_TOOLS, so a
        // custom agent cannot name one yet; widen with the configured servers
        // when someone asks for it.
        .find(|tool| !crate::domain::agent::KNOWN_TOOLS.contains(&tool.as_str()))
    {
        return Err(CrudError::UnknownTool(unknown.clone()));
    }

    let dir = user_dir(home);
    if let Some(owner) = taken_by_others(&dir, workdir, id) {
        return Err(CrudError::NotEditable(owner));
    }

    fs::create_dir_all(&dir).map_err(|e| CrudError::Io(e.to_string()))?;
    let path = dir.join(format!("{id}.agent.md"));
    fs::write(&path, content).map_err(|e| CrudError::Io(e.to_string()))?;

    agent.origin = AgentOrigin::User;
    agent.path = Some(path.to_string_lossy().into_owned());
    Ok(agent)
}

fn delete_in(home: &Path, workdir: &Path, id: &str) -> Result<(), CrudError> {
    if !valid_id(id) {
        return Err(CrudError::BadId);
    }
    let dir = user_dir(home);
    let path = dir.join(format!("{id}.agent.md"));
    if !path.is_file() {
        return Err(match taken_by_others(&dir, workdir, id) {
            Some(owner) => CrudError::NotEditable(owner),
            None => CrudError::NotFound,
        });
    }
    fs::remove_file(&path).map_err(|e| CrudError::Io(e.to_string()))
}

/// The origin that owns `id` when the human's directory does not: the
/// project's `.agents/agents`, or an embedded default. The project wins, as
/// discovery does.
fn taken_by_others(user_dir: &Path, workdir: &Path, id: &str) -> Option<AgentOrigin> {
    let project_dir = workdir.join(".agents").join("agents");
    if load_agents_directory(&project_dir, AgentOrigin::Project)
        .iter()
        .any(|agent| agent.id == id)
    {
        return Some(AgentOrigin::Project);
    }

    // A default id is taken only while the human has not already overridden
    // it: once `~/.agents/agents/<id>.agent.md` exists, it is theirs to edit.
    let is_default = DEFAULTS
        .iter()
        .any(|content| parse_agent_md(content).is_ok_and(|agent| agent.id == id));
    let overridden = user_dir.join(format!("{id}.agent.md")).is_file();
    if is_default && !overridden {
        return Some(AgentOrigin::Default);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::id::new_id;

    const AGENT: &str = "---\nid: mine\nname: Mine\nallowedTools:\n  - read_file\n---\nMine.\n";

    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("fennec-crud-{}", new_id()));
        let home = root.join("home");
        let workdir = root.join("workdir");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&workdir).unwrap();
        (root, home, workdir)
    }

    fn project_agent(workdir: &Path, id: &str) {
        let dir = workdir.join(".agents").join("agents");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{id}.agent.md")),
            format!("---\nid: {id}\n---\nbody\n"),
        )
        .unwrap();
    }

    #[test]
    fn saves_reads_and_deletes_a_user_agent() {
        let (root, home, workdir) = fixture();

        let saved = save_in(&home, &workdir, "mine", AGENT).unwrap();
        assert_eq!(saved.id, "mine");
        assert_eq!(saved.origin, AgentOrigin::User);
        assert!(saved.path.is_some());

        assert_eq!(read_in(&home, "mine").unwrap(), AGENT);
        // The file is where discovery looks.
        assert!(
            super::super::discover_in(Some(&home), &workdir)
                .iter()
                .any(|agent| agent.id == "mine")
        );

        delete_in(&home, &workdir, "mine").unwrap();
        assert_eq!(read_in(&home, "mine"), Err(CrudError::NotFound));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn saving_twice_replaces_the_file() {
        let (root, home, workdir) = fixture();
        save_in(&home, &workdir, "mine", AGENT).unwrap();
        let changed = "---\nid: mine\nname: Changed\n---\nOther.\n";
        let saved = save_in(&home, &workdir, "mine", changed).unwrap();

        assert_eq!(saved.name, "Changed");
        assert_eq!(read_in(&home, "mine").unwrap(), changed);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_an_id_that_is_not_a_file_name() {
        let (root, home, workdir) = fixture();
        for id in ["", "../escape", "Plan", "a/b", "-lead", "trail-"] {
            assert_eq!(
                save_in(&home, &workdir, id, AGENT),
                Err(CrudError::BadId),
                "{id}"
            );
            assert_eq!(
                delete_in(&home, &workdir, id),
                Err(CrudError::BadId),
                "{id}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_content_that_is_not_an_agent() {
        let (root, home, workdir) = fixture();
        let cases = [
            ("id: mine\n---\nbody\n", AgentParseError::MissingFrontmatter),
            ("---\nname: Mine\n---\nbody\n", AgentParseError::MissingId),
            ("---\nid: mine\n---\n", AgentParseError::EmptyBody),
        ];
        for (content, expected) in cases {
            assert_eq!(
                save_in(&home, &workdir, "mine", content),
                Err(CrudError::Invalid(expected)),
                "{content:?}"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_a_frontmatter_id_that_is_not_the_requested_one() {
        let (root, home, workdir) = fixture();
        assert_eq!(
            save_in(&home, &workdir, "other", AGENT),
            Err(CrudError::IdMismatch)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_an_unknown_tool() {
        let (root, home, workdir) = fixture();
        let content = "---\nid: mine\nallowedTools:\n  - read_file\n  - rm_rf\n---\nMine.\n";
        assert_eq!(
            save_in(&home, &workdir, "mine", content),
            Err(CrudError::UnknownTool("rm_rf".to_string()))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_to_write_or_delete_a_default() {
        let (root, home, workdir) = fixture();
        let content = "---\nid: plan\n---\nMine.\n";
        assert_eq!(
            save_in(&home, &workdir, "plan", content),
            Err(CrudError::NotEditable(AgentOrigin::Default))
        );
        assert_eq!(
            delete_in(&home, &workdir, "plan"),
            Err(CrudError::NotEditable(AgentOrigin::Default))
        );
        assert_eq!(
            CrudError::NotEditable(AgentOrigin::Default).status(),
            409,
            "a default is a conflict, not a bad request"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn refuses_to_write_or_delete_a_project_agent() {
        let (root, home, workdir) = fixture();
        project_agent(&workdir, "theirs");
        let content = "---\nid: theirs\n---\nMine.\n";

        assert_eq!(
            save_in(&home, &workdir, "theirs", content),
            Err(CrudError::NotEditable(AgentOrigin::Project))
        );
        assert_eq!(
            delete_in(&home, &workdir, "theirs"),
            Err(CrudError::NotEditable(AgentOrigin::Project))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deleting_an_unknown_id_is_not_found() {
        let (root, home, workdir) = fixture();
        assert_eq!(
            delete_in(&home, &workdir, "missing"),
            Err(CrudError::NotFound)
        );
        assert_eq!(CrudError::NotFound.status(), 404);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_default_already_overridden_stays_the_humans_to_edit() {
        let (root, home, workdir) = fixture();
        // The human overrode `plan` by hand, outside fennec.
        let dir = user_dir(&home);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("plan.agent.md"),
            "---\nid: plan\nname: Mine\n---\nMine.\n",
        )
        .unwrap();

        let saved = save_in(&home, &workdir, "plan", "---\nid: plan\n---\nAgain.\n").unwrap();
        assert_eq!(saved.origin, AgentOrigin::User);
        delete_in(&home, &workdir, "plan").unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_invalid_file_is_readable_so_the_human_can_fix_it() {
        let (root, home, workdir) = fixture();
        let dir = user_dir(&home);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.agent.md"), "---\nname: no id\n---\nbody\n").unwrap();

        // Discovery skips it, but the human can still open and delete it.
        assert!(
            !super::super::discover_in(Some(&home), &workdir)
                .iter()
                .any(|agent| agent.id == "broken")
        );
        assert!(read_in(&home, "broken").is_ok());
        delete_in(&home, &workdir, "broken").unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }
}
