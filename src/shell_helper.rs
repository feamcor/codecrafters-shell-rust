use crate::parser::*;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::{Completer, Context, Helper, Hinter, Validator};
use std::env::var;
use std::os::unix::fs::PermissionsExt;

#[derive(Helper, Completer, Hinter, Validator)]
pub struct ShellHelper {
    #[rustyline(Completer)]
    pub completer: ShellCompleter,
}

impl Highlighter for ShellHelper {}

pub struct ShellCompleter {
    pub commands: Vec<String>,
}

impl ShellCompleter {
    pub fn new() -> Self {
        let mut commands = vec![
            COMMAND_CD.to_string(),
            COMMAND_ECHO.to_string(),
            COMMAND_EXIT.to_string(),
            COMMAND_PWD.to_string(),
            COMMAND_TYPE.to_string(),
            COMMAND_HISTORY.to_string(),
        ];

        if let Ok(path_var) = var(ENVIRONMENT_VARIABLE_PATH) {
            for path_dir in path_var.split(ENVIRONMENT_VARIABLE_PATH_DELIMITER) {
                if let Ok(dir_entries) = std::fs::read_dir(path_dir) {
                    for dir_entry in dir_entries.flatten() {
                        if let Ok(entry_metadata) = dir_entry.metadata() {
                            if entry_metadata.is_file()
                                && (entry_metadata.permissions().mode() & 0o111 != 0)
                            {
                                if let Ok(file_name) = dir_entry.file_name().into_string() {
                                    commands.push(file_name)
                                }
                            }
                        }
                    }
                }
            }
        }

        commands.sort_unstable();
        commands.dedup();

        Self { commands }
    }
}

impl Completer for ShellCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>), ReadlineError> {
        if pos > 0 && line.chars().take(pos).any(|c| c.is_whitespace()) {
            return Ok((0, Vec::new()));
        }

        let (start, word) =
            rustyline::completion::extract_word(line, pos, None, |c| c.is_whitespace());

        let mut candidates = Vec::new();
        for command in &self.commands {
            if command.starts_with(word) {
                candidates.push(Pair {
                    display: command.clone(),
                    replacement: format!("{command} "),
                });
            }
        }
        Ok((start, candidates))
    }
}
