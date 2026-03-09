//! Consensus algorithm for resolver selection
//!
//! Key insight: "No resolver" is a valid resolver choice and must participate in consensus.

use crate::conflict_file_resolver::models::{ResolverChoice, ResolverInstance, TemplateInfo};

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Unique key for resolver instance comparison
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ResolverInstanceKey {
    docker_ref: String,
    docker_tag: String,
    config_hash: u64,
}

impl ResolverInstanceKey {
    fn from_instance(instance: &ResolverInstance) -> Self {
        let config_hash = Self::hash_json(&instance.config);
        ResolverInstanceKey {
            docker_ref: instance.docker_ref.clone(),
            docker_tag: instance.docker_tag.clone(),
            config_hash,
        }
    }

    fn hash_json(value: &serde_json::Value) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.to_string().hash(&mut hasher);
        hasher.finish()
    }
}

/// Result of consensus determination
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusResult {
    /// All variations agree on this resolver instance
    Agreed(ResolverInstance),
    /// All variations have no resolver
    AllNone,
    /// No consensus - some have resolver, some don't
    NoConsensus {
        with_resolver: Vec<(TemplateInfo, ResolverInstance)>,
        without_resolver: Vec<TemplateInfo>,
    },
    /// Ambiguous - multiple different resolvers configured
    Ambiguous {
        resolvers: Vec<(TemplateInfo, ResolverInstance)>,
    },
}

