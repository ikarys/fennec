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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Stopped,
    NotInstalled,
}

impl Status {
    pub fn of(installed: bool, running: bool) -> Status {
        if !installed {
            Status::NotInstalled
        } else if running {
            Status::Running
        } else {
            Status::Stopped
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Status::Running => "fennec service is running",
            Status::Stopped => "fennec service is installed but not running",
            Status::NotInstalled => "fennec service is not installed: run `fennec service install`",
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

/// mise installs each version in its own folder and prunes the old ones: a
/// service tied to one would stop starting after an upgrade. `latest` is the
/// link mise keeps up to date; whether it is this binary is the adapter's check.
pub fn mise_latest(exe: &str) -> Option<String> {
    let (root, rest) = exe.rsplit_once("/installs/")?;
    let mut parts = rest.splitn(3, '/');
    let (tool, _version, file) = (parts.next()?, parts.next()?, parts.next()?);
    Some(format!("{root}/installs/{tool}/latest/{file}"))
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
    fn a_mise_install_is_reached_through_its_latest_link() {
        assert_eq!(
            mise_latest("/h/.local/share/mise/installs/github-x-fennec/0.2.2/fennec").unwrap(),
            "/h/.local/share/mise/installs/github-x-fennec/latest/fennec"
        );
        assert_eq!(mise_latest("/home/me/.local/bin/fennec"), None);
        assert_eq!(mise_latest("/installs/fennec"), None);
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

    #[test]
    fn status_of_all_combinations() {
        assert_eq!(Status::of(false, false), Status::NotInstalled);
        assert_eq!(Status::of(false, true), Status::NotInstalled);
        assert_eq!(Status::of(true, false), Status::Stopped);
        assert_eq!(Status::of(true, true), Status::Running);
    }
}
