use std::process::ExitCode;

use fennec_os::provider::{Model, OpenAi};
use fennec_os::solve::{Task, land, solve};
use fennec_os::world::Worlds;
use fennec_os::{grant, review, rules, serve, sleep, spec};

const USAGE: &str = "usage: fennec-os <command>
  fork <project-path|world-id>   new world from the source's last commit
  run <world-id> -- <cmd>...     run a command inside the world's sandbox
  diff <world-id>                changes since the last checkpoint
  commit <world-id> <message>    checkpoint the world
  rollback <world-id>            back to the last checkpoint
  drop <world-id>                delete the world and its branch
  list                           list worlds
  solve <project> <task.json> [--attempts N] [--steps N]
                                 up to N agents, one world each, in turn until one passes
  spec <project> <request> [--steps N]
                                 compile a request into acceptance tests + a task
  spec list | show <id>          specs waiting on you, one spec's goal and tests
  spec accept <id> [--attempts N] [--steps N]
                                 validate the spec and solve it from its tests
  spec reject <id> [correction]  discard it; the correction goes to the next compile
  land <world-id>                fast-forward the project onto a world that passed
  queue                          runs waiting on you
  show <run-id>                  a run's verdicts and winning diff
  profile <run-id|spec-id>       where each attempt's time went, turn by turn
  reject <run-id> [correction]   discard a run; corrections said twice become rules
  promote <project> <rule-json> [--origin <run-id>]
                                 add a rule to <project>/.fennec-os/rules.json
  sleep                          distil landed runs into candidate know-how
  spar <project> [--steps N]     hunt one defect, proven by a failing test; a spec to validate
  notes                          candidates waiting on you
  keep <k-id> | dismiss <k-id>   add a candidate to <project>/.fennec-os/knowhow.md, or not
  serve [--port N]               the review page on 127.0.0.1 (specs, runs, know-how)

