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
                            if entry_metadata.is_file() && (entry_metadata.permissions().mode() & 0o111 != 0) {
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

    fn find_matching_files(prefix: &str) -> Vec<String> {
        if prefix.is_empty() {
            return Vec::new();
        }

        let (dir_path, file_prefix) = if let Some(last_slash) = prefix.rfind('/') {
            let dir = &prefix[..last_slash + 1];
            let file = &prefix[last_slash + 1..];
            (Some(dir), file)
        } else {
            (None, prefix)
        };

        let search_dir = dir_path.unwrap_or(".");

        std::fs::read_dir(search_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                if name.starts_with(file_prefix) && e.path().is_file() {
                    Some(name)
                } else {
                    None
                }
            })
            .collect()
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
        if pos > 0 && line[..pos].contains(' ') {
            let prefix_start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
            let prefix = &line[prefix_start..pos];

            let matches = Self::find_matching_files(prefix);

            if matches.len() == 1 {
                let dir_prefix = if let Some(slash_pos) = prefix.rfind('/') {
                    &prefix[..=slash_pos]
                } else {
                    ""
                };
                let full_path = format!("{}{}", dir_prefix, matches[0]);
                return Ok((
                    prefix_start,
                    vec![Pair {
                        display: full_path.clone(),
                        replacement: format!("{} ", full_path),
                    }],
                ));
            }

            return Ok((0, Vec::new()));
        }

        let (start, word) = rustyline::completion::extract_word(line, pos, None, |c| c.is_whitespace());

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
