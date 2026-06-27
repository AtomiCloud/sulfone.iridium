use inquire::Confirm;
use std::error::Error;
use std::path::Path;
use std::process::Command;

/// Result of a command execution session
#[derive(Debug, Clone)]
pub struct CommandExecutionResult {
    /// Total commands count
    pub total: usize,
    /// Succeeded count
    pub succeeded: usize,
    /// Failed count
    pub failed: usize,
    /// Indices of failed commands (0-indexed)
    pub failed_indices: Vec<usize>,
    /// Whether execution was aborted by user
    pub aborted: bool,
}

impl CommandExecutionResult {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            succeeded: 0,
            failed: 0,
            failed_indices: Vec::new(),
            aborted: false,
        }
    }

    /// Returns true if all commands succeeded
    pub fn all_succeeded(&self) -> bool {
        self.failed == 0 && !self.aborted
    }
}

/// Command executor for running template commands after composition
pub struct CommandExecutor;

impl CommandExecutor {
    /// Execute a list of commands sequentially in the given working directory.
    ///
    /// Shows an approval prompt before execution, then executes each command
    /// in order. On failure, prompts user to continue or abort.
    pub fn execute_commands(
        commands: &[String],
        working_dir: &Path,
    ) -> Result<CommandExecutionResult, Box<dyn Error + Send>> {
        // Filter out blank/whitespace-only commands as a safety net.
        // The registry mapper strips these on push, but `cyanprint try`
        // feeds local cyan.yaml entries directly into this executor.
        let commands: Vec<&str> = commands
            .iter()
            .map(String::as_str)
            .filter(|cmd| !cmd.trim().is_empty())
            .collect();

        if commands.is_empty() {
            return Ok(CommandExecutionResult::new(0));
        }

        // Approval prompt
        println!("\n⚠️  {} command(s) will be executed:", commands.len());
        for (i, cmd) in commands.iter().copied().enumerate() {
            println!("  {}. {}", i + 1, cmd);
        }

        let proceed = Confirm::new("Do you want to proceed with execution?")
            .with_default(false)
            .prompt()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        if !proceed {
            let mut result = CommandExecutionResult::new(commands.len());
            result.aborted = true;
            println!("⏹️  Command execution cancelled by user");
            return Ok(result);
        }

        println!();

        let mut result = CommandExecutionResult::new(commands.len());

        for (i, cmd) in commands.iter().copied().enumerate() {
            print!("  Running command {}/{}: ", i + 1, commands.len());
            println!("{cmd}");

            let exit_status = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(working_dir)
                .spawn()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
                .wait()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

            if exit_status.success() {
                println!("    ✅ Success");
                result.succeeded += 1;
            } else {
                let exit_code = exit_status.code().unwrap_or(-1);
                println!("    ❌ Failed (exit code: {exit_code})");
                result.failed += 1;
                result.failed_indices.push(i);

                // Ask user whether to continue
                let continue_prompt = Confirm::new(&format!(
                    "Command failed (exit code: {exit_code}). Continue executing remaining commands?",
                ))
                .with_default(false)
                .prompt()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

                if !continue_prompt {
                    result.aborted = true;
                    println!("⏹️  Command execution aborted by user");
                    break;
                }
            }
        }

        // Print summary
        println!("\n📊 Command execution summary:");
        println!("  Total: {}", result.total);
        println!("  Succeeded: {}", result.succeeded);
        println!("  Failed: {}", result.failed);
        if result.aborted {
            println!("  Status: Aborted");
        } else if result.all_succeeded() {
            println!("  Status: All commands succeeded");
        } else {
            println!("  Status: Completed with failures");
        }

        Ok(result)
    }

