#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    let mut command = String::new();
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        command.clear();
        match io::stdin().read_line(&mut command) {
            Ok(bytes_read) => {
                let command = command.trim();
                if command.starts_with("exit") {
                    if bytes_read == 5 {
                        std::process::exit(0);
                    } else {
                        let exit_status = command[4..].trim().parse().unwrap_or(0);
                        std::process::exit(exit_status as i32);
                    }
                }
                eprintln!("{}: command not found", command);
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}
