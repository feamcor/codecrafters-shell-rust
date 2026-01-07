mod commands;
mod parser;
mod shell_helper;

use crate::commands::*;
use crate::parser::*;
use crate::shell_helper::*;
use rustyline::config::{BellStyle, CompletionType, Config};
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::io;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let helper = ShellHelper {
        completer: ShellCompleter::new(),
    };

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .bell_style(BellStyle::Audible)
        .build();

    let mut readline = Editor::with_config(config)?;
    let _ = readline.set_helper(Some(helper));

    'repl: loop {
        let input = match readline.readline(SHELL_PROMPT) {
            Ok(line) => {
                let _ = readline.add_history_entry(line.as_str());
                line
            }
            Err(ReadlineError::Interrupted) => break 'repl,
            Err(ReadlineError::Eof) => break 'repl,
            Err(e) => {
                eprintln!("Error: {:?}", e);
                break 'repl;
            }
        };

        let input = input.trim();
        if input.is_empty() {
            continue 'repl;
        }

        match parse_input(input) {
            Some(parsed_commands) => {
                let pipeline_length = parsed_commands.len();
                let pipeline = parsed_commands.into_iter().peekable().enumerate();
                let mut children: Vec<Child> = Vec::new();
                let mut previous_output: Option<os_pipe::PipeReader> = None;

                for (current_index, current_command) in pipeline {
                    let arguments_vec: Vec<String> =
                        current_command.tokens.clone().unwrap_or_default();
                    let mut arguments = arguments_vec.into_iter().enumerate();

                    let (stdin_builtin, stdin_external) =
                        if let Some(output) = previous_output.take() {
                            let output_for_external = output.try_clone()?;
                            (
                                Box::new(output) as Box<dyn Read>,
                                Stdio::from(output_for_external),
                            )
                        } else {
                            (Box::new(io::empty()) as Box<dyn Read>, Stdio::null())
                        };

                    let (stdout_builtin, _stdout_external, new_previous_output) =
                        if current_index < pipeline_length - 1 {
                            let (reader, writer) = os_pipe::pipe()?;
                            let writer_for_external = writer.try_clone()?;
                            (
                                Box::new(writer) as Box<dyn Write>,
                                Stdio::from(writer_for_external),
                                Some(reader),
                            )
                        } else {
                            let stdout = get_redirection(current_command.stdout.clone())
                                .unwrap_or(Box::new(io::stdout()));
                            (stdout, Stdio::inherit(), None)
                        };
                    previous_output = new_previous_output;

                    let mut stderr_builtin = get_redirection(current_command.stderr.clone())
                        .unwrap_or(Box::new(io::stderr()));

                    let command = match arguments.next() {
                        Some((_, argument)) => argument,
                        None => continue 'repl,
                    };

                    let path = if let Some(p) = search_executable(&command) {
                        p
                    } else if Path::new(&command).is_absolute()
                        && is_executable(&PathBuf::from(&command)).unwrap_or(false)
                    {
                        command.clone()
                    } else {
                        match command.as_str() {
                            COMMAND_CD | COMMAND_ECHO | COMMAND_EXIT | COMMAND_PWD
                            | COMMAND_TYPE | COMMAND_HISTORY => "".to_string(), // It's a builtin
                            _ => {
                                writeln!(stderr_builtin, "{command}: command not found")
                                    .unwrap_or_default();
                                continue;
                            }
                        }
                    };

                    match command.as_str() {
                        COMMAND_CD => {
                            command_cd(arguments, stdin_builtin, stdout_builtin, stderr_builtin);
                        }
                        COMMAND_ECHO => {
                            command_echo(arguments, stdin_builtin, stdout_builtin, stderr_builtin);
                        }
                        COMMAND_EXIT => {
                            command_exit(arguments, stdin_builtin, stdout_builtin, stderr_builtin);
                        }
                        COMMAND_PWD => {
                            command_pwd(arguments, stdin_builtin, stdout_builtin, stderr_builtin);
                        }
                        COMMAND_TYPE => {
                            command_type(arguments, stdin_builtin, stdout_builtin, stderr_builtin);
                        }
                        COMMAND_HISTORY => {
                            command_history(
                                &readline,
                                arguments,
                                stdin_builtin,
                                stdout_builtin,
                                stderr_builtin,
                            );
                        }
                        _ => {
                            if pipeline_length == 1 {
                                let mut stdout_builtin = stdout_builtin;
                                match run_executable(
                                    &path,
                                    &command,
                                    arguments,
                                    stdin_external,
                                    &mut stdout_builtin,
                                    &mut stderr_builtin,
                                    current_command.stdout.file_name.is_none(),
                                    current_command.stderr.file_name.is_none(),
                                    None,
                                ) {
                                    Ok(mut child) => {
                                        let _ = child.wait();
                                    }
                                    Err(e) => {
                                        writeln!(stderr_builtin, "Error: {:?}", e)
                                            .unwrap_or_default();
                                    }
                                }
                            } else {
                                // Pipeline case
                                if let Ok(spawned) = Command::new(&path)
                                    .arg0(&command)
                                    .args(arguments.map(|(_, arg)| arg))
                                    .stdin(stdin_external)
                                    .stdout(_stdout_external)
                                    .spawn()
                                {
                                    children.push(spawned);
                                } else {
                                    writeln!(
                                        stderr_builtin,
                                        "Error: Failed to spawn child process {command}"
                                    )
                                    .unwrap_or_default();
                                }
                            }
                        }
                    }
                }

                for mut child in children {
                    let _ = child.wait();
                }
            }
            None => {}
        }
    }

    Ok(())
}
