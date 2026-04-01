# Rust Mini-Shell

This repository contains a minimal POSIX-style shell implemented in Rust as part of CodeCrafters' "Build Your Own Shell" challenge. It features a readline-powered REPL, a small set of built-in commands, external command execution, pipelines, output redirection, and comprehensive tab completion.

Challenge details: https://app.codecrafters.io/courses/shell/overview

## Overview

The shell starts an interactive loop, reads a line, parses it into one or more commands (a pipeline), and then executes either built-ins or external programs. It supports:
- Interactive prompt with history and tab completion
- Built-in commands: `cd`, `echo`, `exit`, `pwd`, `type`, `history`, `jobs`
- External commands resolved via `PATH` or absolute paths
- Pipelines (`cmd1 | cmd2 | ...`)
- Output redirection for stdout, stderr, and both together
- Background execution with `&` and job control via `jobs`
- History persistence via `HISTFILE`
- Tab completion for commands, filenames, and nested paths

## Project Structure

- `src/main.rs`
  - Entry point. Sets up the readline editor, loads/saves history, drives the REPL loop, invokes parsing, orchestrates pipelines, wiring of stdin/stdout/stderr and delegating to built-ins or external processes. Manages background jobs: spawning them with `&`, tracking them in a `Vec<BackgroundJob>`, reaping finished jobs before each prompt, and recycling job numbers.
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
    - `jobs` — lists all background jobs with their job number, status (`Running`/`Done`), and command. Marks the most recent job with `+` and the second-most-recent with `-`. Finished jobs are removed from the list after being displayed.
  - External command execution:
    - Resolved via `$PATH` using `search_executable`, or uses an absolute path if executable.
    - Supports capturing stdout/stderr when redirected, otherwise inherits the terminal streams.
    - For single commands, reads child pipes and forwards data; for pipelines, spawns chained processes and waits.
  - Output redirection helper: opens files in truncate or append mode and returns a writer when redirection is requested.
- `src/shell_helper.rs`
  - Glue code for `rustyline`: helper and completer implementations.
  - `ShellHelper` struct integrating with rustyline's `Helper`, `Completer`, `Hinter`, and `Validator` traits.
  - `ShellCompleter` providing tab completion for:
    - Built-in commands and PATH executables
    - Filenames and directories in the current working directory
    - Nested path completion (e.g., `cat foo/bar/`)
  - `compute_lcp` function: computes the longest common prefix of matching entries for progressive completion.
  - `find_matching_entries` function: searches directories for entries matching a prefix, distinguishing files from directories.

## Parsing and Features

- Quoting and escaping
  - Single quotes preserve literal text.
  - Double quotes allow certain backslash-escaped characters (e.g., `\"`, `\\`, ``\` ``, `$`, `!`).
  - Outside quotes, `\` escapes the next character.
- Pipelines
  - The input is split on unescaped, unquoted `|` into a sequence of `ParsedCommand`s.
- Background execution
  - Appending `&` to a command runs it as a background job. The shell prints `[<job-id>] <pid>` and immediately returns to the prompt. Job IDs are the lowest available positive integers and are recycled when jobs finish.
- Redirection
  - `1> file` redirects stdout, `2> file` redirects stderr, `&> file` redirects both.
  - `>>` sets append mode; a single `>` truncates.
- History
  - Uses `rustyline` in-memory history. If `HISTFILE` is set, the file is loaded on startup and written back on exit. `history -a` appends only the new entries since the last write, `history -w` rewrites the whole file, and `history -r` loads entries from a file.

## Tab Completion

The shell provides comprehensive tab completion for commands and filenames:

### Command Completion
- Press TAB after typing a partial command to complete it
- Matches built-in commands and PATH executables (sorted alphabetically)
- Multiple matches displayed as a list; single match auto-completed with trailing space

### Filename Completion
- Press TAB after typing a partial filename to complete it
- Matches files and directories in the current working directory
- **Directories**: Completed with trailing `/`
- **Files**: Completed with trailing space

### Nested Path Completion
- Completion works in subdirectories
- Example: `cat foo/bar/<TAB>` completes files in `foo/bar/`
- Recursive directory traversal supported

### Multiple Match Handling
- **First TAB**: Rings bell (`\x07`) if no unique match exists
- **Second TAB**: Lists all matching entries sorted alphabetically (case-insensitive)
- Entries separated by two spaces for readability

### Longest Common Prefix (LCP) Completion
- When multiple matches share a common prefix longer than the current input, auto-completes to the LCP
- Allows progressive completion by typing more characters
- Example with files `xyz_dog/`, `xyz_dog_cow/`, `xyz_dog_cow_pig.txt`:
  - `xyz_<TAB>` → auto-completes to `xyz_dog` (LCP of all matches)
  - `xyz_dog_<TAB>` → auto-completes to `xyz_dog_cow` (LCP of remaining matches)
  - `xyz_dog_cow_<TAB>` → auto-completes to `xyz_dog_cow_pig.txt ` (with space, single match)
- The trailing `/` or space is only added when exactly one match remains

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
- Background jobs:
  ```sh
  $ sleep 10 &
  [1] 12345
  $ jobs
  [1]+  Running                 sleep 10 &
  $ jobs       # after sleep finishes
  [1]+  Done                    sleep 10
  ```
- Tab completion:
  ```sh
  $ ech<TAB>          # Completes to: echo
  $ cat fi<TAB>        # Completes to: filename.txt (with space)
  $ cat foo/<TAB>      # Completes to: foo/bar/ (directory with trailing slash)
  $ xyz_<TAB><TAB>     # First TAB rings bell, second lists all matches
  ```

## Dependencies

Defined in `Cargo.toml`:
- `rustyline` — line editing, history, completion. Features enabled: `with-file-history`, `derive`.
- `os_pipe` — portable OS pipe creation used for pipeline wiring.

## Notes and Limitations

- This is an educational implementation focusing on clarity over complete POSIX compliance.
- Basic job control is implemented: background execution (`&`), `jobs` listing, and automatic reaping. Foreground job resumption (`fg`/`bg`), signal forwarding, and `wait` are not implemented.
- Environment variable expansion, globbing, subshells, and advanced redirection are not implemented.
- Tab completion is limited to the current working directory and explicitly typed paths; it does not follow `$PATH` for filename completion.
- The completion system uses a simple LCP algorithm; it may not handle edge cases with Unicode filenames or complex path patterns.
- Behavior may differ from `bash`/`zsh` in edge cases, quoting/escaping rules, and error handling.

## License

See the repository license (if any) or treat this as example code for educational purposes.
