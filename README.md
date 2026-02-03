# Rust Mini-Shell

This repository contains a minimal POSIX-style shell implemented in Rust as part of CodeCrafters' "Build Your Own Shell" challenge. It features a readline-powered REPL, a small set of built-in commands, external command execution, pipelines, output redirection, and simple tab completion.

Challenge details: https://app.codecrafters.io/courses/shell/overview

## Overview

The shell starts an interactive loop, reads a line, parses it into one or more commands (a pipeline), and then executes either built-ins or external programs. It supports:
- Interactive prompt with history and completion
- Built-in commands: `cd`, `echo`, `exit`, `pwd`, `type`, `history`
- External commands resolved via `PATH` or absolute paths
- Pipelines (`cmd1 | cmd2 | ...`)
- Output redirection for stdout, stderr, and both together
- History persistence via `HISTFILE`

## Project Structure

- `src/main.rs`
  - Entry point. Sets up the readline editor, loads/saves history, drives the REPL loop, invokes parsing, orchestrates pipelines, wiring of stdin/stdout/stderr and delegating to built-ins or external processes.
- `src/parser.rs`
  - Tokenizer and parser for a single input line. Produces a vector of `ParsedCommand` structs forming a pipeline. Handles quoting rules, backslash escapes inside and outside quotes, pipe splitting, and output redirection targets/flags.
  - Constants used across the shell (prompt string, command names, environment variable names, file-descriptor tokens like `1`, `2`, and `&`).
  - Escape expansion helper used by `echo -e`.
- `src/commands.rs`
  - Implementations of built-in commands and the external command runner.
  - Built-ins implemented:
    - `cd [dir]` — changes directory. Defaults to `$HOME`. Interprets `~` as home.
    - `echo [-e] [args...]` — prints arguments; with `-e` expands `\n`, `\t`, `\r`, `\\`, `\0`, `\"`, `\'`.
    - `exit [code]` — terminates the shell with an optional numeric exit code (default 0).
    - `pwd` — prints the current working directory.
    - `type <name>` — reports whether `<name>` is a shell builtin or the full path of an external command.
    - `history [N] | -r <file> | -a <file> | -w <file>` — prints recent history, reads entries from a file, appends only new entries, or writes the full history respectively.
  - External command execution:
    - Resolved via `$PATH` using `search_executable`, or uses an absolute path if executable.
    - Supports capturing stdout/stderr when redirected, otherwise inherits the terminal streams.
    - For single commands, reads child pipes and forwards data; for pipelines, spawns chained processes and waits.
  - Output redirection helper: opens files in truncate or append mode and returns a writer when redirection is requested.
- `src/shell_helper.rs`
  - Glue code for `rustyline`: helper and completer implementations.
  - Tab-completion of built-ins and available `PATH` executables (deduped and sorted).

## Parsing and Features

- Quoting and escaping
  - Single quotes preserve literal text.
  - Double quotes allow certain backslash-escaped characters (e.g., `\"`, `\\`, ``\` ``, `$`, `!`).
  - Outside quotes, `\` escapes the next character.
- Pipelines
  - The input is split on unescaped, unquoted `|` into a sequence of `ParsedCommand`s.
- Redirection
  - `1> file` redirects stdout, `2> file` redirects stderr, `&> file` redirects both.
  - `>>` sets append mode; a single `>` truncates.
- History
  - Uses `rustyline` in-memory history. If `HISTFILE` is set, the file is loaded on startup and written back on exit. `history -a` appends only the new entries since the last write, `history -w` rewrites the whole file, and `history -r` loads entries from a file.

## Building and Running

Prerequisites: Rust toolchain (edition 2021; see `Cargo.toml` for `rust-version`).

- Run with Cargo:
  ```sh
  cargo run
  ```
- Or via the helper script used by CodeCrafters runners:
  ```sh
  ./your_program.sh
  ```

## Usage Examples

- External command:
  ```sh
  $ ls -la
  ```
- Pipeline:
  ```sh
  $ ls | grep rs | wc -l
  ```
- Redirection:
  ```sh
  $ echo hello > out.txt
  $ &> errors_and_output.log ls /no/such/path
  $ echo append >> out.txt
  ```
- Built-ins:
  ```sh
  $ pwd
  $ cd ~/projects
  $ echo -e "line1\nline2"
  $ type echo
  $ history 20
  ```

## Dependencies

Defined in `Cargo.toml`:
- `rustyline` — line editing, history, completion. Features enabled: `with-file-history`, `derive`.
- `os_pipe` — portable OS pipe creation used for pipeline wiring.

## Notes and Limitations

- This is an educational implementation focusing on clarity over complete POSIX compliance.
- Job control, environment variable expansion, globbing, subshells, and advanced redirection are not implemented.
- Behavior may differ from `bash`/`zsh` in edge cases, quoting/escaping rules, and error handling.

## License

See the repository license (if any) or treat this as example code for educational purposes.
