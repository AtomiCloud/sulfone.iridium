//! Conflict File Resolver Module
//!
//! This module provides resolver-based conflict resolution for VFS layering.
//! When multiple templates produce the same file, resolvers can be used to
//! merge the variations when consensus is reached.

mod consensus;
mod models;
mod registry;

pub use consensus::{ConsensusResult, determine_consensus};
pub use models::{
    ConflictResolution, FileConflictEntry, FileOrigin, ResolverChoice, ResolverFile, ResolverInput,
    ResolverInstance, ResolverInstanceInfo, ResolverOutput, TemplateInfo, TemplateResolverInfo,
    TemplateVariationInfo,
};
pub use registry::ConflictFileResolverRegistry;
