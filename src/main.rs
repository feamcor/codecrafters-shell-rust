use core::slice::Iter;
use std::env::{current_dir, set_current_dir, var};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::iter::Enumerate;
use std::path::{Path, PathBuf};
use std::process::Command;

static SHELL_PROMPT: &str = "$ ";

enum OutputType {
    STDOUT = 1,
    STDERR = 2,
}

struct OutputRedirection {
    file_name: Option<String>,
    append_to: bool,
    output_type: OutputType,
}

struct ParsedCommand {
    tokens: Option<Vec<String>>,
    stdout: OutputRedirection,
    stderr: OutputRedirection,
}

fn main() {
    let mut input = String::new();
    loop {
        print!("{SHELL_PROMPT}");
        io::stdout().flush().unwrap();
        input.clear();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim();
                if input.is_empty() { continue; }
                let parsed_command = parse_command(input);
                let mut arguments = match &parsed_command.tokens {
                    Some(tokens) => tokens.iter().enumerate(),
                    None => continue,
                };
                let (_, command) = arguments.next().unwrap();
                match command.as_str() {
                    "cd"   => command_cd(arguments,          parsed_command.stdout, parsed_command.stderr),
                    "echo" => command_echo(arguments,        parsed_command.stdout, parsed_command.stderr),
                    "exit" => command_exit(arguments,        parsed_command.stdout, parsed_command.stderr),
                    "pwd"  => command_pwd(arguments,         parsed_command.stdout, parsed_command.stderr),
                    "type" => command_type(arguments,        parsed_command.stdout, parsed_command.stderr),
                    _ => run_executable(command, arguments,  parsed_command.stdout, parsed_command.stderr),
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
            }
        }
    }
}

fn command_exit(arguments: Enumerate<Iter<String>>, _stdout: OutputRedirection, _stderr: OutputRedirection) {
    let mut exit_status = 0;
    for (_index, argument) in arguments.take(1) {
        exit_status = argument.parse().unwrap_or(1);
    }
    std::process::exit(exit_status);
}

fn command_echo(arguments: Enumerate<Iter<String>>, stdout: OutputRedirection, _stderr: OutputRedirection) {
    let mut stdout = get_output_redirection(stdout);
    for (index, argument) in arguments {
        if index > 1 { write!(stdout, " ").unwrap_or_default(); }
        write!(stdout, "{argument}").unwrap_or_default();
    }
    writeln!(stdout).unwrap_or_default();
}

fn command_type(arguments: Enumerate<Iter<String>>, stdout: OutputRedirection, stderr: OutputRedirection) {
    let mut stdout = get_output_redirection(stdout);
    let mut stderr = get_output_redirection(stderr);
    for (_index, argument) in arguments.take(1) {
        match argument.as_str() {
            "cd" | "echo" | "exit" | "pwd" | "type" =>
                writeln!(stdout, "{argument} is a shell builtin").unwrap_or_default(),
            _ => {
                match search_executable(argument) {
                    Some(full_path_to_executable) => writeln!(stdout, "{argument} is {full_path_to_executable}").unwrap_or_default(),
                    None => writeln!(stderr, "{argument}: not found").unwrap_or_default(),
                }
            },
        }
    }
}

fn search_executable(command: &str) -> Option<String> {
    let paths = var("PATH").unwrap_or(String::new());
    for path in paths.split(":") {
        let full_path_to_executable = Path::new(path).join(command);
        if full_path_to_executable.is_file() && is_executable(&full_path_to_executable).unwrap_or(false) {
            return Some(full_path_to_executable.to_string_lossy().into_owned());
        }
    }
    None
}

fn is_executable(full_path_to_executable: &PathBuf) -> io::Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(full_path_to_executable)?;
    Ok(metadata.permissions().mode() & 0o111 != 0)
}

fn run_executable(command: &str, arguments: Enumerate<Iter<String>>, stdout: OutputRedirection, stderr: OutputRedirection) {
    let mut stdout = get_output_redirection(stdout);
    let mut stderr = get_output_redirection(stderr);
    match search_executable(command) {
        Some(_) => {
            let output = Command::new(command)
                .args(arguments.map(|(_, argument)| argument))
                .output();
            match output {
                Ok(output) => write!(stdout, "{}", String::from_utf8_lossy(&output.stdout)).unwrap_or_default(),
                Err(e) => writeln!(stderr, "{e}").unwrap_or_default(),
            }
        },
        None => writeln!(stderr, "{command}: command not found").unwrap_or_default(),
    }
}

fn command_pwd(_arguments: Enumerate<Iter<String>>, stdout: OutputRedirection, _stderr: OutputRedirection) {
    let mut stdout = get_output_redirection(stdout);
    let current_directory = current_dir().unwrap();
    writeln!(stdout, "{}", current_directory.to_string_lossy()).unwrap_or_default();
}

