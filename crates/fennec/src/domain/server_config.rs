//! The server's settings file (`settings.yaml`): its port, read and written back. No I/O here.

use super::frontmatter;

pub const DEFAULT_PORT: u16 = 7777;

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

/// A port the human may choose: 1024..=65535 (below 1024 needs root).
pub fn valid_port(port: u64) -> Result<u16, String> {
    if !(1024..=65535).contains(&port) {
        return Err("port must be 1024..=65535".to_owned());
    }
    Ok(port as u16)
}

/// `content` with its port set to `port`: a top-level `port:` line is replaced in place (other lines
/// and comments kept byte for byte), else `port: <n>` is appended (with a newline before it if
/// `content` does not end with one). An empty (or whitespace-only) content becomes a short header
/// comment `# fennec's server settings. The --port flag wins over this file.\n` followed by
/// `port: <n>\n`.
pub fn with_port(content: &str, port: u16) -> String {
    let port_line = format!("port: {port}");

    // An empty file has no previous bytes worth preserving; give it a useful header.
    if content.trim().is_empty() {
        return format!(
            "# fennec's server settings. The --port flag wins over this file.\n{port_line}\n"
        );
    }

    let mut out = String::new();
    let mut replaced = false;

    // split_inclusive keeps each line's terminator, so untouched lines survive byte for byte.
    for line in content.split_inclusive('\n') {
        if !replaced && line.starts_with("port:") {
            replaced = true;
            let ending = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str(&format!("port: {port}{ending}"));
        } else {
            out.push_str(line);
        }
    }

    if !replaced {
        if !content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("{port_line}\n"));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
