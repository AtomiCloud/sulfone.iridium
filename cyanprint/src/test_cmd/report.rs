//! Test result reporting.
//!
//! This module provides functionality for:
//! - Generating human-readable test reports
//! - Writing JUnit XML format reports for CI integration

use std::error::Error;
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Result of a single test case.
///
/// Contains name, pass/fail status, duration, and optional failure message.
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Name of the test case
    pub name: String,

    /// Whether the test passed
    pub passed: bool,

    /// Duration of test execution
    pub duration: Duration,

    /// Optional failure message explaining why the test failed
    pub failure_message: Option<String>,
}

/// Write a human-readable test report to stdout.
///
/// Prints a formatted report showing:
/// - Overall summary (passed, failed, total)
/// - Per-test results with dotted lines
/// - Duration for each test
///
/// # Arguments
///
/// * `results` - Vector of [`TestResult`] to report
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::report::{TestResult, write_human_report};
/// use std::time::Duration;
///
/// let results = vec![
///     TestResult {
///         name: "test_basic".to_string(),
///         passed: true,
///         duration: Duration::from_secs(1),
///         failure_message: None,
///     },
///     TestResult {
///         name: "test_advanced".to_string(),
///         passed: false,
///         duration: Duration::from_millis(500),
///         failure_message: Some("Expected 'hello' but got 'goodbye'".to_string()),
///     },
/// ];
/// write_human_report(&results);
/// ```
pub fn write_human_report(results: &[TestResult]) {
    if results.is_empty() {
        println!("No test results to report");
        return;
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    // Print header
    println!();
    println!("Test Results");
    println!("===");

    // Print summary
    if failed == 0 {
        println!("\n  All tests passed ({passed}/{total})");
    } else {
        println!("\n  {passed}/{total} tests passed");
    }

    // Print individual test results
    println!();
    for result in results {
        let status = if result.passed { "PASS" } else { "FAIL" };
        let duration_str = format_duration(result.duration);

        println!("  [{}] {} ({})", status, result.name, duration_str);

        if !result.passed {
            if let Some(ref message) = result.failure_message {
                println!("      Failure: {message}");
            }
        }
    }

    // Print footer with final summary
    println!();
    println!("Summary: {passed}/{total} passed");
    if failed > 0 {
        println!("         {failed}/{total} failed");
    }

    println!();
}

/// Write a JUnit XML report to a file.
///
/// Generates a standard JUnit XML format for CI/CD integration.
///
/// # Arguments
///
/// * `results` - Vector of [`TestResult`] to report
/// * `path` - Path where to write the JUnit XML file
///
/// # Errors
///
/// Returns an error if:
/// - Parent directories don't exist
/// - File cannot be written
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::report::{TestResult, write_junit_report};
/// use std::time::Duration;
///
/// let results = vec![
///     TestResult {
///         name: "test_basic".to_string(),
///         passed: true,
///         duration: Duration::from_secs(1),
///         failure_message: None,
///     },
/// ];
/// write_junit_report(&results, "report.xml").unwrap();
/// ```
pub fn write_junit_report(results: &[TestResult], path: &str) -> Result<(), Box<dyn Error + Send>> {
    // Create parent directories if they don't exist
    let path_buf = Path::new(path);
    if let Some(parent) = path_buf.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| {
                Box::new(std::io::Error::other(format!(
                    "Failed to create parent directories for {path}: {e}"
                ))) as Box<dyn Error + Send>
            })?;
        }
    }

    // Build JUnit XML content
    let mut xml_content = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml_content.push_str("<testsuites>\n");

    let total = results.len();
    let failures = results.iter().filter(|r| !r.passed).count();
    let total_duration: Duration = results.iter().map(|r| r.duration).sum();

    xml_content.push_str(&format!(
        "  <testsuite name=\"cyanprint-tests\" tests=\"{}\" failures=\"{}\" time=\"{}\">\n",
        total,
        failures,
        total_duration.as_secs_f64()
    ));

    for result in results {
        let duration_secs = result.duration.as_secs_f64();
        xml_content.push_str("    <testcase ");
        xml_content.push_str(&format!("name=\"{}\" ", escape_xml(&result.name)));
        xml_content.push_str(&format!("time=\"{duration_secs}\""));

        if result.passed {
            xml_content.push_str("/>\n");
        } else {
            xml_content.push_str(">\n");
            xml_content.push_str("      <failure");

            if let Some(ref message) = result.failure_message {
                xml_content.push_str(&format!(" message=\"{}\"", escape_xml(message)));
            }

            xml_content.push_str(">\n");
            xml_content.push_str("        Test failed\n");
            xml_content.push_str("      </failure>\n");
            xml_content.push_str("    </testcase>\n");
        }
    }

    xml_content.push_str("  </testsuite>\n");
    xml_content.push_str("</testsuites>\n");

    // Write to file
    fs::write(path, xml_content).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to write JUnit report: {e}"
        ))) as Box<dyn Error + Send>
    })?;

    Ok(())
}

/// Format a Duration for display.
///
/// Returns a string like "1.23s", "500ms", etc.
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();

    if secs >= 1.0 {
        format!("{secs:.2}s")
    } else if secs >= 0.001 {
        format!("{:.0}ms", secs * 1000.0)
    } else {
        format!("{:.0}μs", secs * 1_000_000.0)
    }
}

