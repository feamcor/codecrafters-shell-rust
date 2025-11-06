use rustyline::completion::Completer;
use rustyline::completion::Pair;
use rustyline::config::{BellStyle, CompletionType, Config};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::{Completer, Context, Editor, Helper, Hinter, Validator};
use std::env::current_dir;
use std::env::set_current_dir;
use std::env::var;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::iter::Enumerate;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::vec::IntoIter;

const CHAR_BACKSLASH: char = '\\';
const CHAR_BACKTICK: char = '`';
const CHAR_CARRIAGE_RETURN: char = '\r';
const CHAR_EXCLAMATION_MARK: char = '!';
const CHAR_DOLLAR_SIGN: char = '$';
const CHAR_DOUBLE_QUOTE: char = '"';
const CHAR_GREATER_THAN: char = '>';
const CHAR_NEWLINE: char = '\n';
const CHAR_NULL: char = '\0';
const CHAR_PIPE: char = '|';
const CHAR_SINGLE_QUOTE: char = '\'';
const CHAR_TAB: char = '\t';
const COMMAND_CD: &str = "cd";
const COMMAND_ECHO: &str = "echo";
const COMMAND_ECHO_FLAG_EXPAND_ESCAPE: &str = "-e";
const COMMAND_EXIT: &str = "exit";
const COMMAND_PWD: &str = "pwd";
const COMMAND_TYPE: &str = "type";
const ENVIRONMENT_VARIABLE_HOME: &str = "HOME";
const ENVIRONMENT_VARIABLE_PATH: &str = "PATH";
const ENVIRONMENT_VARIABLE_PATH_DELIMITER: char = ':';
const HOME_DIRECTORY: &str = "~";
const SHELL_PROMPT: &str = "$ ";
const STDERR_FILE_DESCRIPTOR: char = '2';
const STDOUT_FILE_DESCRIPTOR: char = '1';
const STDOUT_STDERR_FILE_DESCRIPTOR: char = '&';

#[derive(Clone)]
struct OutputRedirection {
    file_name: Option<String>,
    append_to: bool,
}

#[derive(Clone)]
struct ParsedCommand {
    tokens: Option<Vec<String>>,
    stdout: OutputRedirection,
    stderr: OutputRedirection,
}

#[derive(Helper, Completer, Hinter, Validator)]
struct ShellHelper {
    #[rustyline(Completer)]
    completer: ShellCompleter,
}

impl Highlighter for ShellHelper {}

struct ShellCompleter {
    commands: Vec<String>,
}

