//! Test validation and snapshot comparison.
//!
//! This module provides:
//! - Validate command execution
//! - Snapshot comparison between output and expected directories
//! - File-level comparison results

use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Result of running a single validate command.
///
/// Contains the command that was run, its success status, and any output.
#[derive(Debug, Clone)]
pub struct ValidateResult {
    /// The command that was executed
    pub command: String,

    /// Whether the command succeeded (exit code 0)
    pub passed: bool,

    /// Stdout from the command
    pub stdout: String,

    /// Stderr from the command
    pub stderr: String,

    /// Exit code of the command
    pub exit_code: Option<i32>,
}

/// Result of comparing a single file.
///
/// Contains information about whether the file matched and any differences.
#[derive(Debug, Clone)]
pub struct FileComparisonResult {
    /// Relative path of the file (from comparison root)
    pub path: String,

    /// Whether the files matched
    pub matched: bool,

    /// Type of mismatch (if any)
    pub mismatch_type: Option<String>,

    /// Detailed diff or error message
    pub details: Option<String>,
}

/// Result of comparing two directories for snapshot testing.
///
/// Contains overall match status and per-file results.
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Overall match status
    pub matched: bool,

    /// Files that matched
    pub matched_files: Vec<String>,

    /// Files that didn't match
    pub mismatched_files: Vec<FileComparisonResult>,

    /// Files present in actual but missing from expected
    pub extra_files: Vec<String>,

    /// Files present in expected but missing from actual
    pub missing_files: Vec<String>,

    /// Binary files that were skipped
    pub skipped_binary_files: Vec<String>,
}

/// Run validate commands against an output directory.
///
/// Executes each shell command in the list and captures its output.
///
/// # Arguments
///
/// * `output_dir` - Directory where the command will run
/// * `commands` - List of shell commands to execute
///
/// # Returns
///
/// Returns a vector of [`ValidateResult`] for each command executed.
///
/// # Errors
///
/// Returns an error if a command cannot be spawned (but not if it fails).
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::validation::run_validate_commands;
///
/// let results = run_validate_commands("./output", &vec![
///     "test -f package.json".to_string(),
///     "grep -q 'version' package.json".to_string(),
/// ]).unwrap();
/// ```
pub fn run_validate_commands(
    output_dir: &str,
    commands: &[String],
) -> Result<Vec<ValidateResult>, Box<dyn Error + Send>> {
    let mut results = Vec::new();

    for command in commands {
        let result = run_single_command(output_dir, command)?;
        results.push(result);
    }

    Ok(results)
}

/// Run a single shell command and capture its output.
///
/// Helper function for [`run_validate_commands`].
fn run_single_command(
    output_dir: &str,
    command: &str,
) -> Result<ValidateResult, Box<dyn Error + Send>> {
    use std::process::Command;

    // Run command through shell to handle quoted arguments properly
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(Path::new(output_dir))
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let passed = output.status.success();
            let exit_code = output.status.code();

            Ok(ValidateResult {
                command: command.to_string(),
                passed,
                stdout: stdout.to_string(),
                stderr: stderr.to_string(),
                exit_code,
            })
        }
        Err(e) => {
            // Command failed to spawn (not execution failure)
            Ok(ValidateResult {
                command: command.to_string(),
                passed: false,
                stdout: String::new(),
                stderr: e.to_string(),
                exit_code: None,
            })
        }
    }
}

