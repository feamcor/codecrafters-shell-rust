#[allow(unused_imports)]
use std::io::{self, Write};
use std::iter::Enumerate;
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
                _ => println!("{argument}: not found"),
            }
        }
        break;
    }
}