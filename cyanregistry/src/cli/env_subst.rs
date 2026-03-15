//! Environment variable substitution module for cyan.yaml configuration files.
//!
//! This module provides functionality to substitute environment variables in
//! configuration strings using the `${VAR}` and `${VAR:-default}` syntax.

use std::error::Error;
use std::fmt;

/// Error type for environment variable substitution failures.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvSubstError {
    /// The name of the environment variable that was not found
    pub var_name: String,
}

impl EnvSubstError {
    /// Creates a new EnvSubstError with the given variable name.
    pub fn new(var_name: impl Into<String>) -> Self {
        EnvSubstError {
            var_name: var_name.into(),
        }
    }
}

impl fmt::Display for EnvSubstError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Environment variable '{}' is not set and no default value was provided",
            self.var_name
        )
    }
}

impl Error for EnvSubstError {}

/// Substitutes environment variables in the input string.
///
/// Supports the following syntax:
/// - `${VAR}` - Substitutes with the value of the environment variable VAR.
///   Returns an error if VAR is not set or empty.
/// - `${VAR:-default}` - Substitutes with the value of VAR if set and non-empty,
///   otherwise uses the default value.
///
/// # Arguments
///
/// * `input` - The input string potentially containing `${...}` patterns.
///
/// # Returns
///
/// * `Ok(String)` - The input string with all environment variables substituted.
/// * `Err(EnvSubstError)` - If a required environment variable is missing.
///
/// # Examples
///
/// ```
/// use cyanregistry::cli::env_subst::{substitute_env_vars, EnvSubstError};
///
/// // Basic substitution
/// std::env::set_var("MY_VAR", "hello");
/// assert_eq!(substitute_env_vars("${MY_VAR}").unwrap(), "hello");
///
/// // With default value
/// assert_eq!(substitute_env_vars("${MISSING:-fallback}").unwrap(), "fallback");
///
/// // Multiple variables
/// std::env::set_var("A", "foo");
/// std::env::set_var("B", "bar");
/// assert_eq!(substitute_env_vars("${A}/${B}").unwrap(), "foo/bar");
///
/// // No variables - passthrough unchanged
/// assert_eq!(substitute_env_vars("plain text").unwrap(), "plain text");
/// ```
pub fn substitute_env_vars(input: &str) -> Result<String, EnvSubstError> {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            // Consume the '{'
            chars.next();

            // Find the matching '}'
            let mut var_content = String::new();
            let mut found_closing = false;
            let mut nested_depth = 0;

            for inner_c in chars.by_ref() {
                if inner_c == '{' {
                    nested_depth += 1;
                    var_content.push(inner_c);
                } else if inner_c == '}' {
                    if nested_depth > 0 {
                        nested_depth -= 1;
                        var_content.push(inner_c);
                    } else {
                        found_closing = true;
                        break;
                    }
                } else {
                    var_content.push(inner_c);
                }
            }

            if !found_closing {
                // No closing brace found - treat as literal
                result.push_str("${");
                result.push_str(&var_content);
                continue;
            }

            // Parse the variable content
            let (var_name, default_value) = parse_var_content(&var_content);

            // Empty variable name is treated as literal
            if var_name.is_empty() {
                result.push_str("${");
                result.push_str(&var_content);
                result.push('}');
                continue;
            }

            // Get the environment variable value
            let env_value = std::env::var(&var_name).ok();

            match (env_value, default_value) {
                (Some(val), _) if !val.is_empty() => result.push_str(&val),
                (Some(_), Some(default)) => result.push_str(&default),
                (Some(_), None) => {
                    // Variable is set but empty, and no default
                    return Err(EnvSubstError::new(var_name));
                }
                (None, Some(default)) => result.push_str(&default),
                (None, None) => {
                    return Err(EnvSubstError::new(var_name));
                }
            }
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

/// Parses the content inside `${...}` and returns (var_name, optional_default).
///
/// Supports:
/// - `VAR` -> (VAR, None)
/// - `VAR:-default` -> (VAR, Some(default))
fn parse_var_content(content: &str) -> (String, Option<String>) {
    // Look for the :- operator
    if let Some(colon_dash_pos) = content.find(":-") {
        let var_name = content[..colon_dash_pos].to_string();
        let default = content[colon_dash_pos + 2..].to_string();
        (var_name, Some(default))
    } else {
        (content.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use unique variable names per test to avoid parallel test interference

    #[test]
    fn test_basic_substitution() {
        std::env::set_var("CYAN_TEST_BASIC_VAR", "hello");
        assert_eq!(
            substitute_env_vars("${CYAN_TEST_BASIC_VAR}").unwrap(),
            "hello"
        );
        std::env::remove_var("CYAN_TEST_BASIC_VAR");
    }

    #[test]
    fn test_default_value() {
        std::env::remove_var("CYAN_TEST_MISSING_VAR_1");
        assert_eq!(
            substitute_env_vars("${CYAN_TEST_MISSING_VAR_1:-fallback}").unwrap(),
            "fallback"
        );
    }

    #[test]
    fn test_missing_var_without_default() {
        std::env::remove_var("CYAN_TEST_MISSING_VAR_2");
        let result = substitute_env_vars("${CYAN_TEST_MISSING_VAR_2}");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.var_name, "CYAN_TEST_MISSING_VAR_2");
    }

    #[test]
    fn test_empty_var_without_default() {
        std::env::set_var("CYAN_TEST_EMPTY_VAR_1", "");
        let result = substitute_env_vars("${CYAN_TEST_EMPTY_VAR_1}");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.var_name, "CYAN_TEST_EMPTY_VAR_1");
        std::env::remove_var("CYAN_TEST_EMPTY_VAR_1");
    }

    #[test]
    fn test_empty_var_with_default() {
        std::env::set_var("CYAN_TEST_EMPTY_VAR_2", "");
        assert_eq!(
            substitute_env_vars("${CYAN_TEST_EMPTY_VAR_2:-default_value}").unwrap(),
            "default_value"
        );
        std::env::remove_var("CYAN_TEST_EMPTY_VAR_2");
    }

    #[test]
    fn test_multiple_vars_in_one_string() {
        std::env::set_var("CYAN_TEST_VAR_A", "foo");
        std::env::set_var("CYAN_TEST_VAR_B", "bar");
        assert_eq!(
            substitute_env_vars("${CYAN_TEST_VAR_A}/${CYAN_TEST_VAR_B}").unwrap(),
            "foo/bar"
        );
        std::env::remove_var("CYAN_TEST_VAR_A");
        std::env::remove_var("CYAN_TEST_VAR_B");
    }

    #[test]
    fn test_no_vars_passthrough() {
        assert_eq!(substitute_env_vars("plain text").unwrap(), "plain text");
    }

    #[test]
    fn test_empty_default() {
        std::env::remove_var("CYAN_TEST_MISSING_VAR_3");
        assert_eq!(
            substitute_env_vars("${CYAN_TEST_MISSING_VAR_3:-}").unwrap(),
            ""
        );
    }

    #[test]
    fn test_multiple_vars_complex() {
        std::env::set_var("CYAN_TEST_REGISTRY", "ghcr.io/atomicloud");
        std::env::set_var("CYAN_TEST_IMAGE", "my-template");
        std::env::set_var("CYAN_TEST_TAG", "latest");
        assert_eq!(
            substitute_env_vars("${CYAN_TEST_REGISTRY}/${CYAN_TEST_IMAGE}:${CYAN_TEST_TAG}")
                .unwrap(),
            "ghcr.io/atomicloud/my-template:latest"
        );
        std::env::remove_var("CYAN_TEST_REGISTRY");
        std::env::remove_var("CYAN_TEST_IMAGE");
        std::env::remove_var("CYAN_TEST_TAG");
    }

    #[test]
    fn test_unclosed_brace_literal_passthrough() {
        // Unclosed braces should be treated as literal text
        assert_eq!(substitute_env_vars("${NOTCLOSED").unwrap(), "${NOTCLOSED");
    }

    #[test]
    fn test_dollar_without_brace() {
        // Dollar sign without brace should be literal
        assert_eq!(substitute_env_vars("$NOTVAR").unwrap(), "$NOTVAR");
    }

    #[test]
    fn test_empty_var_name() {
        // Empty var name ${} should be literal
        assert_eq!(substitute_env_vars("${}").unwrap(), "${}");
    }

    #[test]
    fn test_var_with_default_containing_colon() {
        std::env::remove_var("CYAN_TEST_MY_VAR");
        assert_eq!(
            substitute_env_vars("${CYAN_TEST_MY_VAR:-https://example.com}").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn test_mixed_content() {
        std::env::set_var("CYAN_TEST_MIXED_VAR", "ghcr.io/atomicloud");
        assert_eq!(
            substitute_env_vars("prefix/${CYAN_TEST_MIXED_VAR}/suffix").unwrap(),
            "prefix/ghcr.io/atomicloud/suffix"
        );
        std::env::remove_var("CYAN_TEST_MIXED_VAR");
    }

    #[test]
    fn test_env_subst_error_display() {
        let err = EnvSubstError::new("MY_MISSING_VAR");
        assert_eq!(
            err.to_string(),
            "Environment variable 'MY_MISSING_VAR' is not set and no default value was provided"
        );
    }

    #[test]
    fn test_nested_braces_in_default() {
        std::env::remove_var("CYAN_TEST_OUTER");
        // Note: nested braces in default values are not fully supported
        // but we should at least not crash
        let result = substitute_env_vars("${CYAN_TEST_OUTER:-{nested}}");
        assert!(result.is_ok());
    }
}