/// Determine consensus among resolver choices for a file
///
/// # Arguments
/// * `choices` - List of (TemplateInfo, ResolverChoice) pairs
///
/// # Returns
/// * `ConsensusResult` indicating the consensus state
///
/// # Consensus Rules
/// | All Variations Agree?                  | Action                            |
/// |----------------------------------------|--------------------------------- |
/// | ALL = same resolver instance             | Use that resolver                 |
/// | ALL = none                               | LWW (`lww_all_no_resolver`)       |
/// | MIXED (some X, some none)                | LWW (`lww_no_consensus`)          |
/// | MIXED (X vs Y, different resolvers)      | LWW (`lww_ambiguous_resolver`)    |
/// | MIXED (X with configA vs X with configB)    | LWW (`lww_ambiguous_resolver`)    |
pub fn determine_consensus(choices: Vec<(TemplateInfo, ResolverChoice)>) -> ConsensusResult {
    if choices.is_empty() {
        return ConsensusResult::AllNone;
    }

    let mut with_resolver: Vec<(TemplateInfo, ResolverInstance)> = Vec::new();
    let mut without_resolver: Vec<TemplateInfo> = Vec::new();
    let mut resolver_groups: HashMap<ResolverInstanceKey, Vec<(TemplateInfo, ResolverInstance)>> =
        HashMap::new();

    for (template_info, choice) in choices {
        match choice {
            ResolverChoice::None => {
                without_resolver.push(template_info);
            }
            ResolverChoice::Some(resolver) => {
                let key = ResolverInstanceKey::from_instance(&resolver);
                with_resolver.push((template_info.clone(), resolver.clone()));
                resolver_groups
                    .entry(key)
                    .or_default()
                    .push((template_info, resolver));
            }
        }
    }

    // Case 1: All variations agree on same resolver
    if without_resolver.is_empty() && resolver_groups.len() == 1 {
        let (_, group) = resolver_groups.iter().next().unwrap();
        if group.len() == with_resolver.len() {
            // All have the same resolver
            let resolver = group[0].1.clone();
            return ConsensusResult::Agreed(resolver);
        }
    }

    // Case 2: All variations have no resolver
    if resolver_groups.is_empty() {
        return ConsensusResult::AllNone;
    }

    // Case 3: No consensus - some have resolver, some don't
    if !with_resolver.is_empty() && !without_resolver.is_empty() {
        return ConsensusResult::NoConsensus {
            with_resolver,
            without_resolver,
        };
    }

    // Case 4: Multiple different resolvers
    ConsensusResult::Ambiguous {
        resolvers: resolver_groups.into_values().flatten().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_template_info(template_id: &str) -> TemplateInfo {
        TemplateInfo {
            template_id: template_id.to_string(),
            template_version: 1,
            layer: 0,
        }
    }

    fn create_resolver_instance(id: &str) -> ResolverInstance {
        ResolverInstance {
            id: id.to_string(),
            docker_ref: "atomi/json-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({}),
            file_patterns: vec!["package.json".to_string()],
        }
    }

    #[test]
    fn test_consensus_all_agree() {
        let choices = vec![
            (
                create_template_info("template-a"),
                ResolverChoice::Some(create_resolver_instance("resolver-1")),
            ),
            (
                create_template_info("template-b"),
                ResolverChoice::Some(create_resolver_instance("resolver-1")),
            ),
        ];

        let result = determine_consensus(choices);
        if let ConsensusResult::Agreed(resolver) = &result {
            assert_eq!(resolver.id, "resolver-1");
        } else {
            panic!("Expected Agreed result, got: {result:?}");
        }
    }

    #[test]
    fn test_consensus_all_none() {
        let choices = vec![
            (create_template_info("template-a"), ResolverChoice::None),
            (create_template_info("template-b"), ResolverChoice::None),
        ];

        let result = determine_consensus(choices);
        assert!(matches!(result, ConsensusResult::AllNone));
    }

    #[test]
    fn test_consensus_no_consensus() {
        let choices = vec![
            (
                create_template_info("template-a"),
                ResolverChoice::Some(create_resolver_instance("resolver-1")),
            ),
            (create_template_info("template-b"), ResolverChoice::None),
        ];

        let result = determine_consensus(choices);
        if let ConsensusResult::NoConsensus {
            with_resolver,
            without_resolver,
        } = &result
        {
            assert_eq!(with_resolver.len(), 1);
            assert_eq!(without_resolver.len(), 1);
            // Verify resolver info is preserved
            assert_eq!(with_resolver[0].1.id, "resolver-1");
        } else {
            panic!("Expected NoConsensus result, got: {result:?}");
        }
    }

    #[test]
    fn test_consensus_ambiguous() {
        let resolver1 = ResolverInstance {
            id: "resolver-1".to_string(),
            docker_ref: "atomi/json-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({}),
            file_patterns: vec!["*.json".to_string()],
        };
        let resolver2 = ResolverInstance {
            id: "resolver-2".to_string(),
            docker_ref: "atomi/line-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({}),
            file_patterns: vec![".gitignore".to_string()],
        };

        let choices = vec![
            (
                create_template_info("template-a"),
                ResolverChoice::Some(resolver1),
            ),
            (
                create_template_info("template-b"),
                ResolverChoice::Some(resolver2),
            ),
        ];

        let result = determine_consensus(choices);
        if let ConsensusResult::Ambiguous { resolvers } = &result {
            assert_eq!(resolvers.len(), 2);
        } else {
            panic!("Expected Ambiguous result, got: {result:?}");
        }
    }

    #[test]
    fn test_consensus_different_configs() {
        let resolver1 = ResolverInstance {
            id: "resolver-1".to_string(),
            docker_ref: "atomi/json-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({"strategy": "deep"}),
            file_patterns: vec!["*.json".to_string()],
        };
        let resolver2 = ResolverInstance {
            id: "resolver-1".to_string(),
            docker_ref: "atomi/json-merger".to_string(),
            docker_tag: "1".to_string(),
            config: json!({"strategy": "shallow"}),
            file_patterns: vec!["*.json".to_string()],
        };

        let choices = vec![
            (
                create_template_info("template-a"),
                ResolverChoice::Some(resolver1),
            ),
            (
                create_template_info("template-b"),
                ResolverChoice::Some(resolver2),
            ),
        ];

        let result = determine_consensus(choices);
        // Different configs = different resolver instances, no consensus
        assert!(matches!(result, ConsensusResult::Ambiguous { .. }));
    }

    #[test]
    fn test_consensus_empty_choices() {
        let choices: Vec<(TemplateInfo, ResolverChoice)> = vec![];
        let result = determine_consensus(choices);
        assert!(matches!(result, ConsensusResult::AllNone));
    }
}
