use crate::parser::COMMAND_CD;
use crate::parser::COMMAND_ECHO;
use crate::parser::COMMAND_EXIT;
use crate::parser::COMMAND_HISTORY;
use crate::parser::COMMAND_JOBS;
use crate::parser::COMMAND_PWD;
use crate::parser::COMMAND_TYPE;
use crate::parser::ENVIRONMENT_VARIABLE_PATH;
use crate::parser::ENVIRONMENT_VARIABLE_PATH_DELIMITER;
use crate::parser::SHELL_PROMPT;
use rustyline::completion::Completer;
use rustyline::completion::Pair;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::Completer;
use rustyline::Context;
use rustyline::Helper;
use rustyline::Hinter;
use rustyline::Validator;
use std::env::var;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::sync::Mutex;

static LAST_PREFIX: Mutex<Option<String>> = Mutex::new(None);

fn compute_lcp(prefix: &str, matches: &[(String, bool)]) -> String {
    if matches.is_empty() {
        return prefix.to_string();
    }

    let matching: Vec<_> = matches.iter().filter(|(name, _)| name.starts_with(prefix)).collect();

    if matching.is_empty() {
        return prefix.to_string();
    }

    if matching.len() == 1 {
        return matching[0].0.clone();
    }

    let mut lcp_chars: Vec<char> = Vec::new();
    for i in 0.. {
        let Some(c) = matching[0].0.chars().nth(i) else {
            break;
        };
        if matching.iter().all(|(name, _)| name.chars().nth(i) == Some(c)) {
            lcp_chars.push(c);
        } else {
            break;
        }
    }

    lcp_chars.into_iter().collect()
}

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
            COMMAND_JOBS.to_string(),
        ];

        if let Ok(path_var) = var(ENVIRONMENT_VARIABLE_PATH) {
            for path_dir in path_var.split(ENVIRONMENT_VARIABLE_PATH_DELIMITER) {
                if let Ok(dir_entries) = std::fs::read_dir(path_dir) {
                    for dir_entry in dir_entries.flatten() {
                        if let Ok(entry_metadata) = dir_entry.metadata() {
                            if entry_metadata.is_file() && (entry_metadata.permissions().mode() & 0o111 != 0) {
                                if let Ok(file_name) = dir_entry.file_name().into_string() {
                                    commands.push(file_name);
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

    fn find_matching_entries(prefix: &str) -> Vec<(String, bool)> {
        let (dir_path, file_prefix) = if let Some(last_slash) = prefix.rfind('/') {
            let dir = &prefix[..=last_slash];
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
                if name.starts_with(file_prefix) {
                    Some((name, e.path().is_dir()))
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
            let prefix_start = line[..pos].rfind(' ').map_or(0, |i| i + 1);
            let prefix = &line[prefix_start..pos];

            let matches = Self::find_matching_entries(prefix);

            if matches.len() == 1 {
                let (filename, is_dir) = &matches[0];
                let dir_prefix = if let Some(slash_pos) = prefix.rfind('/') {
                    &prefix[..=slash_pos]
                } else {
                    ""
                };
                let full_path = format!("{dir_prefix}{filename}");
                let trailing = if *is_dir { "/" } else { " " };
                return Ok((
                    prefix_start,
                    vec![Pair {
                        display: full_path.clone(),
                        replacement: format!("{full_path}{trailing}"),
                    }],
                ));
            }

            if matches.len() > 1 {
                let lcp = compute_lcp(prefix, &matches);

                if lcp.len() > prefix.len() {
                    let dir_prefix = if let Some(slash_pos) = prefix.rfind('/') {
                        &prefix[..=slash_pos]
                    } else {
                        ""
                    };
                    let full_path = format!("{dir_prefix}{lcp}");
                    return Ok((
                        prefix_start,
                        vec![Pair {
                            display: full_path.clone(),
                            replacement: full_path,
                        }],
                    ));
                }

                let mut last_prefix = LAST_PREFIX.lock().unwrap();
                let first_tab = match &*last_prefix {
                    Some(p) if p == prefix => false,
                    _ => {
                        *last_prefix = Some(prefix.to_string());
                        true
                    }
                };

                if first_tab {
                    eprint!("\x07");
                    return Ok((0, Vec::new()));
                }

                let mut matches_sorted: Vec<_> = matches
                    .iter()
                    .map(|(filename, is_dir)| {
                        if *is_dir {
                            format!("{filename}/")
                        } else {
                            filename.clone()
                        }
                    })
                    .collect();

                matches_sorted.sort_by_key(|a| a.to_lowercase());

                print!("\n{}\n{}{}", matches_sorted.join("  "), SHELL_PROMPT, line);
                std::io::stdout().flush().ok();

                return Ok((0, Vec::new()));
            }

            return Ok((0, Vec::new()));
        }

        let (start, word) = rustyline::completion::extract_word(line, pos, None, char::is_whitespace);

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
