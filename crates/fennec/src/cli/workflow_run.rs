//! Terminal client for a workflow run: starts it on the running server, prints its steps and answers its gates from stdin.

use super::transcript::Transcript;
use crate::domain::event::{ChoiceOption, Event};
use crate::domain::workflow::definition::{Node, NodeKind, Workflow};
use crate::domain::workflow::run::RunOutcome;
use serde_json::Value;
use std::io::{BufRead, Write};
use std::time::Duration;

pub fn run(workflow_id: &str, night: bool, task: &str) -> Result<(), String> {
    let port = crate::server_config::port()?;
    let base = format!("http://127.0.0.1:{port}");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(5)))
        .http_status_as_error(false)
        .build()
        .into();

    let workdir = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .into_owned();
    let project = post(
        &agent,
        &base,
        &format!("{base}/api/projects"),
        serde_json::json!({ "workdir": workdir }),
    )?;
    let project_id = project
        .get("id")
        .and_then(Value::as_str)
        .ok_or("project id missing")?
        .to_string();

    let session = post(
        &agent,
        &base,
        &format!("{base}/api/sessions"),
        serde_json::json!({
            "projectId": project_id,
            "prompt": task,
            "workflowId": workflow_id,
        }),
    )?;
    let session = session
        .get("id")
        .and_then(Value::as_str)
        .ok_or("session id missing")?
        .to_string();

    eprintln!("session {session} · {base}/#/s/{session}");

    let mut client = Client {
        agent,
        base,
        session,
        night,
        run_id: None,
        workflow: None,
        transcript: Transcript::default(),
        streamed: false,
    };
    stream(&mut client)
}

struct Client {
    agent: ureq::Agent,
    base: String,
    session: String,
    night: bool,
    run_id: Option<String>,
    workflow: Option<Workflow>,
    transcript: Transcript,
    /// The current node's text already streamed here; a node run in a child
    /// session streams nothing, so its output prints when it finishes.
    streamed: bool,
}

fn stream(client: &mut Client) -> Result<(), String> {
    let url = format!("{}/api/sessions/{}/events", client.base, client.session);
    let response = client
        .agent
        .get(&url)
        .call()
        .map_err(|e| transport_error(&client.base, e))?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let mut reader = std::io::BufReader::new(response.into_body().into_reader());
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("the event stream closed before the workflow finished".into());
        }
        if let Some(data) = line.strip_prefix("data: ")
            && let Ok(event) = serde_json::from_str::<Event>(data.trim_end())
            && client.handle(event)?
        {
            return Ok(());
        }
    }
}

impl Client {
    fn handle(&mut self, event: Event) -> Result<bool, String> {
        self.transcript.print(&event);
        match event {
            Event::MessageDelta { .. } => {
                self.streamed = true;
                Ok(false)
            }
            Event::WorkflowStarted {
                run_id, workflow, ..
            } => {
                self.run_id = Some(run_id);
                self.workflow = Some(workflow);
                Ok(false)
            }
            Event::WorkflowNodeStarted { node_id, .. } => {
                self.streamed = false;
                let label = self.node_label(&node_id);
                println!("\n\x1b[1m▸ {label}\x1b[0m");
                Ok(false)
            }
            Event::WorkflowNodeFinished {
                node_id, output, ..
            } => {
                if !self.streamed && self.is_agent_node(&node_id) {
                    let output = output.trim();
                    if !output.is_empty() {
                        println!("{output}");
                    }
                }
                Ok(false)
            }
            Event::WorkflowHumanAsked {
                node_id, question, ..
            } => self.human_asked(node_id, question),
            Event::ChatAskUser { options, .. } => self.chat_ask_user(options),
            Event::RuleApprovalRequested { .. } | Event::WaiverRequested { .. } => {
                eprintln!(
                    "waiting for your decision in the browser: {}/#/s/{}",
                    self.base, self.session
                );
                Ok(false)
            }
            Event::WorkflowFinished {
                outcome, reason, ..
            } => self.finish(outcome, reason).map(|()| true),
            _ => Ok(false),
        }
    }

    fn human_asked(&mut self, node_id: String, question: String) -> Result<bool, String> {
        let run_id = self.run_id.clone().ok_or("workflow run id missing")?;
        let choices = self.human_choices(&node_id);
        println!("{}", gate_prompt(&question, &choices));

        let (choice, comment) = if self.night {
            let choice = choices
                .first()
                .cloned()
                .ok_or("human gate has no choices")?;
            eprintln!("night: {choice}");
            (choice, String::new())
        } else {
            self.read_human_answer(&choices)?
        };

        post_when_idle(
            &self.agent,
            &self.base,
            &format!(
                "{}/api/sessions/{}/workflow/answer",
                self.base, self.session
            ),
            serde_json::json!({
                "runId": run_id,
                "choice": choice,
                "comment": comment,
            }),
        )?;
        Ok(false)
    }

    fn chat_ask_user(&mut self, options: Option<Vec<ChoiceOption>>) -> Result<bool, String> {
        let answer = if self.night {
            options
                .as_ref()
                .and_then(|o| o.first())
                .map(|o| o.value.clone())
                .unwrap_or_else(|| "Decide yourself and say what you chose.".to_string())
        } else {
            let line = read_line("answer> ")?;
            super::answer_from_line(&line, options.as_deref().unwrap_or_default())
        };

        post_when_idle(
            &self.agent,
            &self.base,
            &format!("{}/api/sessions/{}/messages", self.base, self.session),
            serde_json::json!({ "content": answer }),
        )?;
        Ok(false)
    }

