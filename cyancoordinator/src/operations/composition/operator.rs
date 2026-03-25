use cyanprompt::domain::models::answer::Answer;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::client::CyanCoordinatorClient;
use crate::conflict_file_resolver::{
    ConflictFileResolverRegistry, FileConflictEntry, ResolverInstance, TemplateInfo,
};
use crate::fs::VirtualFileSystem;
use crate::operations::TemplateOperator;

use super::layerer::{DefaultVfsLayerer, ResolverAwareLayerer, VfsLayerer};
use super::resolver::{DependencyResolver, ResolvedDependency};
use super::state::CompositionState;

/// Composition operator for recursive template execution
pub struct CompositionOperator {
    template_operator: TemplateOperator,
    dependency_resolver: Box<dyn DependencyResolver>,
    vfs_layerer: Box<dyn VfsLayerer>,
    /// Optional client for resolver-aware layering
    client: Option<CyanCoordinatorClient>,
    /// File conflicts tracked during the last composition
    file_conflicts: Vec<FileConflictEntry>,
}

impl CompositionOperator {
    pub fn new(
        template_operator: TemplateOperator,
        dependency_resolver: Box<dyn DependencyResolver>,
        vfs_layerer: Box<dyn VfsLayerer>,
    ) -> Self {
        Self {
            template_operator,
            dependency_resolver,
            vfs_layerer,
            client: None,
            file_conflicts: Vec::new(),
        }
    }

    /// Create a composition operator with resolver-aware layering
    pub fn with_client(
        template_operator: TemplateOperator,
        dependency_resolver: Box<dyn DependencyResolver>,
        client: CyanCoordinatorClient,
    ) -> Self {
        Self {
            template_operator,
            dependency_resolver,
            vfs_layerer: Box::new(DefaultVfsLayerer),
            client: Some(client),
            file_conflicts: Vec::new(),
        }
    }

    /// Build a resolver registry from template response data
    fn build_resolver_registry(
        dependencies: &[ResolvedDependency],
    ) -> ConflictFileResolverRegistry {
        let mut registry = ConflictFileResolverRegistry::new();

        for dep in dependencies {
            let template = &dep.template;
            let template_id = template.principal.id.clone();
            let resolvers: Vec<ResolverInstance> = template
                .resolvers
                .iter()
                .map(|r| ResolverInstance {
                    id: r.id.clone(),
                    docker_ref: r.docker_reference.clone(),
                    docker_tag: r.docker_tag.clone(),
                    config: r.config.clone(),
                    file_patterns: r.files.clone(),
                })
                .collect();

            if !resolvers.is_empty() {
                registry.register(template_id, resolvers);
            }
        }

        registry
    }

    /// Build template info list for layerer from template response data
    fn build_template_infos(dependencies: &[ResolvedDependency]) -> Vec<TemplateInfo> {
        dependencies
            .iter()
            .enumerate()
            .map(|(idx, dep)| TemplateInfo {
                template_id: dep.template.principal.id.clone(),
                template_version: dep.template.principal.version,
                layer: idx as i32,
            })
            .collect()
    }

    /// Get file conflicts from the last composition
    pub fn get_file_conflicts(&self) -> &[FileConflictEntry] {
        &self.file_conflicts
    }

