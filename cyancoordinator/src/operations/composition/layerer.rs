use std::cell::RefCell;
use std::collections::HashSet;
use std::error::Error;
use std::path::{Path, PathBuf};

use crate::client::CyanCoordinatorClient;
use crate::conflict_file_resolver::{
    ConflictFileResolverRegistry, ConflictResolution, ConsensusResult, FileConflictEntry,
    FileOrigin, ResolverChoice, ResolverFile, ResolverInput, ResolverInstance,
    ResolverInstanceInfo, TemplateInfo, TemplateResolverInfo, TemplateVariationInfo,
    determine_consensus,
};
use crate::fs::VirtualFileSystem;

/// Trait for VFS layering operations
pub trait VfsLayerer {
    fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>>;

    /// Get file conflicts from the last layer_merge operation
    /// Default implementation returns empty vec for backward compatibility
    fn get_conflicts(&self) -> Vec<FileConflictEntry> {
        Vec::new()
    }
}

/// Default implementation that overwrites in order (later templates win)
pub struct DefaultVfsLayerer;

impl VfsLayerer for DefaultVfsLayerer {
    fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        if vfs_list.is_empty() {
            return Ok(VirtualFileSystem::new());
        }

        // Start with first VFS
        let mut result = vfs_list[0].clone();

        // Layer each subsequent VFS (later ones overwrite earlier ones)
        for vfs in &vfs_list[1..] {
            for path in vfs.get_paths() {
                if let Some(content) = vfs.get_file(&path) {
                    result.add_file(path, content.clone());
                }
            }
        }

        println!(
            "🔄 Layered {} VFS outputs (later templates overwrite earlier ones)",
            vfs_list.len()
        );
        Ok(result)
    }
}

/// Resolver-aware VFS layerer that handles conflicts using resolvers
///
/// When multiple templates produce the same file:
/// 1. Collect resolver choices from each template for the conflicting file
/// 2. Run consensus algorithm
/// 3. If consensus reached, call resolver to merge
/// 4. If no consensus, fall back to LWW (Last-Write-Wins)
pub struct ResolverAwareLayerer {
    /// Registry mapping template IDs to their resolvers
    registry: ConflictFileResolverRegistry,
    /// Template info for each VFS index (template_id, version, layer)
    template_infos: Vec<TemplateInfo>,
    /// Client for making resolver HTTP calls
    client: CyanCoordinatorClient,
    /// Conflicts tracked during layer_merge
    conflicts: RefCell<Vec<FileConflictEntry>>,
}

impl ResolverAwareLayerer {
    /// Create a new resolver-aware layerer
    pub fn new(
        registry: ConflictFileResolverRegistry,
        template_infos: Vec<TemplateInfo>,
        client: CyanCoordinatorClient,
    ) -> Self {
        Self {
            registry,
            template_infos,
            client,
            conflicts: RefCell::new(Vec::new()),
        }
    }

    /// Collect all unique file paths across all VFS
    fn collect_all_paths(vfs_list: &[VirtualFileSystem]) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();

        for vfs in vfs_list {
            for path in vfs.get_paths() {
                if seen.insert(path.clone()) {
                    paths.push(path);
                }
            }
        }
        paths
    }

    /// Convert bytes to string for resolver input
    fn bytes_to_string(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).to_string()
    }

    /// Get all variations of a file from different VFS
    fn get_file_variations(
        &self,
        path: &Path,
        vfs_list: &[VirtualFileSystem],
    ) -> Vec<(TemplateInfo, ResolverChoice, Vec<u8>)> {
        let mut variations = Vec::new();
        let path_str = path.to_string_lossy();

        for (idx, vfs) in vfs_list.iter().enumerate() {
            if let Some(content) = vfs.get_file(path) {
                let template_info =
                    self.template_infos
                        .get(idx)
                        .cloned()
                        .unwrap_or_else(|| TemplateInfo {
                            template_id: format!("unknown-{idx}"),
                            template_version: 0,
                            layer: idx as i32,
                        });

                let resolver_choice = self
                    .registry
                    .get_resolver_choice(&template_info.template_id, &path_str);

                variations.push((template_info, resolver_choice, content.clone()));
            }
        }

        variations
    }

    /// Resolve file conflict using resolver HTTP call
    fn resolve_with_resolver(
        &self,
        resolver: &ResolverInstance,
        path: &Path,
        variations: &[(TemplateInfo, ResolverChoice, Vec<u8>)],
    ) -> Result<Vec<u8>, Box<dyn Error + Send>> {
        let path_str = path.to_string_lossy();
        let files: Vec<ResolverFile> = variations
            .iter()
            .map(|(template_info, _, content)| ResolverFile {
                path: path_str.to_string(),
                content: Self::bytes_to_string(content),
                origin: FileOrigin {
                    template: template_info.template_id.clone(),
                    layer: template_info.layer,
                },
            })
            .collect();

        let input = ResolverInput {
            config: resolver.config.clone(),
            files,
        };

        // Call resolver via HTTP
        let output = self.client.resolve_files(&resolver.id, &input)?;

        // Return resolved content
        Ok(output.content.into_bytes())
    }

    /// Create a FileConflictEntry for tracking
    #[allow(clippy::too_many_arguments)]
    fn create_conflict_entry(
        path: &Path,
        resolution: ConflictResolution,
        resolver_used: Option<&ResolverInstance>,
        with_resolver: Option<Vec<TemplateResolverInfo>>,
        without_resolver: Option<Vec<String>>,
        winner_template: Option<&str>,
        variations: &[(TemplateInfo, ResolverChoice, Vec<u8>)],
    ) -> FileConflictEntry {
        FileConflictEntry {
            path: path.to_string_lossy().to_string(),
            resolution,
            resolver_used: resolver_used.map(ResolverInstanceInfo::from),
            with_resolver,
            without_resolver,
            winner_template: winner_template.map(|s| s.to_string()),
            variations: variations
                .iter()
                .map(|(t, _, _)| TemplateVariationInfo {
                    template_id: t.template_id.clone(),
                })
                .collect(),
        }
    }
}

