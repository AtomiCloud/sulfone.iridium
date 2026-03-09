//! Registry for mapping files to resolvers
//!
//! This module provides glob-based pattern matching to determine
//! which resolver should handle a specific file.

use crate::conflict_file_resolver::models::{ResolverChoice, ResolverInstance};
use glob::Pattern;
use std::collections::HashMap;

/// Registry that maps templates to their resolvers
/// and resolves which resolver should handle a file
pub struct ConflictFileResolverRegistry {
    /// Map from template ID to list of resolver instances
    template_resolvers: HashMap<String, Vec<ResolverInstance>>,
}

impl ConflictFileResolverRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            template_resolvers: HashMap::new(),
        }
    }

    /// Register resolvers for a template
    pub fn register(&mut self, template_id: String, resolvers: Vec<ResolverInstance>) {
        self.template_resolvers.insert(template_id, resolvers);
    }

    /// Get the resolver choice for a file from a specific template
    ///
    /// Returns:
    /// - ResolverChoice::Some(instance) if a resolver matches the file
    /// - ResolverChoice::None if no resolver is configured for this file
    pub fn get_resolver_choice(&self, template_id: &str, path: &str) -> ResolverChoice {
        let resolvers = match self.template_resolvers.get(template_id) {
            Some(r) => r,
            None => return ResolverChoice::None,
        };

        // Find the first resolver that matches the file path
        for resolver in resolvers {
            if self.matches_any_pattern(path, &resolver.file_patterns) {
                return ResolverChoice::Some(resolver.clone());
            }
        }

        ResolverChoice::None
    }

    /// Check if a path matches any of the glob patterns
    fn matches_any_pattern(&self, path: &str, patterns: &[String]) -> bool {
        for pattern in patterns {
            // Handle different glob pattern styles
            if self.path_matches_glob(path, pattern) {
                return true;
            }
        }
        false
    }

    /// Match a path against a glob pattern
    fn path_matches_glob(&self, path: &str, pattern: &str) -> bool {
        // Normalize the path (use forward slashes)
        let normalized_path = path.replace('\\', "/");

        // Try to match with glob crate
        // Fail closed: return false on invalid glob patterns
        match Pattern::new(pattern) {
            Ok(glob_pattern) => glob_pattern.matches(&normalized_path),
            Err(_) => false,
        }
    }
}

impl Default for ConflictFileResolverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_resolver(patterns: Vec<&str>) -> ResolverInstance {
        ResolverInstance {
            id: "test-id".to_string(),
            docker_ref: "atomi/json-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({}),
            file_patterns: patterns.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_register_and_get_resolver() {
        let mut registry = ConflictFileResolverRegistry::new();
        let resolver = create_test_resolver(vec!["package.json", "**/tsconfig.json"]);

        registry.register("template-a".to_string(), vec![resolver]);

        // Should match exact file
        let choice = registry.get_resolver_choice("template-a", "package.json");
        assert!(matches!(choice, ResolverChoice::Some(_)));

        // Should match nested file
        let choice = registry.get_resolver_choice("template-a", "src/tsconfig.json");
        assert!(matches!(choice, ResolverChoice::Some(_)));

        // Should not match other files
        let choice = registry.get_resolver_choice("template-a", "other.txt");
        assert!(matches!(choice, ResolverChoice::None));
    }

    #[test]
    fn test_no_resolver_for_unknown_template() {
        let registry = ConflictFileResolverRegistry::new();

        let choice = registry.get_resolver_choice("unknown", "package.json");
        assert!(matches!(choice, ResolverChoice::None));
    }

    #[test]
    fn test_multiple_resolvers_first_match_wins() {
        let mut registry = ConflictFileResolverRegistry::new();
        let resolver1 = ResolverInstance {
            id: "json-merger".to_string(),
            docker_ref: "atomi/json-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({"strategy": "deep"}),
            file_patterns: vec!["*.json".to_string()],
        };
        let resolver2 = ResolverInstance {
            id: "line-merger".to_string(),
            docker_ref: "atomi/line-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({}),
            file_patterns: vec![".gitignore".to_string()],
        };

        registry.register("template-a".to_string(), vec![resolver1, resolver2]);

        // JSON files should match first resolver
        let choice = registry.get_resolver_choice("template-a", "package.json");
        if let ResolverChoice::Some(r) = choice {
            assert_eq!(r.id, "json-merger");
        } else {
            panic!("Expected resolver");
        }

        // Gitignore should match second resolver
        let choice = registry.get_resolver_choice("template-a", ".gitignore");
        if let ResolverChoice::Some(r) = choice {
            assert_eq!(r.id, "line-merger");
        } else {
            panic!("Expected resolver");
        }
    }

    #[test]
    fn test_globstar_pattern() {
        let mut registry = ConflictFileResolverRegistry::new();
        let resolver = create_test_resolver(vec!["**/*.json"]);

        registry.register("template-a".to_string(), vec![resolver]);

        // Should match at any depth
        assert!(matches!(
            registry.get_resolver_choice("template-a", "package.json"),
            ResolverChoice::Some(_)
        ));
        assert!(matches!(
            registry.get_resolver_choice("template-a", "src/package.json"),
            ResolverChoice::Some(_)
        ));
        assert!(matches!(
            registry.get_resolver_choice("template-a", "src/nested/deep/config.json"),
            ResolverChoice::Some(_)
        ));
    }
}