    /// Execute a composition of templates (recursive dependencies)
    fn execute_composition(
        &mut self,
        dependencies: &[ResolvedDependency],
        initial_shared_state: &CompositionState,
    ) -> Result<(VirtualFileSystem, CompositionState, Vec<String>), Box<dyn Error + Send>> {
        let mut shared_state = initial_shared_state.clone();
        let mut vfs_outputs = Vec::new();
        let mut all_session_ids = Vec::new();

        // Clear previous conflicts
        self.file_conflicts.clear();

        for dep in dependencies {
            let template = &dep.template;

            // Check if template has execution artifacts (properties field)
            if template.principal.properties.is_none() {
                println!(
                    "⏭️ Skipping template: {}/{} (v{}) - no execution artifacts (group template)",
                    template.template.name,
                    template.template.name, // TODO: Need username
                    template.principal.version
                );
                // Update execution order tracking even for skipped templates
                shared_state
                    .execution_order
                    .push(template.principal.id.clone());
                continue;
            }

            println!(
                "🚀 Executing template: {}/{} (v{})",
                template.template.name,
                template.template.name, // TODO: Need username
                template.principal.version
            );

            // Generate session for this template
            let session_id = self.template_operator.session_id_generator.generate();

            // Merge preset answers into shared_answers for this template only
            let mut template_answers = shared_state.shared_answers.clone();
            for (key, answer) in &dep.preset_answers {
                template_answers
                    .entry(key.clone())
                    .or_insert(answer.clone());
            }

            // Execute template with current shared state (preset answers fill gaps)
            let (archive_data, template_state, actual_session_id) =
                self.template_operator.template_executor.execute_template(
                    template,
                    &session_id,
                    Some(&template_answers),
                    Some(&shared_state.shared_deterministic_states),
                )?;

            // Unpack to VFS
            let vfs = self.template_operator.vfs.unpack_archive(archive_data)?;
            vfs_outputs.push(vfs);

            // Update shared state with results
            shared_state.update_from_template_state(&template_state, template.principal.id.clone());

            // Track session for cleanup
            all_session_ids.push(actual_session_id);
        }

        // Layer all VFS outputs
        let layered_vfs = if vfs_outputs.is_empty() {
            // No templates produced output (all were group templates)
            println!("ℹ️ No execution artifacts produced - all templates were group templates");
            VirtualFileSystem::new()
        } else if let Some(ref client) = self.client {
            // Use resolver-aware layering
            // Vertical layering: collect resolvers from ALL templates in dependency tree
            let registry = Self::build_resolver_registry(dependencies);
            let template_infos = Self::build_template_infos(dependencies);
            let layerer = ResolverAwareLayerer::new(registry, template_infos, client.clone());

            let result = layerer.layer_merge(&vfs_outputs)?;

            // Track conflicts for state writing
            self.file_conflicts = layerer.get_conflicts();

            result
        } else {
            // Use default layering (LWW)
            self.vfs_layerer.layer_merge(&vfs_outputs)?
        };

        Ok((layered_vfs, shared_state, all_session_ids))
    }

    /// Get a reference to the VFS operations
    pub fn get_vfs(&self) -> &dyn crate::fs::Vfs {
        self.template_operator.vfs.as_ref()
    }

    /// Get a reference to the template history
    pub fn get_template_history(&self) -> &dyn crate::template::TemplateHistory {
        self.template_operator.template_history.as_ref()
    }

    // =========================================================================
    // Unified Batch Processing Methods (v2/v3 spec)
    // =========================================================================

    /// Execute a single template spec and return VFS + final state + session IDs + commands.
    /// This is the core primitive - pure function, no side effects.
    /// Dependencies are resolved in post-order and layered internally.
    /// Returns the final CompositionState which contains answers after Q&A,
    /// and commands collected from all resolved dependencies in post-order.
    #[allow(clippy::type_complexity)]
    pub fn execute_template(
        &mut self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        answers: &HashMap<String, Answer>,
        deterministic_states: &HashMap<String, String>,
    ) -> Result<
        (
            VirtualFileSystem,
            CompositionState,
            Vec<String>,
            Vec<String>,
        ),
        Box<dyn Error + Send>,
    > {
        let dependencies = self.dependency_resolver.resolve_dependencies(template)?;

        let shared_state = CompositionState {
            shared_answers: answers.clone(),
            shared_deterministic_states: deterministic_states.clone(),
            execution_order: Vec::new(),
        };

        let (vfs, final_state, session_ids) =
            self.execute_composition(&dependencies, &shared_state)?;
        let commands = Self::collect_commands(&dependencies);
        Ok((vfs, final_state, session_ids, commands))
    }