fn command_cd(arguments: Enumerate<Iter<String>>, _stdout: OutputRedirection, stderr: OutputRedirection) {
    let mut stderr = get_output_redirection(stderr);
    let home_directory = var("HOME").unwrap_or(String::new());
    let mut directory: &str = "";
    for (_index, argument) in arguments.take(1) {
        directory = match argument.as_str() {
            "~" => &home_directory,
            _   => argument,
        };
    }
    directory = if directory.is_empty() { &home_directory } else { directory };
    match set_current_dir(directory) {
        Ok(_) => (),
        Err(_) => writeln!(stderr, "cd: {directory}: No such file or directory").unwrap_or_default(),
    }
}

fn parse_command(input: &str) -> ParsedCommand {
    let mut tokens = Vec::new();
    let mut stdout: OutputRedirection = OutputRedirection { file_name: None, append_to: false, output_type: OutputType::STDOUT };
    let mut stderr: OutputRedirection = OutputRedirection { file_name: None, append_to: false, output_type: OutputType::STDERR };

    let mut current_token = String::new();
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;
    let mut escape_next_char = false;
    let mut in_stdout_redirection = false;
    let mut in_stderr_redirection = false;

    let mut characters = input.trim().chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '\'' if !escape_next_char => {
                if current_token.is_empty() {
                    in_single_quotes = true;
                    in_double_quotes = false;
                } else {
                    if let Some(next_character) = characters.peek() {
                        if in_single_quotes && next_character.is_whitespace() {
                            tokens.push(current_token);
                            current_token = String::new();
                            in_single_quotes = false;
                            in_double_quotes = false;
                        } else if in_double_quotes {
                            current_token.push(character);
                        }
                    }
                }
            },
            '"' if !escape_next_char => {
                if current_token.is_empty() {
                    in_single_quotes = false;
                    in_double_quotes = true;
                } else {
                    if let Some(next_character) = characters.peek() {
                        if in_double_quotes && next_character.is_whitespace() {
                            tokens.push(current_token);
                            current_token = String::new();
                            in_single_quotes = false;
                            in_double_quotes = false;
                        } else if in_single_quotes {
                            current_token.push(character);
                        }
                    }
                }
            },
            '\\' if !escape_next_char => {
                if in_single_quotes {
                    current_token.push(character);
                } else if in_double_quotes {
                    if let Some(next_character) = characters.peek() {
                        match next_character {
                            '"' | '\\' => escape_next_char = true,
                            _ => current_token.push(character),
                        }
                    }
                } else {
                    escape_next_char = true;
                }
            },
            file_descriptor if file_descriptor == '1' && !escape_next_char && !in_single_quotes && !in_double_quotes => {
                if let Some(next_character) = characters.peek() {
                    if *next_character == '>' {
                        in_stdout_redirection = true;
                        characters.next();
                    } else {
                        current_token.push(file_descriptor);
                    }
                }
            },
            file_descriptor if file_descriptor == '2' && !escape_next_char && !in_single_quotes && !in_double_quotes => {
                if let Some(next_character) = characters.peek() {
                    if *next_character == '>' {
                        in_stderr_redirection = true;
                        characters.next();
                    } else {
                        current_token.push(file_descriptor);
                    }
                }
            },
            redirect_operator if redirect_operator == '>' && in_stdout_redirection => {
                stdout.append_to = true;
            },
            redirect_operator if redirect_operator == '>' && in_stderr_redirection => {
                stderr.append_to = true;
            }
            redirect_operator if redirect_operator == '>' && !in_stdout_redirection && !escape_next_char && !in_single_quotes && !in_double_quotes => {
                in_stdout_redirection = true;
            },
            character if character.is_whitespace() && !escape_next_char => {
                if in_single_quotes || in_double_quotes {
                    current_token.push(character);
                } else if !current_token.is_empty() {
                    if in_stdout_redirection {
                        stdout.file_name = Some(current_token);
                        in_stdout_redirection = false;
                    } else if in_stderr_redirection {
                        stderr.file_name = Some(current_token);
                        in_stderr_redirection = false;
                    } else {
                        tokens.push(current_token);
                    }
                    current_token = String::new();
                }
            },
            _ => {
                current_token.push(character);
                escape_next_char = false;
            },
        }
    }

    if !current_token.is_empty() {
        if in_stdout_redirection {
            stdout.file_name = Some(current_token);
        } else if in_stderr_redirection {
            stderr.file_name = Some(current_token);
        } else {
            tokens.push(current_token);
        }
    }

    ParsedCommand {
        tokens: if tokens.is_empty() { None } else { Some(tokens) },
        stdout,
        stderr,
    }
}

fn get_output_redirection(output: OutputRedirection) -> Box<dyn Write> {
    match output.file_name {
        Some(file_name) => {
            let file = if output.append_to {
                OpenOptions::new().append(true).create(true).open(file_name).unwrap()
            } else {
                File::create(file_name).unwrap()
            };
            let writer = io::BufWriter::new(file);
            Box::new(writer)

        },
        None => {
            match output.output_type {
                OutputType::STDOUT => Box::new(io::stdout()),
                OutputType::STDERR => Box::new(io::stderr()),
            }
        },
    }
}