    /// Execute commands non-interactively (no approval prompt).
    ///
    /// Runs all commands, accumulating failures in the result. The caller
    /// should check `all_succeeded()` to determine overall outcome.
    ///
    /// `suppress_stdout` controls child stdio. When `true` (headless mode)
    /// each child's stdout is CAPTURED rather than inherited, and both its stdout and
    /// stderr are forwarded to the PARENT's stderr — so a post-template command that
    /// prints (e.g. `echo …`) can never pollute the process's stdout, which in headless
    /// mode must carry only the final JSON envelope. When `false` (the interactive test
    /// runner) stdio is inherited verbatim so the user sees command output live, exactly
    /// as before.
    pub fn execute_commands_non_interactive(
        commands: &[String],
        working_dir: &Path,
        suppress_stdout: bool,
    ) -> Result<CommandExecutionResult, Box<dyn Error + Send>> {
        let commands: Vec<&str> = commands
            .iter()
            .map(String::as_str)
            .filter(|cmd| !cmd.trim().is_empty())
            .collect();

        if commands.is_empty() {
            return Ok(CommandExecutionResult::new(0));
        }

        let mut result = CommandExecutionResult::new(commands.len());

        for (i, cmd) in commands.iter().copied().enumerate() {
            let mut command = Command::new("sh");
            command.arg("-c").arg(cmd).current_dir(working_dir);

            let exit_status = if suppress_stdout {
                // Capture stdout+stderr (no inherited fds → nothing reaches the process's
                // stdout), then forward both to STDERR so the output stays visible without
                // breaking the machine-readable stdout contract. `output()` reads both
                // pipes to completion, so there is no fill-the-buffer deadlock.
                let out = command
                    .output()
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                if !out.stdout.is_empty() {
                    eprint!("{}", String::from_utf8_lossy(&out.stdout));
                }
                if !out.stderr.is_empty() {
                    eprint!("{}", String::from_utf8_lossy(&out.stderr));
                }
                out.status
            } else {
                command
                    .spawn()
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
                    .wait()
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
            };

            if exit_status.success() {
                result.succeeded += 1;
            } else {
                result.failed += 1;
                result.failed_indices.push(i);
            }
        }

        Ok(result)
    }