/// Compare two directories for snapshot testing.
///
/// Walks both directory trees and compares files:
/// - `.json` files: deep JSON comparison (handles field order)
/// - Other files: trimmed string comparison
/// - Binary files: skipped (reported separately)
///
/// # Arguments
///
/// * `actual_dir` - Path to actual output directory
/// * `expected_dir` - Path to expected snapshot directory
///
/// # Returns
///
/// Returns a [`ComparisonResult`] with details about matched/mismatched files.
///
/// # Errors
///
/// Returns an error if:
/// - Cannot read directory contents
/// - Cannot read file contents
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::validation::compare_directories;
///
/// let result = compare_directories("./output", "./snapshots/expected").unwrap();
/// println!("Matched: {}", result.matched);
/// ```
pub fn compare_directories(
    actual_dir: &str,
    expected_dir: &str,
) -> Result<ComparisonResult, Box<dyn Error + Send>> {
    let actual_path = PathBuf::from(actual_dir);
    let expected_path = PathBuf::from(expected_dir);

    // Check if directories exist
    let actual_exists = actual_path.exists();
    let expected_exists = expected_path.exists();

    if !actual_exists && !expected_exists {
        // Both don't exist - consider it a match (empty test)
        return Ok(ComparisonResult {
            matched: true,
            matched_files: Vec::new(),
            mismatched_files: Vec::new(),
            extra_files: Vec::new(),
            missing_files: Vec::new(),
            skipped_binary_files: Vec::new(),
        });
    }

    if !actual_exists {
        return Ok(ComparisonResult {
            matched: false,
            matched_files: Vec::new(),
            mismatched_files: Vec::new(),
            extra_files: Vec::new(),
            missing_files: vec!["<root>".to_string()],
            skipped_binary_files: Vec::new(),
        });
    }

    if !expected_exists {
        // Collect all files in actual as extra
        let all_files = collect_files_recursive(&actual_path)?;
        let extra_files: Vec<String> = all_files
            .iter()
            .filter_map(|(p, is_binary)| {
                if *is_binary {
                    None // Binary files are handled separately
                } else {
                    Some(p.clone())
                }
            })
            .collect();

        let skipped_binary: Vec<String> = all_files
            .iter()
            .filter_map(
                |(p, is_binary)| {
                    if *is_binary { Some(p.clone()) } else { None }
                },
            )
            .collect();

        return Ok(ComparisonResult {
            matched: false,
            matched_files: Vec::new(),
            mismatched_files: Vec::new(),
            extra_files,
            missing_files: vec!["<root>".to_string()],
            skipped_binary_files: skipped_binary,
        });
    }

    // Collect all files from both directories
    let actual_files = collect_files_recursive(&actual_path)?;
    let expected_files = collect_files_recursive(&expected_path)?;

    // Store the lengths before consuming the vectors
    let actual_files_len = actual_files.len();
    let expected_files_len = expected_files.len();

    // Build maps for easier comparison
    let actual_map: HashMap<String, (bool, Vec<u8>)> = actual_files
        .into_iter()
        .map(|(p, is_bin)| (p, (is_bin, Vec::new())))
        .collect();
    let expected_map: HashMap<String, bool> = expected_files.into_iter().collect();

    let mut matched_files = Vec::new();
    let mut mismatched_files = Vec::new();
    let mut extra_files = Vec::new();
    let mut missing_files = Vec::new();
    let mut skipped_binary_files = Vec::new();

    // Compare files
    let all_paths: std::collections::HashSet<&String> =
        actual_map.keys().chain(expected_map.keys()).collect();

    for path in all_paths {
        let actual_exists = actual_map.contains_key(path);
        let expected_exists = expected_map.contains_key(path);

        match (actual_exists, expected_exists) {
            (true, true) => {
                // File exists in both - compare contents
                let actual_is_binary = actual_map
                    .get(path)
                    .map(|(is_bin, _)| *is_bin)
                    .unwrap_or(false);
                let expected_is_binary = *expected_map.get(path).unwrap();

                if actual_is_binary && expected_is_binary {
                    // Binary file exists in both sides - skip content comparison
                    skipped_binary_files.push(path.clone());
                    continue;
                }

                if actual_is_binary != expected_is_binary {
                    // Binary/text mismatch - record as mismatched
                    mismatched_files.push(FileComparisonResult {
                        path: path.clone(),
                        matched: false,
                        mismatch_type: Some("binary_type_mismatch".to_string()),
                        details: Some(format!(
                            "File type mismatch: actual is {}, expected is {}",
                            if actual_is_binary { "binary" } else { "text" },
                            if expected_is_binary { "binary" } else { "text" }
                        )),
                    });
                    continue;
                }

                let actual_file = actual_path.join(path);
                let expected_file = expected_path.join(path);

                let actual_content = fs::read_to_string(&actual_file)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                let expected_content = fs::read_to_string(&expected_file)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

                // Dispatch by file extension - use JSON comparison for .json files
                let comparison = if path.ends_with(".json") {
                    compare_json(&actual_content, &expected_content)
                } else {
                    compare_strings(&actual_content, &expected_content)
                };

                if comparison.0 {
                    matched_files.push(path.clone());
                } else {
                    mismatched_files.push(FileComparisonResult {
                        path: path.clone(),
                        matched: false,
                        mismatch_type: Some(comparison.1),
                        details: Some(comparison.2),
                    });
                }
            }
            (true, false) => {
                // File only in actual (including binary files)
                extra_files.push(path.clone());
            }
            (false, true) => {
                // File only in expected (including binary files)
                missing_files.push(path.clone());
            }
            (false, false) => {
                // Shouldn't happen
                unreachable!();
            }
        }
    }

    let matched = matched_files.len() + skipped_binary_files.len() == actual_files_len
        && matched_files.len() + skipped_binary_files.len() == expected_files_len
        && mismatched_files.is_empty();

    Ok(ComparisonResult {
        matched,
        matched_files,
        mismatched_files,
        extra_files,
        missing_files,
        skipped_binary_files,
    })
}

