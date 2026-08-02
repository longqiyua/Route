//! REPL loop — rustyline-backed interactive prompt with Tab completion.
//!
//! Tab completes the first word against the command name table. The
//! prompt itself is colored (purple `route❯`). Ctrl+C cancels the
//! current line and keeps running; Ctrl+D exits.

use std::path::PathBuf;

use anyhow::Result;
use route_basic::BasicRepository;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::DefaultHistory;
use rustyline::validate::Validator;
use rustyline::{Config, Context, Editor, Helper};

use crate::commands::{dispatch, Action};
use crate::fmt::{dim, err, prompt};

/// All first-word command names + aliases, used for Tab completion.
const COMMAND_NAMES: &[&str] = &[
    "help", "?", "status", "st", "log", "lg", "commit", "ci", "changes", "branch", "br", "rollback",
    "rb", "undo", "redo", "checkpoint", "cp", "diff", "export", "git", "root", "clear", "cls",
    "exit", "quit", "q",
];

struct RouteHelper;

impl Completer for RouteHelper {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Only complete the first word (the command verb).
        let upto = line.get(..pos).unwrap_or(line);
        if upto.contains(char::is_whitespace) {
            return Ok((pos, vec![]));
        }
        let matches: Vec<Pair> = COMMAND_NAMES
            .iter()
            .filter(|c| c.starts_with(upto))
            .map(|c| Pair {
                display: (*c).to_string(),
                replacement: (*c).to_string(),
            })
            .collect();
        Ok((0, matches))
    }
}

impl Hinter for RouteHelper {
    type Hint = String;
}
impl Highlighter for RouteHelper {}
impl Validator for RouteHelper {}
impl Helper for RouteHelper {}

/// Run the REPL. `repo` is the opened repository (or `None` if the cwd
/// is not a Route repo — in that case only `help`/`exit`/`clear`/`git`
/// work). `project_path` is the resolved project folder, threaded through
/// to `dispatch` so git-mode commands can run even without a route_basic
/// repository.
pub fn run(repo: Option<BasicRepository>, project_path: PathBuf) -> Result<()> {
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut rl: Editor<RouteHelper, DefaultHistory> = Editor::with_config(config)?;
    rl.set_helper(Some(RouteHelper));

    let p = prompt();
    loop {
        match rl.readline(&p) {
            Ok(line) => {
                let _ = rl.add_history_entry(&line);
                if matches!(dispatch(repo.as_ref(), &project_path, &line), Action::Exit) {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C — cancel current line, keep running.
                println!("^C");
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D — quit.
                println!("{}", dim("bye."));
                break;
            }
            Err(e) => {
                eprintln!("{} {}", err("✗"), e);
                break;
            }
        }
    }
    Ok(())
}
