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

    /// Execute commands non-interactively (no approval prompt, fail immediately on error).
    ///
    /// Used by the test runner where commands should always execute without user interaction.
    pub fn execute_commands_non_interactive(
        commands: &[String],
        working_dir: &Path,
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
            let exit_status = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(working_dir)
                .spawn()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
                .wait()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

            if exit_status.success() {
                result.succeeded += 1;
            } else {
                result.failed += 1;
                result.failed_indices.push(i);
            }
        }

        Ok(result)
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
}
