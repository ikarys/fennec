//! The background service `fennec service` manages: the systemd unit, the
//! launchd agent and the service manager's arguments. No I/O.

pub const SYSTEMD_UNIT: &str = "fennec";
pub const LAUNCHD_LABEL: &str = "dev.fennec";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Start,
    Stop,
    Restart,
}

impl Control {
    fn verb(self) -> &'static str {
        match self {
            Control::Start => "start",
            Control::Stop => "stop",
            Control::Restart => "restart",
        }
    }
}

/// The paths come from the environment: a newline in one would add directives
/// to a file the service manager runs at every login.
fn refuse_control_chars(value: &str) -> Result<(), String> {
    if value.chars().any(char::is_control) {
        return Err(format!(
            "control character in {value:?}: refused in a service file"
        ));
    }
    Ok(())
}

fn systemd_word(value: &str) -> Result<String, String> {
    refuse_control_chars(value)?;
    // `$` expands in ExecStart but not in Environment: no one escaping fits both.
    if value.contains('$') {
        return Err(format!("`$` in {value:?}: refused in a systemd unit"));
    }
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    Ok(format!("\"{escaped}\""))
}

fn xml_text(value: &str) -> Result<String, String> {
    refuse_control_chars(value)?;
    Ok(value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;"))
}

/// A service starts with a bare PATH: `path` keeps the human's, so the agent's
/// commands find the same tools (git, cargo, node) as in their shell.
pub fn systemd_unit(exe: &str, path: &str) -> Result<String, String> {
    let exec = systemd_word(exe)?;
    let environment = systemd_word(&format!("PATH={path}"))?;
    Ok(format!(
        "[Unit]\nDescription=fennec\n\n\
         [Service]\nExecStart={exec} serve\nEnvironment={environment}\nRestart=on-failure\n\n\
         [Install]\nWantedBy=default.target\n"
    ))
}

pub fn launchd_plist(exe: &str, path: &str, log: &str) -> Result<String, String> {
    let (exe, path, log) = (xml_text(exe)?, xml_text(path)?, xml_text(log)?);
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCHD_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>serve</string>
    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path}</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#
    ))
}

pub fn systemctl_args(control: Control) -> Vec<String> {
    vec!["--user".into(), control.verb().into(), SYSTEMD_UNIT.into()]
}

/// launchd has no plain start or stop for an agent: it is loaded or not.
/// `None`: nothing to do, the agent is already stopped.
pub fn launchctl_args(
    control: Control,
    loaded: bool,
    uid: &str,
    plist: &str,
) -> Option<Vec<String>> {
    let domain = format!("gui/{uid}");
    let target = format!("{domain}/{LAUNCHD_LABEL}");
    let args: Vec<String> = match (control, loaded) {
        (Control::Stop, false) => return None,
        (Control::Stop, true) => vec!["bootout".into(), domain, plist.into()],
        (Control::Start | Control::Restart, false) => {
            vec!["bootstrap".into(), domain, plist.into()]
        }
        (Control::Start, true) => vec!["kickstart".into(), target],
        (Control::Restart, true) => vec!["kickstart".into(), "-k".into(), target],
    };
    Some(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unit_runs_the_binary_with_the_path_of_the_shell() {
        let unit = systemd_unit("/home/me/.local/bin/fennec", "/usr/bin:/opt/x").unwrap();
        assert!(unit.contains("ExecStart=\"/home/me/.local/bin/fennec\" serve\n"));
        assert!(unit.contains("Environment=\"PATH=/usr/bin:/opt/x\"\n"));
    }

    #[test]
    fn a_systemd_path_is_quoted_and_its_specifiers_escaped() {
        let unit = systemd_unit("/a b/100%/\"x\"/fennec", "/bin").unwrap();
        assert!(unit.contains(r#"ExecStart="/a b/100%%/\"x\"/fennec" serve"#));
    }

    #[test]
    fn a_path_cannot_add_a_directive_to_a_service_file() {
        let hostile = "/bin\nExecStartPre=/tmp/evil";
        assert!(systemd_unit(hostile, "/bin").is_err());
        assert!(systemd_unit("/bin/fennec", hostile).is_err());
        assert!(launchd_plist(hostile, "/bin", "/log").is_err());
        assert!(systemd_unit("/$HOME/fennec", "/bin").is_err());
    }

    #[test]
    fn a_plist_path_cannot_close_its_element() {
        let plist = launchd_plist("/a</string>&/fennec", "/bin", "/log").unwrap();
        assert!(plist.contains("<string>/a&lt;/string&gt;&amp;/fennec</string>"));
    }

    #[test]
    fn systemd_controls_the_user_unit() {
        assert_eq!(
            systemctl_args(Control::Restart),
            ["--user", "restart", "fennec"]
        );
    }

    #[test]
    fn launchd_loads_an_agent_that_is_not_loaded() {
        for control in [Control::Start, Control::Restart] {
            assert_eq!(
                launchctl_args(control, false, "501", "/p").unwrap(),
                ["bootstrap", "gui/501", "/p"]
            );
        }
    }

    #[test]
    fn launchd_kicks_a_loaded_agent_and_kills_it_only_to_restart() {
        assert_eq!(
            launchctl_args(Control::Start, true, "501", "/p").unwrap(),
            ["kickstart", "gui/501/dev.fennec"]
        );
        assert_eq!(
            launchctl_args(Control::Restart, true, "501", "/p").unwrap(),
            ["kickstart", "-k", "gui/501/dev.fennec"]
        );
    }

    #[test]
    fn stopping_launchd_unloads_and_is_idempotent() {
        assert_eq!(
            launchctl_args(Control::Stop, true, "501", "/p").unwrap(),
            ["bootout", "gui/501", "/p"]
        );
        assert_eq!(launchctl_args(Control::Stop, false, "501", "/p"), None);
    }
}
