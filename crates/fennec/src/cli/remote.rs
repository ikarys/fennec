//! `fennec remote`: opens or closes the server to the network, for a machine without a screen.

use crate::domain::server_config::{DEFAULT_LISTEN, remote_report};
use crate::server_config;
use std::io::IsTerminal;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};

const USAGE: &str = "usage: fennec remote <enable|disable|status>";
/// The fennec emblem, drawn above the report.
const HEADER: &str = include_str!("remote_banner.txt");

/// `fennec remote <enable|disable|status>`: enable asks for a password first when none is set.
pub fn run(args: &[String]) -> Result<(), String> {
    let [action] = args else {
        return Err(USAGE.into());
    };
    let changed = match action.as_str() {
        "enable" => {
            if server_config::password()?.is_none() {
                server_config::set_password(&crate::cli::password::ask_new_hash()?)?;
            }
            server_config::set_listen(IpAddr::V4(Ipv4Addr::UNSPECIFIED))?;
            true
        }
        "disable" => {
            server_config::set_listen(DEFAULT_LISTEN)?;
            true
        }
        "status" => false,
        _ => return Err(USAGE.into()),
    };
    let report = remote_report(
        server_config::listen()?,
        server_config::port()?,
        server_config::password()?.is_some(),
        lan_address(),
        changed,
    );
    if std::io::stdout().is_terminal() {
        print!("\x1b[38;5;208m{HEADER}\x1b[0m");
    } else {
        print!("{HEADER}");
    }
    println!();
    print!("{report}");
    Ok(())
}

/// The address other machines reach this one by: the source address of the default route.
/// Connecting a UDP socket only consults the routing table; no packet is sent.
fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let local = socket.local_addr().ok()?.ip();
    if local.is_unspecified() || local.is_loopback() {
        None
    } else {
        Some(local)
    }
}