impl VfsLayerer for ResolverAwareLayerer {
    fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        // Clear previous conflicts
        self.conflicts.borrow_mut().clear();

        if vfs_list.is_empty() {
            return Ok(VirtualFileSystem::new());
        }

        if vfs_list.len() == 1 {
            return Ok(vfs_list[0].clone());
        }

        let all_paths = Self::collect_all_paths(vfs_list);
        let mut result = VirtualFileSystem::new();

        for path in &all_paths {
            let variations = self.get_file_variations(path, vfs_list);

            if variations.len() == 1 {
                // No conflict - just use the content
                let (_, _, content) = &variations[0];
                result.add_file(path.clone(), content.clone());
                continue;
            }

            // Multiple variations - need to determine consensus
            let choices: Vec<(TemplateInfo, ResolverChoice)> = variations
                .iter()
                .map(|(t, r, _)| (t.clone(), r.clone()))
                .collect();

            let consensus = determine_consensus(choices);
            let path_str = path.to_string_lossy();

            match consensus {
                ConsensusResult::Agreed(resolver) => {
                    // All agree on same resolver - resolve the conflict via HTTP
                    let resolved_content =
                        self.resolve_with_resolver(&resolver, path, &variations)?;
                    result.add_file(path.clone(), resolved_content);

                    // Track conflict resolution
                    let entry = Self::create_conflict_entry(
                        path,
                        ConflictResolution::Resolver,
                        Some(&resolver),
                        None,
                        None,
                        None,
                        &variations,
                    );
                    self.conflicts.borrow_mut().push(entry);
                    println!(
                        "✅ File {} resolved via resolver {}",
                        path_str, resolver.docker_ref
                    );
                }
                ConsensusResult::AllNone => {
                    // All have no resolver - LWW
                    if let Some((template_info, _, content)) = variations.last() {
                        result.add_file(path.clone(), content.clone());

                        // Track conflict resolution
                        let entry = Self::create_conflict_entry(
                            path,
                            ConflictResolution::LwwAllNoResolver,
                            None,
                            None,
                            None,
                            Some(&template_info.template_id),
                            &variations,
                        );
                        self.conflicts.borrow_mut().push(entry);
                        println!(
                            "⚠️ File {} has conflict - LWW (all no resolver): {}",
                            path_str, template_info.template_id
                        );
                    }
                }
                ConsensusResult::NoConsensus {
                    with_resolver: wr,
                    without_resolver: wor,
                } => {
                    // Some have resolver, some don't - LWW
                    if let Some((template_info, _, content)) = variations.last() {
                        result.add_file(path.clone(), content.clone());

                        // Track conflict resolution - now with resolver info preserved
                        let with_resolver_info: Vec<TemplateResolverInfo> = wr
                            .iter()
                            .map(|(t, resolver)| TemplateResolverInfo {
                                template_id: t.template_id.clone(),
                                docker_ref: format!(
                                    "{}:{}",
                                    resolver.docker_ref, resolver.docker_tag
                                ),
                            })
                            .collect();
                        let without_resolver_info: Vec<String> =
                            wor.iter().map(|t| t.template_id.clone()).collect();

                        let entry = Self::create_conflict_entry(
                            path,
                            ConflictResolution::LwwNoConsensus,
                            None,
                            Some(with_resolver_info),
                            Some(without_resolver_info),
                            Some(&template_info.template_id),
                            &variations,
                        );
                        self.conflicts.borrow_mut().push(entry);
                        println!(
                            "⚠️ File {} has conflict - LWW (no consensus): {}",
                            path_str, template_info.template_id
                        );
                    }
                }
                ConsensusResult::Ambiguous { resolvers: _ } => {
                    // Multiple different resolvers - LWW
                    if let Some((template_info, _, content)) = variations.last() {
                        result.add_file(path.clone(), content.clone());

                        // Track conflict resolution
                        let entry = Self::create_conflict_entry(
                            path,
                            ConflictResolution::LwwAmbiguousResolver,
                            None,
                            None,
                            None,
                            Some(&template_info.template_id),
                            &variations,
                        );
                        self.conflicts.borrow_mut().push(entry);
                        println!(
                            "⚠️ File {} has conflict - LWW (ambiguous resolvers): {}",
                            path_str, template_info.template_id
                        );
                    }
                }
            }
        }

        println!(
            "🔄 Layered {} VFS outputs with resolver-aware conflict resolution",
            vfs_list.len()
        );
        Ok(result)
    }

    fn get_conflicts(&self) -> Vec<FileConflictEntry> {
        self.conflicts.borrow().clone()
    }
}