    /// Execute commands in the mode appropriate to the invocation.
    ///
    /// In headless mode this routes to [`Self::execute_commands_non_interactive`] —
    /// no `inquire::Confirm` approval prompt (which would block/error on a non-TTY)
    /// and no progress `println!`s that would pollute the JSON stdout
    /// contract. In interactive mode it uses the approval-prompting
    /// [`Self::execute_commands`] so the user-visible behavior is unchanged.
    pub fn execute_commands_for_mode(
        commands: &[String],
        working_dir: &Path,
        headless: bool,
    ) -> Result<CommandExecutionResult, Box<dyn Error + Send>> {
        if headless {
            // Suppress child stdout (forward to stderr) so post-template command
            // output never pollutes the JSON envelope on stdout.
            Self::execute_commands_non_interactive(commands, working_dir, true)
        } else {
            Self::execute_commands(commands, working_dir)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_execute_empty_commands_returns_immediately() {
        let temp_dir = TempDir::new().unwrap();
        let result = CommandExecutor::execute_commands(&[], temp_dir.path()).unwrap();
        assert_eq!(result.total, 0);
        assert_eq!(result.succeeded, 0);
        assert_eq!(result.failed, 0);
        assert!(!result.aborted);
    }

    // Note: Tests that require interactive Confirm prompt cannot run in non-TTY environments
    // (e.g., CI, cargo test). These are validated manually or via integration tests.
    // The CommandExecutionResult struct tests below verify the result handling logic.

    #[test]
    fn test_execution_result_all_succeeded() {
        let result = CommandExecutionResult {
            total: 5,
            succeeded: 5,
            failed: 0,
            failed_indices: vec![],
            aborted: false,
        };
        assert!(result.all_succeeded());
    }

    #[test]
    fn test_execution_result_with_failures() {
        let result = CommandExecutionResult {
            total: 5,
            succeeded: 3,
            failed: 2,
            failed_indices: vec![1, 3],
            aborted: false,
        };
        assert!(!result.all_succeeded());
    }

    #[test]
    fn test_execution_result_aborted() {
        let result = CommandExecutionResult {
            total: 5,
            succeeded: 2,
            failed: 1,
            failed_indices: vec![1],
            aborted: true,
        };
        assert!(!result.all_succeeded());
    }

    // NFC2: in headless mode the command executor routes to
    // the NON-INTERACTIVE path (`execute_commands_non_interactive`), which has no
    // `inquire::Confirm` prompt and no `println!`/`print!` — so the JSON-envelope
    // stdout contract holds on the done path. `execute_commands_for_mode` is the single
    // dispatch point every command's done path uses, so asserting its headless routing
    // here covers the done-path command execution surface for create/update/try.
    //
    // This proves the routing is headless-aware at runtime (the non-interactive path
    // returns a normal result without ever touching a TTY/Confirm), complementing the
    // source-level NFC2 grep (every headless-reachable `println!` is gated) captured in
    // the source-level NFC2 evidence.
    #[test]
    fn headless_mode_routes_to_non_interactive_executor() {
        let temp_dir = TempDir::new().unwrap();
        // Headless mode runs the no-op command to completion with no prompt and no abort
        // (the non-interactive path never sets `aborted`).
        let result =
            CommandExecutor::execute_commands_for_mode(&[":".to_string()], temp_dir.path(), true)
                .expect("headless execution must succeed without a TTY");
        assert_eq!(result.total, 1);
        assert!(
            result.all_succeeded(),
            "headless command execution must succeed (no-op command)"
        );
        assert!(
            !result.aborted,
            "headless path can never abort (no Confirm prompt exists)"
        );
    }

    // NFC2: a post-template command that writes to stdout must still RUN under the
    // headless (suppress_stdout) path — its output is captured/forwarded to stderr, never
    // emitted on the process's stdout. We prove execution via a side-effect file (the
    // command both echoes to stdout AND writes a marker), and assert success. The
    // stdout-hygiene itself is structural: `suppress_stdout` uses `Command::output()`
    // (captured pipes), so no child fd is ever connected to the parent's stdout.
    #[test]
    fn headless_suppress_runs_stdout_writing_command() {
        let temp_dir = TempDir::new().unwrap();
        let marker = temp_dir.path().join("marker.txt");
        let cmd = format!(
            "echo polluting-stdout-output; echo done > {}",
            marker.display()
        );
        let result =
            CommandExecutor::execute_commands_non_interactive(&[cmd], temp_dir.path(), true)
                .expect("stdout-writing command must run under the suppress path");
        assert!(
            result.all_succeeded(),
            "the command must execute successfully even with stdout suppressed"
        );
        assert!(
            marker.exists(),
            "the command actually ran (side-effect marker written)"
        );
    }

    // A post-template command that EXITS NON-ZERO under the headless path must
    // surface as a result where `all_succeeded()` is false (and never `aborted`). This is
    // exactly the predicate every headless done path now checks — a false value converts
    // the run into an error envelope / exit 1 instead of `done` / exit 0. Without this
    // guard a failed command would be silently ignored by the caller (the non-interactive
    // path returns Ok and never sets `aborted`).
    #[test]
    fn headless_non_zero_command_is_not_all_succeeded() {
        let temp_dir = TempDir::new().unwrap();
        let result = CommandExecutor::execute_commands_for_mode(
            &["false".to_string()],
            temp_dir.path(),
            true,
        )
        .expect("the non-interactive path returns Ok even on command failure");
        assert!(!result.aborted, "the non-interactive path never aborts");
        assert_eq!(
            result.failed, 1,
            "the failing command is recorded as failed"
        );
        assert!(
            !result.all_succeeded(),
            "a failed command must NOT report all_succeeded — this is the predicate the \
             headless done paths check to convert the run into an error / exit 1"
        );
    }

    // Mixed: when some commands succeed and one fails, the result is still NOT
    // `all_succeeded()` — the partial failure must not be masked by the successes.
    #[test]
    fn headless_partial_failure_is_not_all_succeeded() {
        let temp_dir = TempDir::new().unwrap();
        let result = CommandExecutor::execute_commands_for_mode(
            &[":".to_string(), "false".to_string(), ":".to_string()],
            temp_dir.path(),
            true,
        )
        .expect("the non-interactive path returns Ok even with a partial failure");
        assert_eq!(result.succeeded, 2);
        assert_eq!(result.failed, 1);
        assert!(
            !result.all_succeeded(),
            "a partial failure must not be reported as all_succeeded"
        );
    }
}
