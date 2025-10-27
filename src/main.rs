#[allow(unused_imports)]
use std::io::{self, Write};
use std::iter::Enumerate;
use std::path::{Path, PathBuf};
use std::str::SplitWhitespace;

static SHELL_PROMPT: &str = "$";

fn main() {
    let mut input = String::new();
    loop {
        print!("{} ", SHELL_PROMPT);
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
                    "type" => command_type(arguments),
                    _ => eprintln!("{command}: command not found"),
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
    std::process::exit(exit_status as i32);
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
                "echo" | "exit" | "type" => println!("{argument} is a shell builtin"),
                _ => {
                    match search_executable(argument) {
                        Some(path) => println!("{argument} is {path}"),
                        None => println!("{argument}: not found")
                    }
                },
            }
        }
        break;
    }
}

fn search_executable(executable: &str) -> Option<String> {
    let mut paths = std::env::var("PATH").unwrap();
    paths.push_str(":");
    for directory in paths.split(":") {
        let full_path = Path::new(directory).join(executable);
        if full_path.is_file() && is_executable(&full_path).unwrap_or(false) {
            return Some(full_path.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &PathBuf) -> std::io::Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.permissions().mode() & 0o111 != 0)
}