task.json: {\"goal\": \"...\", \"verify\": [\"cmd\"], \"protect\": [\"path\"]}
model env: FENNEC_OS_BASE_URL, FENNEC_OS_MODELS (comma-separated), FENNEC_OS_API_KEY";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("fennec-os: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> std::io::Result<ExitCode> {
    let worlds = Worlds::from_env()?;
    let arg = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    if let (true, Ok(Some(reminder))) = (arg(0) != "grant", grant::summary(&worlds)) {
        eprintln!("fennec-os: {reminder}");
    }
    match arg(0) {
        "fork" if args.len() == 2 => println!("{}", worlds.fork(arg(1))?.id),
        "run" if args.len() > 3 && arg(2) == "--" => {
            let world = worlds.open(arg(1))?;
            let status = worlds.sandbox_command(&world, &args[3..])?.status()?;
            return Ok(ExitCode::from(status.code().unwrap_or(1) as u8));
        }
        "diff" if args.len() == 2 => println!("{}", worlds.diff(&worlds.open(arg(1))?)?),
        "commit" if args.len() == 3 => {
            println!("{}", worlds.commit(&worlds.open(arg(1))?, arg(2))?)
        }
        "rollback" if args.len() == 2 => worlds.rollback(&worlds.open(arg(1))?)?,
        "drop" if args.len() == 2 => worlds.drop(worlds.open(arg(1))?)?,
        "grant" if args.len() >= 3 && arg(1) == "give" => {
            let paths: Vec<_> = args[3..].iter().map(std::path::PathBuf::from).collect();
            grant::give(&worlds, &worlds.open(arg(2))?, &paths)?
        }
        "grant" if args.len() == 2 && arg(1) == "list" => print!("{}", grant::list(&worlds)?),
        "grant" if args.len() == 2 && arg(1) == "remind" => grant::remind(&worlds)?,
        "grant" if args.len() == 3 && arg(1) == "diff" => {
            print!("{}", grant::diff(&worlds, &worlds.open(arg(2))?)?)
        }
        "grant" if args.len() == 3 && arg(1) == "keep" => {
            grant::keep(&worlds, &worlds.open(arg(2))?)?
        }
        "grant" if args.len() == 3 && arg(1) == "revert" => {
            grant::revert(&worlds, &worlds.open(arg(2))?)?
        }
        "list" if args.len() == 1 => worlds.list()?.iter().for_each(|id| println!("{id}")),
        "solve" if args.len() >= 3 => {
            let mut task = Task::from_json(&std::fs::read_to_string(arg(2))?)?;
            task.grant = args
                .windows(2)
                .filter(|w| w[0] == "--grant")
                .map(|w| w[1].clone().into())
                .collect();
            let variants = OpenAi::variants_from_env(flag(args, "--attempts", 1)?.max(1))?;
            let models: Vec<&dyn Model> = variants.iter().map(|m| m as &dyn Model).collect();
            let record = solve(
                &worlds,
                std::path::Path::new(arg(1)),
                &task,
                &models,
                flag(args, "--steps", 40)?,
            )?;
            return Ok(print_run(&worlds, &record));
        }
        "spec" if args.len() >= 3 && arg(1) == "accept" => {
            let variants = OpenAi::variants_from_env(flag(args, "--attempts", 1)?.max(1))?;
            let models: Vec<&dyn Model> = variants.iter().map(|m| m as &dyn Model).collect();
            let record = spec::accept(&worlds, arg(2), &models, flag(args, "--steps", 40)?)?;
            return Ok(print_run(&worlds, &record));
        }
        "spec" if (args.len() == 3 || args.len() == 4) && arg(1) == "reject" => {
            spec::reject(&worlds, arg(2), args.get(3).map(String::as_str))?
        }
        "spec" if args.len() == 3 && arg(1) == "show" => {
            let s = spec::load(&worlds, arg(2))?;
            println!(
                "request: {}
status: {}  {}

goal: {}",
                s["request"].as_str().unwrap_or(""),
                s["status"].as_str().unwrap_or("?"),
                s["reason"].as_str().unwrap_or(""),
                s["goal"].as_str().unwrap_or("")
            );
            for v in s["verify"].as_array().into_iter().flatten() {
                println!("verify: {}", v.as_str().unwrap_or(""));
            }
            println!("\n{}", s["diff"].as_str().unwrap_or(""));
        }
        "spec" if args.len() == 2 && arg(1) == "list" => {
            for (id, s) in spec::specs(&worlds)? {
                if s["status"] == "proposed" {
                    println!(
                        "{id}  {}",
                        s["request"]
                            .as_str()
                            .unwrap_or("")
                            .lines()
                            .next()
                            .unwrap_or("")
                    );
                }
            }
        }
        "spec" if args.len() >= 3 => {
            let model = OpenAi::variants_from_env(1)?.remove(0);
            let s = spec::compile(
                &worlds,
                std::path::Path::new(arg(1)),
                arg(2),
                &model,
                flag(args, "--steps", 40)?,
            )?;
            let id = s["world"].as_str().unwrap_or("?");
            if s["status"] != "proposed" {
                println!("spec {id} invalid: {}", s["reason"].as_str().unwrap_or(""));
                return Ok(ExitCode::FAILURE);
            }
            println!(
                "spec {id}\n  read:   fennec-os spec show {id}\n  build:  fennec-os spec accept {id} [--attempts N]\n  refuse: fennec-os spec reject {id} [correction]"
            );
        }
        "land" if args.len() == 2 => println!("{}", land(&worlds, arg(1), None, None)?),
        "queue" if args.len() == 1 => {
            for (id, r) in review::queue(&worlds)? {
                let goal = r["goal"]
                    .as_str()
                    .unwrap_or("")
                    .lines()
                    .next()
                    .unwrap_or("");
                println!(
                    "{id}  {:<6}  {}  {goal}",
                    r["status"].as_str().unwrap_or("?"),
                    r["project"].as_str().unwrap_or("?")
                );
            }
        }
        "show" if args.len() == 2 => {
            let r = review::load(&worlds, arg(1))?;
            println!(
                "{}\nstatus: {}\nproject: {}",
                r["goal"].as_str().unwrap_or(""),
                r["status"],
                r["project"]
            );
            for a in r["attempts"].as_array().into_iter().flatten() {
                let verdict = if a["passed"] == true { "PASS" } else { "FAIL" };
                println!(
                    "\n{} {} {verdict} {}",
                    a["world"].as_str().unwrap_or("?"),
                    a["model"].as_str().unwrap_or("?"),
                    a["reason"].as_str().unwrap_or("")
                );
                for c in a["checks"].as_array().into_iter().flatten() {
                    println!(
                        "  exit {}  {}",
                        c["exit"],
                        c["command"].as_str().unwrap_or("")
                    );
                }
                for v in a["violations"].as_array().into_iter().flatten() {
                    println!(
                        "  rule {}  {}",
                        v["rule"].as_str().unwrap_or(""),
                        v["violation"].as_str().unwrap_or("")
                    );
                }
                if Some(a["world"].as_str()) == Some(r["winner"].as_str()) {
                    println!("{}", a["diff"].as_str().unwrap_or(""));
                }
            }
        }
        "profile" if args.len() == 2 => {
            // A spec is profiled like a run of one attempt.
            let attempts = match review::load(&worlds, arg(1)) {
                Ok(run) => run["attempts"].as_array().cloned().unwrap_or_default(),
                Err(_) => vec![spec::load(&worlds, arg(1))?],
            };
            for a in &attempts {
                let verdict = match a["passed"].as_bool() {
                    Some(true) => "PASS",
                    Some(false) => "FAIL",
                    None => a["status"].as_str().unwrap_or("?"),
                };
                println!(
                    "{} {} {verdict}\n  {}",
                    a["world"].as_str().unwrap_or("?"),
                    a["model"].as_str().unwrap_or("?"),
                    review::profile_line(&review::profile(a))
                );
                println!("  turn      at   model      in     out  think  tools");
                // `-` marks a turn the model was asked to take without reasoning.
                for t in a["turns"].as_array().into_iter().flatten() {
                    let secs = |v: &serde_json::Value| v.as_u64().unwrap_or(0) as f64 / 1000.0;
                    let tools: Vec<String> = t["tools"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|x| {
                            format!(
                                "{} {:.1}s",
                                x["name"].as_str().unwrap_or("?"),
                                secs(&x["ms"])
                            )
                        })
                        .collect();
                    println!(
                        "  {:>4}{} {:>6.1}s {:>6.1}s {:>7} {:>7} {:>6}  {}",
                        t["step"],
                        if t["think"] == false { "-" } else { " " },
                        secs(&t["at_ms"]),
                        secs(&t["model_ms"]),
                        t["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                        t["usage"]["completion_tokens"].as_u64().unwrap_or(0),
                        t["usage"]["completion_tokens_details"]["reasoning_tokens"]
                            .as_u64()
                            .unwrap_or(0),
                        tools.join(", ")
                    );
                }
                for c in a["checks"].as_array().into_iter().flatten() {
                    println!(
                        "  verify {:.1}s exit {}  {}",
                        c["ms"].as_u64().unwrap_or(0) as f64 / 1000.0,
                        c["exit"],
                        c["command"].as_str().unwrap_or("")
                    );
                }
            }
        }
        "reject" if args.len() == 2 || args.len() == 3 => {
            let correction = args.get(2).map(String::as_str);
            let count = review::reject(&worlds, arg(1), correction)?;
            if count >= 2 {
                let project = review::load(&worlds, arg(1))?["project"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                println!(
                    "said {count} times: \"{}\"\nwrite it down once, e.g.\n  fennec-os promote {project} '{{\"kind\": \"standing_command\", \"command\": \"...\"}}' --origin {}",
                    correction.unwrap_or(""),
                    arg(1)
                );
            }
        }
        "spar" if args.len() == 2 || (args.len() == 4 && arg(2) == "--steps") => {
            let model = OpenAi::variants_from_env(1)?.remove(0);
            let s = spec::spar(
                &worlds,
                std::path::Path::new(arg(1)),
                &model,
                flag(args, "--steps", 40)?,
            )?;
            let id = s["world"].as_str().unwrap_or("?");
            if s["status"] != "proposed" {
                println!("no finding ({id}): {}", s["reason"].as_str().unwrap_or(""));
                return Ok(ExitCode::FAILURE);
            }
            println!(
                "finding {id}: {}\n  read: fennec-os spec show {id}",
                s["request"].as_str().unwrap_or("")
            );
        }
        "sleep" if args.len() == 1 => {
            let model = OpenAi::variants_from_env(1)?.remove(0);
            let new = sleep::sleep(&worlds, &model)?;
            println!("{} new candidates; see `fennec-os notes`", new.len());
        }
        "notes" if args.len() == 1 => {
            for c in sleep::candidates(&worlds)?
                .iter()
                .filter(|c| c["status"] == "proposed")
            {
                println!(
                    "{}  {}  {}",
                    c["id"].as_str().unwrap_or("?"),
                    c["project"].as_str().unwrap_or("?"),
                    c["text"].as_str().unwrap_or("")
                );
            }
        }
        "keep" if args.len() == 2 => {
            let path = sleep::keep(&worlds, arg(1))?;
            println!("added to {}; commit it to take effect", path.display());
        }
        "dismiss" if args.len() == 2 => sleep::dismiss(&worlds, arg(1))?,
        "serve" if args.len() == 1 || (args.len() == 3 && arg(1) == "--port") => {
            let port = u16::try_from(flag(args, "--port", 0)?).map_err(std::io::Error::other)?;
            let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
            let token = serve::token()?;
            println!("http://{}/?t={token}", listener.local_addr()?);
            serve::serve(worlds, listener, &token)?;
        }
        "promote" if args.len() == 3 || (args.len() == 5 && arg(3) == "--origin") => {
            let rule: serde_json::Value =
                serde_json::from_str(arg(2)).map_err(std::io::Error::other)?;
            let id = rules::promote(std::path::Path::new(arg(1)), &rule, arg(4))?;
            println!(
                "{id} added to {}/{}; commit it to take effect",
                arg(1),
                rules::RULES_FILE
            );
        }
        _ => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn flag(args: &[String], name: &str, default: usize) -> std::io::Result<usize> {
    match args.iter().position(|a| a == name) {
        None => Ok(default),
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| std::io::Error::other(format!("{name} needs a number"))),
    }
}

fn print_run(worlds: &Worlds, record: &serde_json::Value) -> ExitCode {
    for a in record["attempts"].as_array().into_iter().flatten() {
        println!(
            "{}  {:<24} {}  {} lines  {}",
            a["world"].as_str().unwrap_or("?"),
            a["model"].as_str().unwrap_or("?"),
            if a["passed"] == true { "PASS" } else { "FAIL" },
            a["changed_lines"],
            a["reason"].as_str().unwrap_or(""),
        );
    }
    match record["winner"].as_str() {
        Some(id) => {
            println!(
                "winner {id}\n  review: git -C {} diff HEAD world/{id}\n  land:   fennec-os land {id}",
                record["project"].as_str().unwrap_or("?")
            );
            ExitCode::SUCCESS
        }
        None => {
            println!(
                "no attempt passed; records in {}/runs",
                worlds.root.display()
            );
            ExitCode::FAILURE
        }
    }
}
