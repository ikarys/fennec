//! The server's settings file on disk: `$XDG_CONFIG_HOME/fennec/settings.yaml` (default `~/.config/...`).

use std::net::IpAddr;
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

/// The file's content; an absent file reads as empty.
fn read() -> Result<String, String> {
    let path = path();
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

fn located<T>(result: Result<T, String>) -> Result<T, String> {
    result.map_err(|message| format!("{}: {message}", path().display()))
}

/// The configured port: the file's, else the default; a bad value is an error.
pub fn port() -> Result<u16, String> {
    Ok(located(domain::port(&read()?))?.unwrap_or(domain::DEFAULT_PORT))
}

/// The configured listen address: the file's, else loopback; a bad value is an error.
pub fn listen() -> Result<IpAddr, String> {
    Ok(located(domain::listen(&read()?))?.unwrap_or(domain::DEFAULT_LISTEN))
}

/// The stored password hash, if any; a malformed value is an error.
pub fn password() -> Result<Option<String>, String> {
    located(domain::password(&read()?))
}

/// Writes `port` into the file, its other lines kept.
pub fn set_port(port: u16) -> Result<(), String> {
    write(&domain::with_port(&read()?, port))
}

/// Writes the password hash into the file, its other lines kept.
pub fn set_password(hash: &str) -> Result<(), String> {
    write(&domain::with_field(&read()?, "password", hash))
}

/// Renamed into place, never half written; owner-only, since it may hold the password hash.
fn write(content: &str) -> Result<(), String> {
    let path = path();
    let parent = path
        .parent()
        .ok_or_else(|| format!("{}: no parent directory", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    let temp = parent.join(format!(
        "settings.yaml.{}.{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::write(&temp, content).map_err(|err| format!("{}: {err}", temp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))
            .map_err(|err| format!("{}: {err}", temp.display()))?;
    }
    if let Err(err) = std::fs::rename(&temp, &path) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("{}: {err}", path.display()));
    }
    Ok(())
}
