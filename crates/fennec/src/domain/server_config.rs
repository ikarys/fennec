//! The server's settings file (`settings.yaml`): its port, listen address and
//! password hash, read and written back. No I/O here.

use super::frontmatter;
use std::net::IpAddr;

pub const DEFAULT_PORT: u16 = 7777;
pub const DEFAULT_LISTEN: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));

/// The port the file sets; None when it sets none. Err names the bad value.
pub fn port(content: &str) -> Result<Option<u16>, String> {
    let fields = frontmatter::parse(content);
    let value = match fields.scalar("port") {
        Some(value) => value,
        None => return Ok(None),
    };

    let value = value.trim();
    let number = value
        .parse::<u64>()
        .map_err(|_| format!("port: {value:?} is not a number"))?;
    valid_port(number).map(Some)
}

/// The listen address the file sets; None when it sets none. Err names the bad value.
pub fn listen(content: &str) -> Result<Option<IpAddr>, String> {
    let fields = frontmatter::parse(content);
    let Some(value) = fields.scalar("listen") else {
        return Ok(None);
    };

    let value = value.trim();
    value
        .parse::<IpAddr>()
        .map(Some)
        .map_err(|_| format!("listen: {value:?} is not an IP address"))
}

/// The stored password hash; None when absent. Err names a malformed value.
pub fn password(content: &str) -> Result<Option<String>, String> {
    let fields = frontmatter::parse(content);
    let Some(value) = fields.scalar("password") else {
        return Ok(None);
    };

    let value = value.trim();
    if super::password::is_valid(value) {
        Ok(Some(value.to_string()))
    } else {
        Err(format!("password: {value:?} is not a valid hash"))
    }
}

/// Listening beyond the loopback needs a password: without one, fennec refuses to listen.
pub fn check_listen(listen: IpAddr, password_set: bool) -> Result<(), String> {
    if listen.is_loopback() || password_set {
        Ok(())
    } else {
        Err(format!(
            "listen {listen} is not a loopback address; set a password first with `fennec password`"
        ))
    }
}

/// A port the human may choose: 1024..=65535 (below 1024 needs root).
pub fn valid_port(port: u64) -> Result<u16, String> {
    if !(1024..=65535).contains(&port) {
        return Err("port must be 1024..=65535".to_owned());
    }
    Ok(port as u16)
}

/// `content` with `key` set to `value`: a top-level `key:` line is replaced in place (other lines
/// and comments kept byte for byte), else `key: <value>` is appended (with a newline before it if
/// `content` does not end with one). An empty (or whitespace-only) content becomes a short header
/// comment `# fennec's server settings. The --port flag wins over this file.\n` followed by
/// `key: <value>\n`.
pub fn with_field(content: &str, key: &str, value: &str) -> String {
    let field_line = format!("{key}: {value}");

    // An empty file has no previous bytes worth preserving; give it a useful header.
    if content.trim().is_empty() {
        return format!(
            "# fennec's server settings. The --port flag wins over this file.\n{field_line}\n"
        );
    }

    let mut out = String::new();
    let mut replaced = false;
    let prefix = format!("{key}:");

    // split_inclusive keeps each line's terminator, so untouched lines survive byte for byte.
    for line in content.split_inclusive('\n') {
        if !replaced && line.starts_with(&prefix) {
            replaced = true;
            let ending = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str(&format!("{key}: {value}{ending}"));
        } else {
            out.push_str(line);
        }
    }

    if !replaced {
        if !content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("{field_line}\n"));
    }

    out
}

pub fn with_port(content: &str, port: u16) -> String {
    with_field(content, "port", &port.to_string())
}

/// `content` with `listen` set to `ip`: a thin wrapper over `with_field`.
pub fn with_listen(content: &str, ip: IpAddr) -> String {
    with_field(content, "listen", &ip.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listening_beyond_the_loopback_needs_a_password() {
        let anywhere: IpAddr = "0.0.0.0".parse().unwrap();
        let lan: IpAddr = "192.168.1.10".parse().unwrap();
        let loopback6: IpAddr = "::1".parse().unwrap();
        assert!(check_listen(anywhere, false).is_err());
        assert!(check_listen(lan, false).is_err());
        assert!(check_listen(anywhere, true).is_ok());
        assert!(check_listen(DEFAULT_LISTEN, false).is_ok());
        assert!(check_listen(loopback6, false).is_ok());
    }

    #[test]
    fn absent_port_is_none() {
        assert_eq!(port(""), Ok(None));
    }

    #[test]
    fn reads_a_port() {
        assert_eq!(port("port: 8080\n"), Ok(Some(8080)));
    }

    #[test]
    fn a_non_number_port_is_an_error() {
        assert!(port("port: abc\n").is_err());
    }

    #[test]
    fn a_low_port_is_an_error() {
        assert!(port("port: 80\n").is_err());
    }

    #[test]
    fn with_port_replaces_in_place() {
        let content = "# comment\nport: 1234\nother: x\n";
        assert_eq!(
            with_port(content, 5678),
            "# comment\nport: 5678\nother: x\n"
        );
    }

    #[test]
    fn with_port_appends_without_trailing_newline() {
        assert_eq!(with_port("other: x", 9000), "other: x\nport: 9000\n");
    }

    #[test]
    fn with_port_uses_a_header_for_empty_content() {
        assert_eq!(
            with_port("", 8080),
            "# fennec's server settings. The --port flag wins over this file.\nport: 8080\n"
        );
    }

    #[test]
    fn with_port_output_reads_back() {
        let content = with_port("port: 8080\n", 9000);
        assert_eq!(port(&content), Ok(Some(9000)));
    }

    #[test]
    fn with_listen_replaces_in_place() {
        let content = "# comment\nlisten: 127.0.0.1\nother: x\n";
        assert_eq!(
            with_listen(content, IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
            "# comment\nlisten: 0.0.0.0\nother: x\n"
        );
    }
}
