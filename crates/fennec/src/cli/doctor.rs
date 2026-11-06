//! `fennec doctor`: the first-run checklist in the terminal, the same checks the home page shows.

use crate::config::Settings;
use crate::domain::setup::{CheckId, CheckStatus};
use crate::store::SqliteStore;

pub fn run(settings: &Settings, store: &SqliteStore) -> Result<(), String> {
    let checks = crate::domain::setup::checks(&crate::setup::facts(settings, store)?);
    for check in &checks {
        let line = format!(
            "{:<5} {:<9} {}",
            status_word(&check.status),
            id_word(&check.id),
            check.detail.as_deref().unwrap_or_default()
        );
        println!("{}", line.trim_end());
    }

    if crate::domain::setup::complete(&checks) {
        println!("ready");
        Ok(())
    } else {
        Err("setup incomplete: fix the todo lines above".into())
    }
}

fn status_word(status: &CheckStatus) -> &'static str {
    match status {
        CheckStatus::Ok => "ok",
        CheckStatus::Todo => "todo",
        CheckStatus::Warn => "warn",
    }
}

fn id_word(id: &CheckId) -> &'static str {
    match id {
        CheckId::Model => "model",
        CheckId::Sandbox => "sandbox",
        CheckId::Git => "git",
        CheckId::Project => "project",
        CheckId::Scanners => "scanners",
    }
}
