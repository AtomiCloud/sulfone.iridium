//! Models for conflict file resolution
//!
//! These models match the Helium SDK structure for resolver input/output.

use serde::{Deserialize, Serialize};

/// Unique resolver instance identified by docker reference, tag, and config
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolverInstance {
    /// Resolver ID from the registry
    pub id: String,
    /// Docker reference (e.g., "atomi/json-merger")
    pub docker_ref: String,
    /// Docker tag (e.g., "1")
    pub docker_tag: String,
    /// Config passed to resolver at runtime
    pub config: serde_json::Value,
    /// Glob patterns for files this resolver handles
    pub file_patterns: Vec<String>,
}

/// Represents a resolver choice for a file - either None or a specific resolver
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverChoice {
    /// No resolver configured for this file
    None,
    /// Specific resolver instance
    Some(ResolverInstance),
}

/// Template metadata for tracking file origins
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct TemplateInfo {
    /// Template ID
    pub template_id: String,
    /// Template version
    pub template_version: i64,
    /// Layer index (order in composition)
    pub layer: i32,
}

/// File origin metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileOrigin {
    /// Template that produced this file
    pub template: TemplateInfo,
}

/// Single file variation from a template
#[derive(Debug, Clone, Serialize)]
pub struct ResolverFile {
    /// File path
    pub path: String,
    /// File content
    pub content: String,
    /// Origin metadata
    pub origin: FileOrigin,
}

/// Resolver input - matches Helium SDK ResolverInput
#[derive(Debug, Clone, Serialize)]
pub struct ResolverInput {
    /// Config for this resolver instance
    pub config: serde_json::Value,
    /// File variations to resolve
    pub files: Vec<ResolverFile>,
}

/// Resolver output - matches Helium SDK ResolverOutput
#[derive(Debug, Clone, Deserialize)]
pub struct ResolverOutput {
    /// Resolved file path
    pub path: String,
    /// Resolved file content
    pub content: String,
}

/// Resolution type for conflict tracking
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Resolver successfully resolved
    Resolver,
    /// LWW - all variations have no resolver
    LwwAllNoResolver,
    /// LWW - some have resolver, some don't
    LwwNoConsensus,
    /// LWW - multiple different resolvers
    LwwAmbiguousResolver,
}

/// Entry for tracking file conflicts in state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConflictEntry {
    /// File path
    pub path: String,
    /// Resolution type used
    pub resolution: ConflictResolution,
    /// Resolver used (if resolution is Resolver)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolver_used: Option<ResolverInstanceInfo>,
    /// Templates with resolver (for LwwNoConsensus)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with_resolver: Option<Vec<TemplateResolverInfo>>,
    /// Templates without resolver (for LwwNoConsensus)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub without_resolver: Option<Vec<String>>,
    /// Winning template (for LWW resolutions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner_template: Option<String>,
    /// All variations that conflicted
    pub variations: Vec<TemplateVariationInfo>,
}

/// Resolver instance info for state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolverInstanceInfo {
    pub id: String,
    pub docker_ref: String,
    pub docker_tag: String,
    pub config: serde_json::Value,
}

/// Template with resolver info for conflict tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateResolverInfo {
    pub template_id: String,
    pub docker_ref: String,
}

/// Template variation info for conflict tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariationInfo {
    pub template_id: String,
}

impl From<&ResolverInstance> for ResolverInstanceInfo {
    fn from(resolver: &ResolverInstance) -> Self {
        ResolverInstanceInfo {
            id: resolver.id.clone(),
            docker_ref: resolver.docker_ref.clone(),
            docker_tag: resolver.docker_tag.clone(),
            config: resolver.config.clone(),
        }
    }
}
