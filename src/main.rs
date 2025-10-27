use core::slice::Iter;
use std::env::{current_dir, set_current_dir, var};
use std::io::{self, Write};
use std::iter::Enumerate;
use std::path::{Path, PathBuf};
use std::process::Command;

static SHELL_PROMPT: &str = "$ ";

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
                let tokens = parse_tokens(input);
                let mut arguments = tokens.iter().enumerate();
                let (_, command) = arguments.next().unwrap();
                match command.as_str() {
                    "cd"   => command_cd(arguments),
                    "echo" => command_echo(arguments),
                    "exit" => command_exit(arguments),
                    "pwd"  => command_pwd(arguments),
                    "type" => command_type(arguments),
                    _ => run_executable(command, arguments),
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
            }
        }
    }
}

fn command_exit(arguments: Enumerate<Iter<String>>) {
    let mut exit_status = 0;
    for (_index, argument) in arguments.take(1) {
        exit_status = argument.parse().unwrap_or(1);
    }
    std::process::exit(exit_status);
}

fn command_echo(arguments: Enumerate<Iter<String>>) {
    for (index, argument) in arguments {
        if index > 1 { print!(" "); }
        print!("{argument}");
    }
    println!();
}

fn command_type(arguments: Enumerate<Iter<String>>) {
    for (_index, argument) in arguments.take(1) {
        match argument.as_str() {
            "cd" | "echo" | "exit" | "pwd" | "type" =>
                println!("{argument} is a shell builtin"),
            _ => {
                match search_executable(argument) {
                    Some(full_path_to_executable) => println!("{argument} is {full_path_to_executable}"),
                    None => eprintln!("{argument}: not found")
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

fn run_executable(command: &str, arguments: Enumerate<Iter<String>>) {
    match search_executable(command) {
        Some(_) => {
            let output = Command::new(command)
                .args(arguments.map(|(_, argument)| argument))
                .output();
            match output {
                Ok(output) => print!("{}", String::from_utf8_lossy(&output.stdout)),
                Err(e) => eprintln!("{e}"),
            }
        },
        None => eprintln!("{command}: command not found"),
    }
}

fn command_pwd(_arguments: Enumerate<Iter<String>>) {
    let current_directory = current_dir().unwrap();
    println!("{}", current_directory.to_string_lossy());
}

fn command_cd(arguments: Enumerate<Iter<String>>) {
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
        Err(_) => eprintln!("cd: {directory}: No such file or directory"),
    }
}

fn parse_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_single_quotes = false;

    let mut characters = input.trim().chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '\'' => {
                if current_token.is_empty() {
                    in_single_quotes = true;
                } else {
                    if let Some(next_character) = characters.peek() {
                        if in_single_quotes && next_character.is_whitespace() {
                            tokens.push(current_token);
                            current_token = String::new();
                            in_single_quotes = false;
                        }
                    }
                }
            },
            character if character.is_whitespace() => {
                if in_single_quotes {
                    current_token.push(character);
                } else if !current_token.is_empty() {
                    tokens.push(current_token);
                    current_token = String::new();
                }
            },
            _ => current_token.push(character),
        }
    }

    if !current_token.is_empty() { tokens.push(current_token); }

    tokens
}
