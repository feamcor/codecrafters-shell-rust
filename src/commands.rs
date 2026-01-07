use crate::parser::*;
use rustyline::history::SearchDirection;
use rustyline::Editor;
use std::env::{current_dir, set_current_dir, var};
use std::fs::OpenOptions;
use std::io;
use std::io::{Read, Write};
use std::iter::Enumerate;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::vec::IntoIter;

pub fn is_executable(full_path_to_executable: &PathBuf) -> io::Result<bool> {
    Ok(full_path_to_executable.is_file()
        && (full_path_to_executable.metadata()?.permissions().mode() & 0o111 != 0))
}

pub fn search_executable(command: &str) -> Option<String> {
    if let Ok(path_var) = var(ENVIRONMENT_VARIABLE_PATH) {
        for path_dir in path_var.split(ENVIRONMENT_VARIABLE_PATH_DELIMITER) {
            let full_path = PathBuf::from(path_dir).join(command);
            if is_executable(&full_path).unwrap_or(false) {
                return Some(full_path.to_string_lossy().to_string());
            }
        }
    }
    None
}

pub fn get_redirection(output: OutputRedirection) -> Option<Box<dyn Write>> {
    if let Some(file_name) = output.file_name {
        let mut options = OpenOptions::new();
        options.create(true).write(true);
        if output.append_to {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let file = options.open(&file_name);
        match file {
            Ok(file) => Some(Box::new(file) as Box<dyn Write>),
            Err(e) => {
                eprintln!("Error opening file {file_name}: {e}");
                None
            }
        }
    } else {
        None
    }
}

pub fn run_executable(
    executable_path: &str,
    original_command: &str,
    command_arguments: Enumerate<IntoIter<String>>,
    stdin: Stdio,
    stdout: &mut Box<dyn Write>,
    stderr: &mut Box<dyn Write>,
    inherit_stdout: bool,
    inherit_stderr: bool,
    previous_child: Option<Child>,
) -> Result<Child, io::Error> {
    let mut command = Command::new(executable_path);
    command.arg0(original_command);
    command.stdin(stdin);

    if inherit_stdout {
        command.stdout(Stdio::inherit());
    } else {
        command.stdout(Stdio::piped());
    }

    if inherit_stderr {
        command.stderr(Stdio::inherit());
    } else {
        command.stderr(Stdio::piped());
    }

    for (_, argument) in command_arguments {
        command.arg(argument);
    }

    let mut child = command.spawn()?;

    if let Some(mut previous) = previous_child {
        previous.wait()?;
    }

    if !inherit_stdout {
        if let Some(mut child_stdout) = child.stdout.take() {
            io::copy(&mut child_stdout, stdout)?;
        }
    }

    if !inherit_stderr {
        if let Some(mut child_stderr) = child.stderr.take() {
            io::copy(&mut child_stderr, stderr)?;
        }
    }

    Ok(child)
}

pub fn command_exit(
    mut arguments: Enumerate<IntoIter<String>>,
    _stdin: Box<dyn Read>,
    _stdout: Box<dyn Write>,
    _stderr: Box<dyn Write>,
) {
    let exit_code = match arguments.next() {
        Some((_, code)) => code.parse::<i32>().unwrap_or(0),
        None => 0,
    };
    std::process::exit(exit_code);
}

pub fn command_echo(
    arguments: Enumerate<IntoIter<String>>,
    _stdin: Box<dyn Read>,
    mut stdout: Box<dyn Write>,
    _stderr: Box<dyn Write>,
) {
    let mut expand_escape_sequences_flag = false;
    let mut first_argument = true;

    for (index, argument) in arguments {
        if first_argument && index == 0 && argument == COMMAND_ECHO_FLAG_EXPAND_ESCAPE {
            expand_escape_sequences_flag = true;
            continue;
        }

        if !first_argument {
            write!(stdout, " ").unwrap_or_default();
        }

        if expand_escape_sequences_flag {
            write!(stdout, "{}", expand_escape_sequences(&argument)).unwrap_or_default();
        } else {
            write!(stdout, "{argument}").unwrap_or_default();
        }

        first_argument = false;
    }
    writeln!(stdout).unwrap_or_default();
    stdout.flush().unwrap_or_default();
}

pub fn command_type(
    mut arguments: Enumerate<IntoIter<String>>,
    _stdin: Box<dyn Read>,
    mut stdout: Box<dyn Write>,
    mut stderr: Box<dyn Write>,
) {
    if let Some((_, command)) = arguments.next() {
        match command.as_str() {
            COMMAND_CD | COMMAND_ECHO | COMMAND_EXIT | COMMAND_PWD | COMMAND_TYPE
            | COMMAND_HISTORY => {
                writeln!(stdout, "{command} is a shell builtin").unwrap_or_default();
            }
            _ => {
                if let Some(path) = search_executable(&command) {
                    writeln!(stdout, "{command} is {path}").unwrap_or_default();
                } else {
                    writeln!(stderr, "{command}: not found").unwrap_or_default();
                }
            }
        }
    }
    stdout.flush().unwrap_or_default();
    stderr.flush().unwrap_or_default();
}

pub fn command_pwd(
    _arguments: Enumerate<IntoIter<String>>,
    _stdin: Box<dyn Read>,
    mut stdout: Box<dyn Write>,
    mut stderr: Box<dyn Write>,
) {
    if let Ok(current_dir) = current_dir() {
        writeln!(stdout, "{}", current_dir.display()).unwrap_or_default();
    } else {
        writeln!(stderr, "pwd: error retrieving current directory").unwrap_or_default();
    }
    stdout.flush().unwrap_or_default();
    stderr.flush().unwrap_or_default();
}

pub fn command_history<H: rustyline::Helper, I: rustyline::history::History>(
    readline: &Editor<H, I>,
    arguments: Enumerate<IntoIter<String>>,
    _stdin: Box<dyn Read>,
    mut stdout: Box<dyn Write>,
    mut stderr: Box<dyn Write>,
) {
    let history = readline.history();
    let args: Vec<String> = arguments.map(|(_, a)| a).collect();
    let count = if let Some(arg) = args.first() {
        arg.parse::<usize>().unwrap_or(0)
    } else {
        0
    };

    let len = history.len();
    let start_index = if count > 0 {
        len.saturating_sub(count)
    } else {
        0
    };

    for i in start_index..len {
        if let Ok(Some(entry)) = history.get(i, SearchDirection::Forward) {
            writeln!(stdout, "{:>5}  {}", i + 1, entry.entry).unwrap_or_default();
        }
    }
    stdout.flush().unwrap_or_default();
    stderr.flush().unwrap_or_default();
}

pub fn command_cd(
    mut arguments: Enumerate<IntoIter<String>>,
    _stdin: Box<dyn Read>,
    mut stdout: Box<dyn Write>,
    mut stderr: Box<dyn Write>,
) {
    let directory = match arguments.next() {
        Some((_, dir)) => {
            if dir == HOME_DIRECTORY {
                var(ENVIRONMENT_VARIABLE_HOME).unwrap_or_default()
            } else {
                dir
            }
        }
        None => var(ENVIRONMENT_VARIABLE_HOME).unwrap_or_default(),
    };

    match set_current_dir(&directory) {
        Ok(_) => {}
        Err(_) => {
            writeln!(stderr, "cd: {directory}: No such file or directory").unwrap_or_default()
        }
    }
    stdout.flush().unwrap_or_default();
    stderr.flush().unwrap_or_default();
}