    /// Layer merge a list of VFS into one (LWW semantics).
    pub fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.vfs_layerer.layer_merge(vfs_list)
    }

    /// Horizontal layering with resolver support.
    /// Collects resolvers ONLY from root templates being merged (not from dependencies).
    /// This is used when merging multiple independent templates in batch processing.
    pub fn layer_merge_with_resolvers(
        &mut self,
        vfs_list: &[VirtualFileSystem],
        root_templates: &[cyanregistry::http::models::template_res::TemplateVersionRes],
        client: &CyanCoordinatorClient,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        if vfs_list.is_empty() {
            return Ok(VirtualFileSystem::new());
        }

        if vfs_list.len() == 1 {
            return Ok(vfs_list[0].clone());
        }

        // Convert root templates to ResolvedDependency for helper functions
        // (preset_answers are not applicable for horizontal layering)
        let root_dependencies: Vec<ResolvedDependency> = root_templates
            .iter()
            .map(|t| ResolvedDependency {
                template: t.clone(),
                preset_answers: HashMap::new(),
            })
            .collect();

        // Build resolver registry from ONLY root templates (horizontal layering scope)
        let registry = Self::build_resolver_registry(&root_dependencies);
        let template_infos = Self::build_template_infos(&root_dependencies);

        // Use resolver-aware layerer
        let layerer = ResolverAwareLayerer::new(registry, template_infos, client.clone());
        let result = layerer.layer_merge(vfs_list)?;

        // Track conflicts for state writing
        self.file_conflicts = layerer.get_conflicts();

        Ok(result)
    }

    /// 3-way merge: (base, local, incoming) -> merged.
    pub fn merge(
        &self,
        base: &VirtualFileSystem,
        local: &VirtualFileSystem,
        incoming: &VirtualFileSystem,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.template_operator.vfs.merge(base, local, incoming)
    }

    /// Load local files from target directory.
    pub fn load_local_files(
        &self,
        target_dir: &Path,
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.template_operator.vfs.load_local_files(target_dir, &[])
    }

    /// Write VFS to disk.
    pub fn write_to_disk(
        &self,
        target_dir: &Path,
        vfs: &VirtualFileSystem,
    ) -> Result<(), Box<dyn Error + Send>> {
        self.template_operator.vfs.write_to_disk(target_dir, vfs)
    }

    /// Delete files that were present before merge but absent after merge.
    pub fn cleanup_deleted_files(
        &self,
        target_dir: &Path,
        local_vfs: &VirtualFileSystem,
        merged_vfs: &VirtualFileSystem,
    ) -> Result<Vec<std::path::PathBuf>, Box<dyn Error + Send>> {
        self.template_operator
            .vfs
            .cleanup_deleted_files(target_dir, local_vfs, merged_vfs)
    }

    // =========================================================================
    // Command Collection (for post-composition execution)
    // =========================================================================

    /// Collect commands from resolved dependencies in post-order.
    /// Iterates over dependencies (already in post-order from resolve_dependencies),
    /// collects non-empty commands from each template, and flattens into a single vec.
    pub fn collect_commands(dependencies: &[ResolvedDependency]) -> Vec<String> {
        let mut commands = Vec::new();
        for dep in dependencies {
            let template_commands = &dep.template.commands;
            if !template_commands.is_empty() {
                commands.extend(template_commands.iter().cloned());
            }
        }
        commands
    }

    /// Collect commands from template version responses.
    /// Used by batch_process where commands are collected from raw template results.
    pub fn collect_commands_from_templates(templates: &[TemplateVersionRes]) -> Vec<String> {
        let mut commands = Vec::new();
        for template in templates {
            if !template.commands.is_empty() {
                commands.extend(template.commands.iter().cloned());
            }
        }
        commands
    }
}