    fn finish(&self, outcome: RunOutcome, reason: Option<String>) -> Result<(), String> {
        let url = format!("{}/api/sessions/{}/verification", self.base, self.session);
        let verification = get_json(&self.agent, &self.base, &url)?;
        let block = verification
            .get("block")
            .and_then(Value::as_str)
            .unwrap_or("no criteria");

        println!("\n{block}");
        println!(
            "Review and integrate: {}/#/s/{}/changes",
            self.base, self.session
        );

        match outcome {
            RunOutcome::Done => Ok(()),
            _ => Err(format!(
                "workflow ended: {outcome:?}{}",
                reason.map(|r| format!(" ({r})")).unwrap_or_default()
            )),
        }
    }

    fn read_human_answer(&self, choices: &[String]) -> Result<(String, String), String> {
        loop {
            let line = read_line("answer> ")?;
            if let Some(answer) = human_answer(&line, choices) {
                return Ok(answer);
            }
            eprintln!("pick a number from 1 to {}", choices.len());
        }
    }

    fn node(&self, node_id: &str) -> Option<&Node> {
        self.workflow
            .as_ref()?
            .nodes
            .iter()
            .find(|n| n.id == node_id)
    }

    fn node_label(&self, node_id: &str) -> String {
        self.node(node_id)
            .and_then(|n| n.name.clone())
            .unwrap_or_else(|| node_id.to_string())
    }

    fn is_agent_node(&self, node_id: &str) -> bool {
        self.node(node_id)
            .is_some_and(|n| matches!(&n.kind, NodeKind::Agent { .. }))
    }

    fn human_choices(&self, node_id: &str) -> Vec<String> {
        self.node(node_id)
            .and_then(|n| match &n.kind {
                NodeKind::Human { choices, .. } => Some(choices.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}

fn post(agent: &ureq::Agent, base: &str, url: &str, body: Value) -> Result<Value, String> {
    let response = agent
        .post(url)
        .send_json(body)
        .map_err(|e| transport_error(base, e))?;
    status_value(response)
}

/// The server records the job's last commit after its last event: an answer
/// sent right on the question can find the session still busy, so it waits.
fn post_when_idle(
    agent: &ureq::Agent,
    base: &str,
    url: &str,
    body: Value,
) -> Result<Value, String> {
    const BUSY: &str = "a turn is already running";
    for _ in 0..60 {
        match post(agent, base, url, body.clone()) {
            Err(e) if e == BUSY => std::thread::sleep(Duration::from_millis(500)),
            other => return other,
        }
    }
    Err(BUSY.into())
}

fn get_json(agent: &ureq::Agent, base: &str, url: &str) -> Result<Value, String> {
    let response = agent
        .get(url)
        .call()
        .map_err(|e| transport_error(base, e))?;
    status_value(response)
}

fn status_value(mut response: ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let status = response.status();
    let value = response
        .body_mut()
        .read_json::<Value>()
        .unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(value);
    }
    Err(value
        .get("error")
        .and_then(Value::as_str)
        .map_or_else(|| format!("HTTP {status}"), str::to_string))
}

fn transport_error(base: &str, error: ureq::Error) -> String {
    format!(
        "no fennec server on {base} ({error}): start it with `fennec` or `fennec service start`"
    )
}

fn read_line(prompt: &str) -> Result<String, String> {
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    let n = std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("stdin closed while the workflow waits for a choice".into());
    }
    Ok(line)
}

fn gate_prompt(question: &str, choices: &[String]) -> String {
    let mut out = question.to_string();
    out.push('\n');
    for (i, choice) in choices.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        out.push_str(&format!("[{}] {choice}", i + 1));
    }
    out
}

fn human_answer(line: &str, choices: &[String]) -> Option<(String, String)> {
    let trimmed = line.trim();
    let (token, comment) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    let choice = match token.parse::<usize>() {
        Ok(n) => choices.get(n.checked_sub(1)?)?,
        Err(_) => choices.iter().find(|c| c.eq_ignore_ascii_case(token))?,
    };
    Some((choice.clone(), comment.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_prompt_numbers_choices() {
        let choices = vec!["Go".to_string(), "Revise".to_string()];
        assert_eq!(
            gate_prompt("Go with this plan?", &choices),
            "Go with this plan?\n[1] Go  [2] Revise"
        );
    }

    #[test]
    fn human_answer_parses_numbers_and_choice_words() {
        let choices = vec!["Go".to_string(), "Revise".to_string()];
        assert_eq!(
            human_answer("1", &choices),
            Some(("Go".to_string(), String::new()))
        );
        assert_eq!(
            human_answer("2 make it smaller", &choices),
            Some(("Revise".to_string(), "make it smaller".to_string()))
        );
        assert_eq!(
            human_answer("revise split step 2", &choices),
            Some(("Revise".to_string(), "split step 2".to_string()))
        );
        assert_eq!(human_answer("3", &choices), None);
        assert_eq!(human_answer("maybe", &choices), None);
        assert_eq!(human_answer("", &choices), None);
    }
}
