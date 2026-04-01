mod commands;
mod executor;
mod jobs;
mod parser;
mod shell_helper;

use crate::commands::BuiltinAction;
use crate::executor::execute_pipeline;
use crate::executor::ShellContext;
use crate::jobs::JobManager;
use crate::parser::parse_input;
use crate::parser::SHELL_PROMPT;
use crate::shell_helper::ShellCompleter;
use crate::shell_helper::ShellHelper;
use rustyline::config::BellStyle;
use rustyline::config::CompletionType;
use rustyline::config::Config;
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::history::SearchDirection;
use rustyline::Editor;
use std::io::Write;

fn save_history_plain<H: rustyline::Helper, I: History>(readline: &Editor<H, I>, path: &str) {
    if let Ok(mut file) = std::fs::File::create(path) {
        let history = readline.history();
        for i in 0..history.len() {
            if let Ok(Some(entry)) = history.get(i, SearchDirection::Forward) {
                let _ = writeln!(file, "{}", entry.entry);
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let helper = ShellHelper {
        completer: ShellCompleter::new(),
    };

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .bell_style(BellStyle::Audible)
        .history_ignore_dups(false)?
        .build();

    let mut readline = Editor::with_config(config)?;
    readline.set_helper(Some(helper));

    let histfile_path: Option<String> = std::env::var("HISTFILE").ok();
    if let Some(ref path) = histfile_path {
        let _ = readline.load_history(path);
    }

    let mut last_appended_index: usize = readline.history().len();
    let mut job_mgr = JobManager::new();

    'repl: loop {
        job_mgr.reap();
        let input = match readline.readline(SHELL_PROMPT) {
            Ok(line) => {
                let _ = readline.add_history_entry(line.as_str());
                line
            }
            Err(ReadlineError::Interrupted | ReadlineError::Eof) => break 'repl,
            Err(e) => {
                eprintln!("Error: {e:?}");
                break 'repl;
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue 'repl;
        }

        if let Some(pipeline) = parse_input(input) {
            let mut ctx = ShellContext {
                editor: &mut readline,
                last_appended_index: &mut last_appended_index,
            };
            match execute_pipeline(pipeline, &mut job_mgr, &mut ctx)? {
                BuiltinAction::Exit(code) => {
                    if let Some(ref path) = histfile_path {
                        save_history_plain(&readline, path);
                    }
                    std::process::exit(code);
                }
                BuiltinAction::Continue => {}
            }
        }
    }

    job_mgr.wait_all();
    if let Some(ref path) = histfile_path {
        save_history_plain(&readline, path);
    }

    Ok(())
}
