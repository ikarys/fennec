//! `fennec service`: keeps the server running in the background.

use crate::domain::service::Control;

const USAGE: &str = "usage: fennec service <install|start|stop|restart|status|uninstall>";

pub fn run(args: &[String]) -> Result<(), String> {
    let [action] = args else {
        return Err(USAGE.into());
    };
    let control = match action.as_str() {
        "install" => {
            let logs = crate::service::install()?;
            println!("fennec runs as a service. Logs: {logs}");
            return Ok(());
        }
        "status" => {
            let message = crate::service::status()?.message();
            println!("{message}");
            return Ok(());
        }
        "uninstall" => {
            let removed = crate::service::uninstall()?;
            println!("{removed}");
            return Ok(());
        }
        "start" => Control::Start,
        "stop" => Control::Stop,
        "restart" => Control::Restart,
        _ => return Err(USAGE.into()),
    };
    crate::service::control(control)
}
