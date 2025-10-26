#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    let mut command = String::new();
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        command.clear();
        match io::stdin().read_line(&mut command) {
            Ok(_) => {
                eprintln!("{}: command not found", command.trim());
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
}
