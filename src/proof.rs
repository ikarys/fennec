//! Proof: before a run is called ready, the product is used the way its user
//! would use it, not through its tests. The model only proposes how to try
//! it; the harness runs the tries in the winner's world and keeps their real
//! output. A try exits 0 when the product behaves as the goal says, so the
//! verdict is still read from exit codes. What the tries cannot show is
//! written down as unverified, never left out.

use std::io;

use serde_json::{Value, json};

use crate::provider::Model;
use crate::review;
use crate::rules::{Kind, Rule};
use crate::solve;
use crate::world::Worlds;

/// More tries make the proof longer to read, not stronger.
const MAX_TRIES: usize = 5;

const BRIEF: &str = "A change was made to a project and its tests pass. Show that it works \
the way its user would see it: run the program or command, call the new code from a short \
script, start the server and query it. Do not run the test suite. The commands run from the \
project root in a sandbox without network; anything you start must stop on its own (use \
`timeout`). Each command must print what a user would look at, and exit 0 only if the \
product behaves as the goal says (catch an expected error and print it). \
Reply in exactly this form, nothing else:\n\
try: <one shell command>   (one line per command, at most 5)\n\
unverified: <what these commands cannot show, e.g. a graphical interface>   (one line per item, \
omit if none)";

/// Gathers the proof for the run `id` (its winner's world) and saves it in
/// the run record. Returns `{"tries": [...], "unverified": [...], "passed": bool}`.
pub fn gather(worlds: &Worlds, id: &str, model: &dyn Model) -> io::Result<Value> {
    let mut record = review::load(worlds, id)?;
    let world = worlds.open(id)?;
    let diff = record["attempts"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["world"] == id))
        .and_then(|t| t["diff"].as_str())
        .unwrap_or("");
    let request = format!(
        "Goal:\n{}\n\nChange:\n{diff}",
        record["goal"].as_str().unwrap_or("")
    );
    let messages = [
        json!({"role": "system", "content": BRIEF}),
        json!({"role": "user", "content": request}),
    ];
    let reply = model.chat(&messages, &json!([]))?;
    let content = reply["content"].as_str().unwrap_or("");
    // Some servers leave the reasoning inline.
    let (tries, mut unverified) = parse(content.rsplit("</think>").next().unwrap_or(content));

    let rules: Vec<Rule> = tries
        .iter()
        .map(|c| Rule {
            id: "proof.try".to_string(),
            kind: Kind::StandingCommand(c.clone()),
            block: true,
        })
        .collect();
    // A try may write files: the world is rolled back below rather than the
    // proof failed, so the second half of verify's answer is not used.
    let (checks, _) = solve::verify(worlds, &world, &rules)?;
    // What lands must stay what the tests checked.
    worlds.rollback(&world)?;
    if tries.is_empty() {
        unverified.push("aucun essai proposé".to_string());
    }
    let passed = !tries.is_empty() && checks.iter().all(|c| c["exit"] == 0);
    let proof = json!({"tries": checks, "unverified": unverified, "passed": passed});
    record["proof"] = proof.clone();
    review::save(worlds, id, &record)?;
    Ok(proof)
}

fn parse(reply: &str) -> (Vec<String>, Vec<String>) {
    let (mut tries, mut unverified) = (Vec::new(), Vec::new());
    for line in reply.lines() {
        let line = line.trim().trim_start_matches("- ");
        let clean = |s: &str| s.trim().trim_matches('`').trim().to_string();
        if let Some(cmd) = line.strip_prefix("try:") {
            let cmd = clean(cmd);
            if !cmd.is_empty() && tries.len() < MAX_TRIES {
                tries.push(cmd);
            }
        } else if let Some(what) = line.strip_prefix("unverified:") {
            let what = clean(what);
            if !what.is_empty() && !what.eq_ignore_ascii_case("none") {
                unverified.push(what);
            }
        }
    }
    (tries, unverified)
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reply_parsing() {
        let (tries, unverified) = parse(
            "Sure.\ntry: `python3 -c 'print(1)'`\n- try: ./run.sh\nunverified: the GUI\nunverified: none\n",
        );
        assert_eq!(tries, ["python3 -c 'print(1)'", "./run.sh"]);
        assert_eq!(unverified, ["the GUI"]);
        let many = "try: a\n".repeat(9);
        assert_eq!(parse(&many).0.len(), 5);
    }
}