/// Collect all files in a directory recursively.
///
/// Returns a list of (relative_path, is_binary) tuples.
fn collect_files_recursive(dir: &Path) -> Result<Vec<(String, bool)>, Box<dyn Error + Send>> {
    let mut files = Vec::new();
    collect_files_recursive_helper(dir, dir, &mut files)?;
    Ok(files)
}

/// Helper function for recursive file collection.
fn collect_files_recursive_helper(
    base_path: &Path,
    current_path: &Path,
    files: &mut Vec<(String, bool)>,
) -> Result<(), Box<dyn Error + Send>> {
    if !current_path.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(current_path).map_err(|e| Box::new(e) as Box<dyn Error + Send>)? {
        let entry = entry.map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subdirectory
            collect_files_recursive_helper(base_path, &path, files)?;
        } else if path.is_file() {
            // Get relative path
            let relative = path
                .strip_prefix(base_path)
                .map_err(|e| {
                    Box::new(std::io::Error::other(format!(
                        "Failed to compute relative path: {e}"
                    ))) as Box<dyn Error + Send>
                })?
                .to_string_lossy()
                .replace('\\', "/"); // Normalize path separators

            // Check if file is binary
            let is_binary = is_binary_file(&path);

            files.push((relative, is_binary));
        }
    }

    Ok(())
}

/// Check if a file is binary.
///
/// Reads the first few bytes to check for binary content.
fn is_binary_file(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };

    // Check for common binary file extensions
    if let Some(ext) = path.extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        let binary_extensions = [
            "png", "jpg", "jpeg", "gif", "bmp", "ico", "webp", "pdf", "zip", "tar", "gz", "7z",
            "rar", "exe", "dll", "so", "dylib", "bin", "class", "jar", "war", "ear", "o", "a",
            "lib",
        ];
        if binary_extensions.contains(&ext_lower.as_str()) {
            return true;
        }
    }

    // Check content for null bytes (common in binary files)
    let sample_size = bytes.len().min(512);
    for byte in &bytes[..sample_size] {
        if *byte == 0 {
            return true;
        }
    }

    false
}

/// Compare two JSON strings for deep equality.
///
/// Returns (matched, mismatch_type, details).
fn compare_json(actual: &str, expected: &str) -> (bool, String, String) {
    let actual_value: serde_json::Value = match serde_json::from_str(actual) {
        Ok(v) => v,
        Err(e) => {
            return (
                false,
                "json_parse_error".to_string(),
                format!("Failed to parse actual JSON: {e}"),
            );
        }
    };

    let expected_value: serde_json::Value = match serde_json::from_str(expected) {
        Ok(v) => v,
        Err(e) => {
            return (
                false,
                "json_parse_error".to_string(),
                format!("Failed to parse expected JSON: {e}"),
            );
        }
    };

    if actual_value == expected_value {
        (true, String::new(), String::new())
    } else {
        (
            false,
            "json_mismatch".to_string(),
            format!("JSON values differ\nActual:   {actual_value}\nExpected: {expected_value}"),
        )
    }
}