impl ShellCompleter {
    fn new() -> Self {
        let mut commands = vec![
            COMMAND_CD.to_string(),
            COMMAND_ECHO.to_string(),
            COMMAND_EXIT.to_string(),
            COMMAND_PWD.to_string(),
            COMMAND_TYPE.to_string(),
        ];

        if let Ok(path_var) = var(ENVIRONMENT_VARIABLE_PATH) {
            for path_dir in path_var.split(ENVIRONMENT_VARIABLE_PATH_DELIMITER) {
                if let Ok(dir_entries) = std::fs::read_dir(path_dir) {
                    for dir_entry in dir_entries.flatten() {
                        if let Ok(entry_metadata) = dir_entry.metadata() {
                            if entry_metadata.is_file()
                                && (entry_metadata.permissions().mode() & 0o111 != 0)
                            {
                                if let Some(file_name) = dir_entry.file_name().into_string().ok() {
                                    commands.push(file_name)
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
}

impl Completer for ShellCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Self::Candidate>), ReadlineError> {
        if pos > 0 && line.chars().take(pos).any(|c| c.is_whitespace()) {
            return Ok((0, Vec::new()));
        }

        let (start, word) =
            rustyline::completion::extract_word(line, pos, None, |c| c.is_whitespace());

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let helper = ShellHelper {
        completer: ShellCompleter::new(),
    };

    let config = Config::builder()
        .completion_type(CompletionType::List)
        .bell_style(BellStyle::Audible)
        .build();

    let mut readline = Editor::with_config(config)?;
    readline.set_helper(Some(helper));

    'repl: loop {
        let input = match readline.readline(SHELL_PROMPT) {
            Ok(line) => line,
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

        let parsed_input = parse_input(input);
        match parsed_input {
            Some(parsed_commands) => {
                let pipeline_length = parsed_commands.len();
                let mut previous_child = None;
                let mut previous_output = None;
                for (index, parsed_command) in parsed_commands.into_iter().enumerate() {
                    let inherit_stdout = parsed_command.stdout.file_name.is_none();
                    let inherit_stderr = parsed_command.stderr.file_name.is_none();
                    let mut arguments = match parsed_command.tokens {
                        Some(tokens) => tokens.into_iter().enumerate(),
                        None => continue 'repl,
                    };
                    let mut stdout = get_output_redirection(parsed_command.stdout)
                        .unwrap_or(Box::new(io::stdout()));
                    let mut stderr = get_output_redirection(parsed_command.stderr)
                        .unwrap_or(Box::new(io::stderr()));
                    let (_, command) = arguments.next().unwrap();
                    match command.as_str() {
                        COMMAND_CD => {
                            command_cd(arguments, stdout, stderr);
                        }
                        COMMAND_ECHO => {
                            command_echo(arguments, stdout, stderr);
                        }
                        COMMAND_EXIT => {
                            command_exit(arguments, stdout, stderr);
                        }
                        COMMAND_PWD => {
                            command_pwd(arguments, stdout, stderr);
                        }
                        COMMAND_TYPE => {
                            command_type(arguments, stdout, stderr);
                        }
                        _ => {
                            if pipeline_length == 1 {
                                // there is only one command in the pipeline
                                if let Err(e) = run_executable(
                                    &command,
                                    arguments,
                                    Stdio::null(),
                                    &mut stdout,
                                    &mut stderr,
                                    inherit_stdout,
                                    inherit_stderr,
                                    None,
                                ) {
                                    writeln!(stderr, "Error: {:?}", e).unwrap_or_default();
                                }
                            } else if index == 0 {
                                // first command in the pipeline
                                if let Ok(mut spawned) = Command::new(&command)
                                    .args(arguments.map(|(_, argument)| argument))
                                    .stdin(Stdio::null())
                                    .stdout(Stdio::piped())
                                    .spawn()
                                {
                                    previous_output = spawned.stdout.take();
                                    previous_child = Some(spawned);
                                } else {
                                    writeln!(
                                        stderr,
                                        "Error: Failed to spawn child process {}",
                                        command
                                    )
                                    .unwrap_or_default();
                                }
                            } else if index < pipeline_length - 1 {
                                // middle command in the pipeline
                                if let Ok(mut spawned) = Command::new(&command)
                                    .args(arguments.map(|(_, argument)| argument))
                                    .stdin(Stdio::from(previous_output.take().unwrap()))
                                    .stdout(Stdio::piped())
                                    .spawn()
                                {
                                    if let Some(mut previous_child) = previous_child.take() {
                                        if let Err(e) = previous_child.wait() {
                                            writeln!(stderr, "Error: {:?}", e).unwrap_or_default();
                                        }
                                    }
                                    previous_output = spawned.stdout.take();
                                    previous_child = Some(spawned);
                                } else {
                                    writeln!(
                                        stderr,
                                        "Error: Failed to spawn child process {}",
                                        command
                                    )
                                    .unwrap_or_default();
                                }
                            } else {
                                // last command in the pipeline
                                if let Err(e) = run_executable(
                                    &command,
                                    arguments,
                                    Stdio::from(previous_output.take().unwrap()),
                                    &mut stdout,
                                    &mut stderr,
                                    inherit_stdout,
                                    inherit_stderr,
                                    previous_child.take(),
                                ) {
                                    writeln!(stderr, "Error: {:?}", e).unwrap_or_default();
                                }
                            }
                        }
                    }
                }
            }
            None => continue 'repl,
        }
    }

    Ok(())
}

fn command_exit(
    arguments: Enumerate<IntoIter<String>>,
    _stdout: Box<dyn Write>,
    _stderr: Box<dyn Write>,
) {
    let mut exit_status = 0;
    for (_index, argument) in arguments.take(1) {
        exit_status = argument.parse().unwrap_or(1);
    }
    std::process::exit(exit_status);
}

fn command_echo(
    arguments: Enumerate<IntoIter<String>>,
    mut stdout: Box<dyn Write>,
    _stderr: Box<dyn Write>,
) {
    let mut expand_escape = false;
    for (index, argument) in arguments {
        if index == 1 && argument == COMMAND_ECHO_FLAG_EXPAND_ESCAPE {
            expand_escape = true;
            continue;
        }
        if !(expand_escape && index == 2) && index > 1 {
            write!(stdout, " ").unwrap_or_default();
        }
        if expand_escape {
            write!(stdout, "{}", expand_escape_sequences(&argument)).unwrap_or_default();
        } else {
            write!(stdout, "{}", argument).unwrap_or_default();
        };
    }
    writeln!(stdout).unwrap_or_default();
    stdout.flush().unwrap_or_default();
}

fn command_type(
    arguments: Enumerate<IntoIter<String>>,
    mut stdout: Box<dyn Write>,
    mut stderr: Box<dyn Write>,
) {
    for (_index, argument) in arguments.take(1) {
        match argument.as_str() {
            COMMAND_CD | COMMAND_ECHO | COMMAND_EXIT | COMMAND_PWD | COMMAND_TYPE => {
                writeln!(stdout, "{argument} is a shell builtin").unwrap_or_default()
            }
            _ => match search_executable(&*argument) {
                Some(full_path_to_executable) => {
                    writeln!(stdout, "{argument} is {full_path_to_executable}").unwrap_or_default()
                }
                None => writeln!(stderr, "{argument}: not found").unwrap_or_default(),
            },
        }
    }
    stdout.flush().unwrap_or_default();
    stderr.flush().unwrap_or_default();
}

fn is_executable(full_path_to_executable: &PathBuf) -> io::Result<bool> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(full_path_to_executable)?;
    Ok(metadata.permissions().mode() & 0o111 != 0)
}

fn search_executable(command: &str) -> Option<String> {
    let path_var = var(ENVIRONMENT_VARIABLE_PATH).unwrap_or(String::new());
    for path_dir in path_var.split(ENVIRONMENT_VARIABLE_PATH_DELIMITER) {
        let full_path_to_executable = Path::new(path_dir).join(command);
        if full_path_to_executable.is_file()
            && is_executable(&full_path_to_executable).unwrap_or(false)
        {
            return Some(full_path_to_executable.to_string_lossy().into_owned());
        }
    }
    None
}

fn run_executable(
    command: &str,
    arguments: Enumerate<IntoIter<String>>,
    stdin: Stdio,
    stdout: &mut Box<dyn Write>,
    stderr: &mut Box<dyn Write>,
    inherit_stdout: bool,
    inherit_stderr: bool,
    child: Option<Child>,
) -> Result<(), io::Error> {
    let command_path = if Path::new(command).is_absolute() {
        Some(command.to_string())
    } else {
        search_executable(command)
    };
    match command_path {
        Some(_) => {
            if inherit_stdout && inherit_stderr {
                let mut spawned = Command::new(command)
                    .args(arguments.map(|(_, argument)| argument))
                    .stdin(stdin)
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .spawn()?;

                if let Some(mut previous_child) = child {
                    let _status = previous_child.wait();
                }

                let _status = spawned.wait();
            } else {
                let output = Command::new(command)
                    .args(arguments.map(|(_, argument)| argument))
                    .stdin(stdin)
                    .output();

                if let Some(mut previous_child) = child {
                    let _status = previous_child.wait();
                }

                match output {
                    Ok(output) => {
                        if !output.stdout.is_empty() {
                            stdout.write_all(&output.stdout)?;
                        }
                        if !output.stderr.is_empty() {
                            stderr.write_all(&output.stderr)?;
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        None => writeln!(stderr, "{command}: command not found")?,
    }

    Ok(())
}

fn command_pwd(
    _arguments: Enumerate<IntoIter<String>>,
    mut stdout: Box<dyn Write>,
    mut stderr: Box<dyn Write>,
) {
    let current_directory = current_dir().unwrap();
    writeln!(stdout, "{}", current_directory.to_string_lossy()).unwrap_or_default();
    stdout.flush().unwrap_or_default();
    stderr.flush().unwrap_or_default();
}

fn command_cd(
    mut arguments: Enumerate<IntoIter<String>>,
    mut stdout: Box<dyn Write>,
    mut stderr: Box<dyn Write>,
) {
    let home_directory = var(ENVIRONMENT_VARIABLE_HOME).unwrap_or(String::new());
    let argument = arguments.next();
    let directory = match argument {
        Some((_index, path)) => match path.as_str() {
            HOME_DIRECTORY => home_directory,
            _ => path,
        },
        None => home_directory,
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

fn parse_input(input: &str) -> Option<Vec<ParsedCommand>> {
    let mut pipeline = Vec::new();
    let mut characters = input.trim().chars().peekable();

    'pipeline: loop {
        let mut tokens = Vec::new();
        let mut stdout: OutputRedirection = OutputRedirection {
            file_name: None,
            append_to: false,
        };
        let mut stderr: OutputRedirection = OutputRedirection {
            file_name: None,
            append_to: false,
        };

        let mut current_token = String::new();
        let mut in_single_quotes = false;
        let mut in_double_quotes = false;
        let mut escape_next_char = false;
        let mut in_stdout_redirection = false;
        let mut in_stderr_redirection = false;

        while let Some(character) = characters.next() {
            match character {
                CHAR_SINGLE_QUOTE if !escape_next_char => {
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
                }

                CHAR_DOUBLE_QUOTE if !escape_next_char => {
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
                }

                CHAR_BACKSLASH if !escape_next_char => {
                    if in_single_quotes {
                        current_token.push(character);
                    } else if in_double_quotes {
                        if let Some(next_character) = characters.peek() {
                            match *next_character {
                                CHAR_BACKTICK
                                | CHAR_BACKSLASH
                                | CHAR_DOLLAR_SIGN
                                | CHAR_DOUBLE_QUOTE
                                | CHAR_EXCLAMATION_MARK => escape_next_char = true,
                                _ => current_token.push(character),
                            }
                        }
                    } else {
                        escape_next_char = true;
                    }
                }

                CHAR_PIPE if !escape_next_char && !in_single_quotes && !in_double_quotes => {
                    pipeline.push(ParsedCommand {
                        tokens: if tokens.is_empty() {
                            None
                        } else {
                            Some(tokens)
                        },
                        stdout,
                        stderr,
                    });
                    continue 'pipeline;
                }

                file_descriptor
                    if file_descriptor == STDOUT_FILE_DESCRIPTOR && current_token.is_empty() =>
                {
                    if let Some(next_character) = characters.peek() {
                        if *next_character == CHAR_GREATER_THAN {
                            in_stdout_redirection = true;
                            characters.next();
                        } else {
                            current_token.push(file_descriptor);
                        }
                    }
                }

                file_descriptor
                    if file_descriptor == STDERR_FILE_DESCRIPTOR && current_token.is_empty() =>
                {
                    if let Some(next_character) = characters.peek() {
                        if *next_character == CHAR_GREATER_THAN {
                            in_stderr_redirection = true;
                            characters.next();
                        } else {
                            current_token.push(file_descriptor);
                        }
                    }
                }

                file_descriptor
                    if file_descriptor == STDOUT_STDERR_FILE_DESCRIPTOR
                        && current_token.is_empty() =>
                {
                    if let Some(next_character) = characters.peek() {
                        if *next_character == CHAR_GREATER_THAN {
                            in_stdout_redirection = true;
                            in_stderr_redirection = true;
                            characters.next();
                        } else {
                            current_token.push(file_descriptor);
                        }
                    }
                }

                redirect_operator
                    if redirect_operator == CHAR_GREATER_THAN
                        && !in_stdout_redirection
                        && !in_stderr_redirection
                        && !escape_next_char
                        && !in_single_quotes
                        && !in_double_quotes =>
                {
                    in_stdout_redirection = true;
                }

                redirect_operator if redirect_operator == CHAR_GREATER_THAN => {
                    stdout.append_to = in_stdout_redirection;
                    stderr.append_to = in_stderr_redirection;
                }

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
                }

                _ => {
                    current_token.push(character);
                    escape_next_char = false;
                }
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

        pipeline.push(ParsedCommand {
            tokens: if tokens.is_empty() {
                None
            } else {
                Some(tokens)
            },
            stdout,
            stderr,
        });

        break;
    }

    if pipeline.is_empty() {
        None
    } else {
        Some(pipeline)
    }
}

fn get_output_redirection(output: OutputRedirection) -> Option<Box<dyn Write>> {
    match output.file_name {
        Some(file_name) => {
            let file = OpenOptions::new()
                .append(output.append_to)
                .write(true)
                .create(true)
                .open(file_name);
            match file {
                Ok(file) => Some(Box::new(io::BufWriter::new(file))),
                Err(e) => {
                    eprintln!("Error: {e}");
                    None
                }
            }
        }
        None => None,
    }
}

fn expand_escape_sequences(string: &str) -> String {
    let mut result = String::with_capacity(string.len());
    let mut characters = string.chars();

    while let Some(character) = characters.next() {
        if character == CHAR_BACKSLASH {
            if let Some(next) = characters.next() {
                match next {
                    'n' => result.push(CHAR_NEWLINE),
                    't' => result.push(CHAR_TAB),
                    'r' => result.push(CHAR_CARRIAGE_RETURN),
                    CHAR_BACKSLASH => result.push(CHAR_BACKSLASH),
                    '0' => result.push(CHAR_NULL),
                    CHAR_DOUBLE_QUOTE => result.push(CHAR_DOUBLE_QUOTE),
                    CHAR_SINGLE_QUOTE => result.push(CHAR_SINGLE_QUOTE),
                    _ => {
                        result.push(CHAR_BACKSLASH);
                        result.push(next);
                    }
                }
            }
        } else {
            result.push(character);
        }
    }

    result
}