/// Escape XML special characters.
///
/// Replaces characters that would break XML structure.
/// Also strips XML 1.0 invalid control characters (U+0000–U+0008, U+000B, U+000C, U+000E–U+001F)
/// while preserving tab (U+0009), newline (U+000A), and carriage return (U+000D).
fn escape_xml(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect::<Vec<_>>(),
            '>' => "&gt;".chars().collect::<Vec<_>>(),
            '"' => "&quot;".chars().collect::<Vec<_>>(),
            '\'' => "&apos;".chars().collect::<Vec<_>>(),
            // Strip invalid XML 1.0 control characters
            // Valid: \t (0x09), \n (0x0A), \r (0x0D)
            // Invalid: 0x00-0x08, 0x0B, 0x0C, 0x0E-0x1F
            c if c as u32 <= 0x1F && c != '\t' && c != '\n' && c != '\r' => vec![],
            _ => vec![c],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_seconds() {
        let duration = Duration::from_secs(2);
        let formatted = format_duration(duration);
        assert_eq!(formatted, "2.00s");
    }

    #[test]
    fn test_format_duration_milliseconds() {
        let duration = Duration::from_millis(500);
        let formatted = format_duration(duration);
        assert_eq!(formatted, "500ms");
    }

    #[test]
    fn test_format_duration_microseconds() {
        let duration = Duration::from_micros(100);
        let formatted = format_duration(duration);
        assert_eq!(formatted, "100μs");
    }

    #[test]
    fn test_escape_xml_ampersand() {
        let escaped = escape_xml("a & b < c > d \" e ' f");
        assert_eq!(escaped, "a &amp; b &lt; c &gt; d &quot; e &apos; f");
    }

    #[test]
    fn test_escape_xml_only_special() {
        let escaped = escape_xml("normal_text");
        assert_eq!(escaped, "normal_text");
    }

    #[test]
    fn test_escape_xml_strips_invalid_control_chars() {
        // Valid control characters should be preserved
        let with_valid_controls = escape_xml("hello\tworld\nnew\rline");
        assert_eq!(with_valid_controls, "hello\tworld\nnew\rline");

        // Invalid control characters should be stripped
        let with_invalid_controls = escape_xml("a\x00b\x01c\x08d\x0Be\x0Cf\x1Fg");
        assert_eq!(with_invalid_controls, "abcdefg");

        // Mix of valid and invalid
        let mixed = escape_xml("start\x00mid\ntab\there\x1Fend");
        assert_eq!(mixed, "startmid\ntab\thereend");
    }

    #[test]
    fn test_write_junit_report_basic() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let report_path = temp_dir.path().join("report.xml");

        let results = vec![
            TestResult {
                name: "test_passed".to_string(),
                passed: true,
                duration: Duration::from_secs(1),
                failure_message: None,
            },
            TestResult {
                name: "test_failed".to_string(),
                passed: false,
                duration: Duration::from_millis(500),
                failure_message: Some("Expected X, got Y".to_string()),
            },
        ];

        write_junit_report(&results, report_path.to_str().unwrap())
            .expect("Failed to write JUnit report");

        let content = fs::read_to_string(&report_path).expect("Failed to read written report");

        assert!(content.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(content.contains("<testsuites>"));
        assert!(content.contains("name=\"cyanprint-tests\""));
        assert!(content.contains("tests=\"2\""));
        assert!(content.contains("failures=\"1\""));
        assert!(content.contains("<testcase name=\"test_passed\""));
        assert!(content.contains("<testcase name=\"test_failed\""));
        assert!(content.contains("<failure"));
        assert!(content.contains("Expected X, got Y"));
    }

    #[test]
    fn test_write_junit_report_all_passed() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let report_path = temp_dir.path().join("report.xml");

        let results = vec![
            TestResult {
                name: "test1".to_string(),
                passed: true,
                duration: Duration::from_secs(1),
                failure_message: None,
            },
            TestResult {
                name: "test2".to_string(),
                passed: true,
                duration: Duration::from_secs(2),
                failure_message: None,
            },
        ];

        write_junit_report(&results, report_path.to_str().unwrap())
            .expect("Failed to write JUnit report");

        let content = fs::read_to_string(&report_path).expect("Failed to read written report");

        assert!(content.contains("tests=\"2\""));
        assert!(content.contains("failures=\"0\""));
        assert!(!content.contains("<failure>"));
    }

    #[test]
    fn test_write_junit_report_creates_parent_dirs() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let report_path = temp_dir.path().join("nested/dir/report.xml");

        let results = vec![];

        write_junit_report(&results, report_path.to_str().unwrap())
            .expect("Failed to write JUnit report");

        assert!(report_path.exists(), "Report file should exist");
    }

    #[test]
    fn test_write_human_report_empty() {
        let results: Vec<TestResult> = vec![];
        // Just make sure it doesn't panic
        write_human_report(&results);
    }

    #[test]
    fn test_write_human_report_mixed() {
        let results = vec![
            TestResult {
                name: "test1".to_string(),
                passed: true,
                duration: Duration::from_secs(1),
                failure_message: None,
            },
            TestResult {
                name: "test2".to_string(),
                passed: false,
                duration: Duration::from_millis(500),
                failure_message: Some("Error occurred".to_string()),
            },
            TestResult {
                name: "test3".to_string(),
                passed: true,
                duration: Duration::from_secs(2),
                failure_message: None,
            },
        ];

        // Capture stdout (this test just verifies the function runs without panic)
        write_human_report(&results);
    }
}
