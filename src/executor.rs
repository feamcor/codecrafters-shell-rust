use crate::commands::dispatch_builtin;
use crate::commands::get_redirection;
use crate::commands::is_executable;
use crate::commands::run_executable;
use crate::commands::BuiltinAction;
use crate::jobs::JobManager;
use crate::parser::ParsedCommand;
use crate::parser::COMMAND_CD;
use crate::parser::COMMAND_ECHO;
use crate::parser::COMMAND_EXIT;
use crate::parser::COMMAND_HISTORY;
use crate::parser::COMMAND_JOBS;
use crate::parser::COMMAND_PWD;
use crate::parser::COMMAND_TYPE;
use rustyline::Editor;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;

pub struct ShellContext<'a, H: rustyline::Helper, I: rustyline::history::History> {
    pub editor: &'a mut Editor<H, I>,
    pub last_appended_index: &'a mut usize,
}

#[allow(clippy::too_many_lines)]
pub fn execute_pipeline<H: rustyline::Helper, I: rustyline::history::History>(
    pipeline: Vec<ParsedCommand>,
    job_mgr: &mut JobManager,
    ctx: &mut ShellContext<'_, H, I>,
) -> io::Result<BuiltinAction> {
    use crate::commands::search_executable;

    let pipeline_length = pipeline.len();
    let mut children: Vec<Child> = Vec::new();
    let mut previous_output: Option<os_pipe::PipeReader> = None;

    for (current_index, current_command) in pipeline.into_iter().enumerate() {
        let arguments_vec: Vec<String> = current_command.tokens.clone().unwrap_or_default();
        let mut arguments = arguments_vec.into_iter().enumerate();

        let (stdin_builtin, stdin_external) = if let Some(output) = previous_output.take() {
            let output_for_external = output.try_clone()?;
            (Box::new(output) as Box<dyn Read>, Stdio::from(output_for_external))
        } else {
            (Box::new(io::empty()) as Box<dyn Read>, Stdio::null())
        };

        let (stdout_builtin, stdout_external, new_previous_output) = if current_index < pipeline_length - 1 {
            let (reader, writer) = os_pipe::pipe()?;
            let writer_for_external = writer.try_clone()?;
            (
                Box::new(writer) as Box<dyn Write>,
                Stdio::from(writer_for_external),
                Some(reader),
            )
        } else {
            let stdout = get_redirection(current_command.stdout.clone()).unwrap_or(Box::new(io::stdout()));
            (stdout, Stdio::inherit(), None)
        };
        previous_output = new_previous_output;

        let mut stderr_builtin = get_redirection(current_command.stderr.clone()).unwrap_or(Box::new(io::stderr()));

        let Some((_, command)) = arguments.next() else {
            return Ok(BuiltinAction::Continue);
        };

        // Check if it's a built-in first (no resource consumption).
        let is_builtin = matches!(
            command.as_str(),
            COMMAND_CD | COMMAND_ECHO | COMMAND_EXIT | COMMAND_PWD | COMMAND_TYPE | COMMAND_HISTORY | COMMAND_JOBS
        );

        if is_builtin {
            // dispatch_builtin always returns Some for known built-ins.
            let action = dispatch_builtin(
                &command,
                arguments,
                stdin_builtin,
                stdout_builtin,
                stderr_builtin,
                ctx.editor,
                ctx.last_appended_index,
                job_mgr,
            )
            .unwrap_or(BuiltinAction::Continue);
            if let BuiltinAction::Exit(code) = action {
                return Ok(BuiltinAction::Exit(code));
            }
            continue;
        }

        // Resolve external command path.
        let path = if let Some(p) = search_executable(&command) {
            p
        } else if Path::new(&command).is_absolute() && is_executable(&PathBuf::from(&command)).unwrap_or(false) {
            command.clone()
        } else {
            let _ = writeln!(stderr_builtin, "{command}: command not found");
            continue;
        };

        if pipeline_length == 1 {
            let mut stdout_builtin = stdout_builtin;
            match run_executable(
                &path,
                &command,
                arguments,
                stdin_external,
                &mut stdout_builtin,
                &mut stderr_builtin,
                current_command.stdout.file_name.is_none(),
                current_command.stderr.file_name.is_none(),
                None,
            ) {
                Ok(child) => {
                    if current_command.background {
                        let cmd_str = current_command.tokens.as_ref().map(|t| t.join(" ")).unwrap_or_default();
                        job_mgr.add(child, cmd_str);
                    } else {
                        let mut child = child;
                        let _ = child.wait();
                    }
                }
                Err(e) => {
                    let _ = writeln!(stderr_builtin, "Error: {e:?}");
                }
            }
        } else {
            // Pipeline case
            if let Ok(spawned) = Command::new(&path)
                .arg0(&command)
                .args(arguments.map(|(_, arg)| arg))
                .stdin(stdin_external)
                .stdout(stdout_external)
                .spawn()
            {
                children.push(spawned);
            } else {
                let _ = writeln!(stderr_builtin, "Error: Failed to spawn child process {command}");
            }
        }
    }

    for mut child in children {
        let _ = child.wait();
    }

    Ok(BuiltinAction::Continue)
}
