//! The server's settings file on disk: `$XDG_CONFIG_HOME/fennec/settings.yaml` (default `~/.config/...`).

use std::path::PathBuf;

use crate::domain::server_config as domain;

/// Where the file lives; it need not exist.
pub fn path() -> PathBuf {
    // XDG is the first-class location; HOME and "." are fallbacks for minimal environments.
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".config"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    base.join("fennec").join("settings.yaml")
}

/// The configured port: the file's, else the default; a bad value is an error.
pub fn port() -> Result<u16, String> {
    let path = path();
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(domain::DEFAULT_PORT),
        Err(err) => return Err(format!("{}: {err}", path.display())),
    };

    match domain::port(&content) {
        Ok(Some(port)) => Ok(port),
        Ok(None) => Ok(domain::DEFAULT_PORT),
        Err(message) => Err(format!("{}: {message}", path.display())),
    }
}

/// Writes `port` into the file, its other lines kept; renamed into place, never half written.
pub fn set_port(port: u16) -> Result<(), String> {
    let path = path();
    let parent = path
        .parent()
        .ok_or_else(|| format!("{}: no parent directory", path.display()))?;

    std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;

    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(format!("{}: {err}", path.display())),
    };

    let updated = domain::with_port(&content, port);
    let temp = parent.join(format!(
        "settings.yaml.{}.{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));

    std::fs::write(&temp, updated).map_err(|err| format!("{}: {err}", temp.display()))?;

    if let Err(err) = std::fs::rename(&temp, &path) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("{}: {err}", path.display()));
    }

    Ok(())
}
