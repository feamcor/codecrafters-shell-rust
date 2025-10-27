#[allow(unused_imports)]
use std::io::{self, Write};
use std::iter::Enumerate;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::SplitWhitespace;

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
                let mut arguments = input.split_whitespace().enumerate();
                let (_, command) = arguments.next().unwrap();
                match command {
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

fn command_exit(arguments: Enumerate<SplitWhitespace>) {
    let mut exit_status = 0;
    for (index, argument) in arguments {
        if index == 1 {
            exit_status = argument.parse().unwrap_or(1);
            break;
        }
    }
    std::process::exit(exit_status);
}

fn command_echo(arguments: Enumerate<SplitWhitespace>) {
    for (index, argument) in arguments {
        if index > 1 { print!(" "); }
        print!("{argument}");
    }
    println!();
}

fn command_type(arguments: Enumerate<SplitWhitespace>) {
    for (index, argument) in arguments {
        if index == 1 {
            match argument {
                "echo" | "exit" | "pwd" | "type" => println!("{argument} is a shell builtin"),
                _ => {
                    match search_executable(argument) {
                        Some(full_path_to_executable) => println!("{argument} is {full_path_to_executable}"),
                        None => eprintln!("{argument}: not found")
                    }
                },
            }
        }
        break;
    }
}

fn search_executable(command: &str) -> Option<String> {
    let paths = std::env::var("PATH").unwrap_or(String::new());
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

fn run_executable(command: &str, arguments: Enumerate<SplitWhitespace>) {
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

fn command_pwd(_arguments: Enumerate<SplitWhitespace>) {
    let current_directory = std::env::current_dir().unwrap();
    println!("{}", current_directory.to_string_lossy());
}