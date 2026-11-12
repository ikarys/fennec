//! `fennec password`: the remote-access password, read without echo and stored hashed.

use crate::server_config;
use std::io::{BufRead, IsTerminal, Write};

/// `fennec password`: asks for the remote-access password twice and stores its hash.
pub fn run() -> Result<(), String> {
    let hash = ask_new_hash()?;
    server_config::set_password(&hash)?;

    println!("wrote password to {}", server_config::path().display());
    println!("open fennec to the network with: fennec remote enable");
    Ok(())
}

/// Asks for a new password twice and returns its hash.
pub fn ask_new_hash() -> Result<String, String> {
    let first = read_password_line("New password: ")?;
    let second = read_password_line("Again: ")?;
    if first != second {
        return Err("passwords do not match".into());
    }

    server_config::new_hash(&first)
}

fn read_password_line(prompt: &str) -> Result<String, String> {
    eprint!("{prompt}");
    std::io::stderr().flush().map_err(|e| e.to_string())?;

    let stdin = std::io::stdin();
    let interactive = stdin.is_terminal();
    if interactive {
        let _ = std::process::Command::new("stty").arg("-echo").status();
    }

    let mut line = String::new();
    let read = stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;

    if interactive {
        let _ = std::process::Command::new("stty").arg("echo").status();
        // The newline the user typed was not echoed either.
        eprintln!();
    }

    if read == 0 {
        return Err("no password entered".into());
    }
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}
