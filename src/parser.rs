pub const CHAR_BACKSLASH: char = '\\';
pub const CHAR_BACKTICK: char = '`';
pub const CHAR_CARRIAGE_RETURN: char = '\r';
pub const CHAR_EXCLAMATION_MARK: char = '!';
pub const CHAR_DOLLAR_SIGN: char = '$';
pub const CHAR_DOUBLE_QUOTE: char = '"';
pub const CHAR_GREATER_THAN: char = '>';
pub const CHAR_NEWLINE: char = '\n';
pub const CHAR_NULL: char = '\0';
pub const CHAR_PIPE: char = '|';
pub const CHAR_SINGLE_QUOTE: char = '\'';
pub const CHAR_TAB: char = '\t';
pub const COMMAND_CD: &str = "cd";
pub const COMMAND_ECHO: &str = "echo";
pub const COMMAND_ECHO_FLAG_EXPAND_ESCAPE: &str = "-e";
pub const COMMAND_EXIT: &str = "exit";
pub const COMMAND_PWD: &str = "pwd";
pub const COMMAND_TYPE: &str = "type";
pub const COMMAND_HISTORY: &str = "history";
pub const ENVIRONMENT_VARIABLE_HOME: &str = "HOME";
pub const ENVIRONMENT_VARIABLE_PATH: &str = "PATH";
pub const ENVIRONMENT_VARIABLE_PATH_DELIMITER: char = ':';
pub const HOME_DIRECTORY: &str = "~";
pub const SHELL_PROMPT: &str = "$ ";
pub const STDERR_FILE_DESCRIPTOR: char = '2';
pub const STDOUT_FILE_DESCRIPTOR: char = '1';
pub const STDOUT_STDERR_FILE_DESCRIPTOR: char = '&';

#[derive(Clone, Debug)]
pub struct OutputRedirection {
    pub file_name: Option<String>,
    pub append_to: bool,
}

#[derive(Clone, Debug)]
pub struct ParsedCommand {
    pub tokens: Option<Vec<String>>,
    pub stdout: OutputRedirection,
    pub stderr: OutputRedirection,
}

pub fn expand_escape_sequences(string: &str) -> String {
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

pub fn parse_input(input: &str) -> Option<Vec<ParsedCommand>> {
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
                    } else if let Some(next_character) = characters.peek() {
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

                CHAR_DOUBLE_QUOTE if !escape_next_char => {
                    if current_token.is_empty() {
                        in_single_quotes = false;
                        in_double_quotes = true;
                    } else if let Some(next_character) = characters.peek() {
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
                    } else {
                        current_token.push(file_descriptor);
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
                    } else {
                        current_token.push(file_descriptor);
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
                    } else {
                        current_token.push(file_descriptor);
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