/// Compare two strings after trimming whitespace.
///
/// Returns (matched, mismatch_type, details).
fn compare_strings(actual: &str, expected: &str) -> (bool, String, String) {
    // Normalize line endings (CRLF -> LF) and strip a single trailing newline.
    // We do NOT use trim() because that masks real whitespace regressions
    // in indentation-sensitive files (YAML, Markdown, etc.).
    let actual_replaced = actual.replace("\r\n", "\n");
    let actual_normalized = actual_replaced
        .strip_suffix('\n')
        .unwrap_or(&actual_replaced)
        .to_string();
    let expected_replaced = expected.replace("\r\n", "\n");
    let expected_normalized = expected_replaced
        .strip_suffix('\n')
        .unwrap_or(&expected_replaced)
        .to_string();

    if actual_normalized == expected_normalized {
        (true, String::new(), String::new())
    } else {
        (
            false,
            "content_mismatch".to_string(),
            format!(
                "String content differs\nActual:   {actual_normalized:?}\nExpected: {expected_normalized:?}"
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_run_validate_commands_single() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_path = temp_dir.path().join("output");
        fs::create_dir(&output_path).expect("Failed to create output dir");

        // Create a file to test against
        let test_file = output_path.join("test.txt");
        fs::write(&test_file, "hello world").expect("Failed to write test file");

        let commands = vec!["test -f test.txt".to_string()];
        let results = run_validate_commands(output_path.to_str().unwrap(), &commands)
            .expect("Failed to run validate commands");

        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        assert_eq!(results[0].command, "test -f test.txt");
    }

    #[test]
    fn test_run_validate_commands_failure() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let output_path = temp_dir.path().join("output");
        fs::create_dir(&output_path).expect("Failed to create output dir");

        let commands = vec!["nonexistent_command".to_string()];
        let results = run_validate_commands(output_path.to_str().unwrap(), &commands)
            .expect("Failed to run validate commands");

        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
    }

    #[test]
    fn test_compare_directories_identical() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let actual_path = temp_dir.path().join("actual");
        let expected_path = temp_dir.path().join("expected");

        fs::create_dir(&actual_path).expect("Failed to create actual dir");
        fs::create_dir(&expected_path).expect("Failed to create expected dir");

        // Create identical files
        let actual_file = actual_path.join("test.txt");
        let expected_file = expected_path.join("test.txt");
        fs::write(&actual_file, "hello world").expect("Failed to write actual");
        fs::write(&expected_file, "hello world").expect("Failed to write expected");

        let result = compare_directories(
            actual_path.to_str().unwrap(),
            expected_path.to_str().unwrap(),
        )
        .expect("Failed to compare directories");

        assert!(result.matched);
        assert_eq!(result.matched_files.len(), 1);
        assert_eq!(result.mismatched_files.len(), 0);
        assert_eq!(result.extra_files.len(), 0);
        assert_eq!(result.missing_files.len(), 0);
    }

    #[test]
    fn test_compare_directories_with_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let actual_path = temp_dir.path().join("actual");
        let expected_path = temp_dir.path().join("expected");

        fs::create_dir(&actual_path).expect("Failed to create actual dir");
        fs::create_dir(&expected_path).expect("Failed to create expected dir");

        // Create JSON files with same content
        let actual_file = actual_path.join("config.json");
        let expected_file = expected_path.join("config.json");
        fs::write(&actual_file, "{\"a\": 1, \"b\": 2}").expect("Failed to write actual");
        fs::write(&expected_file, "{\"a\": 1, \"b\": 2}").expect("Failed to write expected");

        let result = compare_directories(
            actual_path.to_str().unwrap(),
            expected_path.to_str().unwrap(),
        )
        .expect("Failed to compare directories");

        assert!(result.matched, "JSON files with same content should match");
    }

    #[test]
    fn test_compare_directories_missing_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let actual_path = temp_dir.path().join("actual");
        let expected_path = temp_dir.path().join("expected");

        fs::create_dir(&actual_path).expect("Failed to create actual dir");
        fs::create_dir(&expected_path).expect("Failed to create expected dir");

        // Create file only in expected
        let expected_file = expected_path.join("missing.txt");
        fs::write(&expected_file, "content").expect("Failed to write expected");

        let result = compare_directories(
            actual_path.to_str().unwrap(),
            expected_path.to_str().unwrap(),
        )
        .expect("Failed to compare directories");

        assert!(!result.matched);
        assert_eq!(result.missing_files.len(), 1);
        assert!(
            result
                .missing_files
                .iter()
                .any(|p| p.contains("missing.txt"))
        );
    }

    #[test]
    fn test_compare_directories_extra_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let actual_path = temp_dir.path().join("actual");
        let expected_path = temp_dir.path().join("expected");

        fs::create_dir(&actual_path).expect("Failed to create actual dir");
        fs::create_dir(&expected_path).expect("Failed to create expected dir");

        // Create file only in actual
        let actual_file = actual_path.join("extra.txt");
        fs::write(&actual_file, "content").expect("Failed to write actual");

        let result = compare_directories(
            actual_path.to_str().unwrap(),
            expected_path.to_str().unwrap(),
        )
        .expect("Failed to compare directories");

        assert!(!result.matched);
        assert_eq!(result.extra_files.len(), 1);
        assert!(result.extra_files.iter().any(|p| p.contains("extra.txt")));
    }

    #[test]
    fn test_compare_directories_both_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let actual_path = temp_dir.path().join("actual");
        let expected_path = temp_dir.path().join("expected");

        fs::create_dir(&actual_path).expect("Failed to create actual dir");
        fs::create_dir(&expected_path).expect("Failed to create expected dir");

        let result = compare_directories(
            actual_path.to_str().unwrap(),
            expected_path.to_str().unwrap(),
        )
        .expect("Failed to compare directories");

        assert!(result.matched, "Empty directories should match");
    }

    #[test]
    fn test_compare_directories_both_nonexistent() {
        let result = compare_directories("/nonexistent/actual", "/nonexistent/expected")
            .expect("Failed to compare directories");

        assert!(result.matched, "Both non-existent should match");
    }

    #[test]
    fn test_compare_json_same_content_different_order() {
        let actual = r#"
{
  "z": 3,
  "y": 2,
  "x": 1
}"#;
        let expected = r#"
{
  "x": 1,
  "y": 2,
  "z": 3
}"#;

        let (matched, mismatch_type, details) = compare_json(actual, expected);
        assert!(matched);
        assert!(mismatch_type.is_empty());
        assert!(details.is_empty());
    }

    #[test]
    fn test_compare_json_different_content() {
        let actual = r#"{"key": "value1"}"#;
        let expected = r#"{"key": "value2"}"#;

        let (matched, mismatch_type, _) = compare_json(actual, expected);
        assert!(!matched);
        assert_eq!(mismatch_type, "json_mismatch");
    }

    #[test]
    fn test_compare_strings_trailing_newline() {
        let actual = "hello world\n";
        let expected = "hello world";

        let (matched, mismatch_type, details) = compare_strings(actual, expected);
        assert!(
            matched,
            "Strings should match after trailing newline normalization"
        );
        assert!(mismatch_type.is_empty());
        assert!(details.is_empty());
    }

    #[test]
    fn test_compare_strings_leading_whitespace_differs() {
        let actual = "  hello world";
        let expected = "hello world";

        let (matched, _, _) = compare_strings(actual, expected);
        assert!(
            !matched,
            "Leading whitespace differences should be detected"
        );
    }

    #[test]
    fn test_compare_strings_different() {
        let actual = "hello world";
        let expected = "goodbye world";

        let (matched, mismatch_type, _) = compare_strings(actual, expected);
        assert!(!matched);
        assert_eq!(mismatch_type, "content_mismatch");
    }

    #[test]
    fn test_is_binary_file_text() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let text_file = temp_dir.path().join("test.txt");
        fs::write(&text_file, "plain text").expect("Failed to write text");

        assert!(
            !is_binary_file(&text_file),
            "Text file should not be binary"
        );
    }

    #[test]
    fn test_is_binary_file_json() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let json_file = temp_dir.path().join("test.json");
        fs::write(&json_file, r#"{"key": "value"}"#).expect("Failed to write JSON");

        assert!(
            !is_binary_file(&json_file),
            "JSON file should not be binary"
        );
    }

    #[test]
    fn test_is_binary_file_png() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let png_file = temp_dir.path().join("test.png");
        // Write PNG signature bytes
        let png_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        fs::write(&png_file, png_bytes).expect("Failed to write PNG");

        assert!(is_binary_file(&png_file), "PNG file should be binary");
    }

    #[test]
    fn test_compare_directories_binary_text_mismatch() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let actual_path = temp_dir.path().join("actual");
        let expected_path = temp_dir.path().join("expected");

        fs::create_dir(&actual_path).expect("Failed to create actual dir");
        fs::create_dir(&expected_path).expect("Failed to create expected dir");

        // Create text file in actual
        let actual_file = actual_path.join("test.dat");
        fs::write(&actual_file, "text content").expect("Failed to write actual");

        // Create binary file in expected (using null byte)
        let expected_file = expected_path.join("test.dat");
        fs::write(&expected_file, b"binary\x00content").expect("Failed to write expected");

        let result = compare_directories(
            actual_path.to_str().unwrap(),
            expected_path.to_str().unwrap(),
        )
        .expect("Failed to compare directories");

        assert!(
            !result.matched,
            "Binary/text mismatch should fail comparison"
        );
        assert_eq!(result.mismatched_files.len(), 1);
        assert_eq!(
            result.mismatched_files[0].mismatch_type,
            Some("binary_type_mismatch".to_string())
        );
        assert!(
            result.mismatched_files[0]
                .details
                .as_ref()
                .unwrap()
                .contains("binary")
        );
        assert!(
            result.mismatched_files[0]
                .details
                .as_ref()
                .unwrap()
                .contains("text")
        );
    }
}
