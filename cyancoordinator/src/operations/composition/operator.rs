use cyanprompt::domain::models::answer::Answer;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::fs::VirtualFileSystem;
use crate::operations::TemplateOperator;

use super::layerer::VfsLayerer;
use super::resolver::DependencyResolver;
use super::state::CompositionState;

/// Result of collecting VFS outputs for a single template
#[derive(Clone)]
pub struct TemplateVfsCollection {
    /// The template's principal ID
    pub template_id: String,
    /// The layered VFS from the previous version execution (for upgrades)
    pub prev_vfs: Option<VirtualFileSystem>,
    /// The layered VFS from the current version execution
    pub curr_vfs: VirtualFileSystem,
    /// Session IDs created during execution (for cleanup)
    pub session_ids: Vec<String>,
    /// The final composition state after execution
    pub final_state: CompositionState,
}

/// Composition operator for recursive template execution
pub struct CompositionOperator {
    template_operator: TemplateOperator,
    dependency_resolver: Box<dyn DependencyResolver>,
    vfs_layerer: Box<dyn VfsLayerer>,
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
        }
    }

    /// Execute a composition of templates (recursive dependencies)
    fn execute_composition(
        &self,
        templates: &[cyanregistry::http::models::template_res::TemplateVersionRes],
        initial_shared_state: &CompositionState,
    ) -> Result<(VirtualFileSystem, CompositionState, Vec<String>), Box<dyn Error + Send>> {
        let mut shared_state = initial_shared_state.clone();
        let mut vfs_outputs = Vec::new();
        let mut all_session_ids = Vec::new();

        for template in templates {
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

            // Execute template with current shared state
            let (archive_data, template_state, actual_session_id) =
                self.template_operator.template_executor.execute_template(
                    template,
                    &session_id,
                    Some(&shared_state.shared_answers),
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

        // Layer all VFS outputs (later templates overwrite earlier ones)
        let layered_vfs = if vfs_outputs.is_empty() {
            // No templates produced output (all were group templates)
            println!("ℹ️ No execution artifacts produced - all templates were group templates");
            VirtualFileSystem::new()
        } else {
            self.vfs_layerer.layer_merge(&vfs_outputs)?
        };

        Ok((layered_vfs, shared_state, all_session_ids))
    }

    /// Create new project from template composition
    pub fn create_new_composition(
        &self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        target_dir: &Path,
        username: &str,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!("✨ Creating new project from template composition");

        // 1. Resolve dependencies (post-order traversal)
        let templates = self.dependency_resolver.resolve_dependencies(template)?;

        // 2. Execute all templates with shared state
        let initial_state = CompositionState::new();
        let (layered_vfs, final_state, session_ids) =
            self.execute_composition(&templates, &initial_state)?;

        // 3. Merge with local files (same as current implementation)
        let base_vfs = VirtualFileSystem::new(); // Empty for new template
        let paths = layered_vfs.get_paths();
        let local_vfs = self
            .template_operator
            .vfs
            .load_local_files(target_dir, &paths)?;

        // Final 3-way merge
        let merged_vfs = self
            .template_operator
            .vfs
            .merge(&base_vfs, &local_vfs, &layered_vfs)?;

        // 4. Write to disk
        self.template_operator
            .vfs
            .write_to_disk(target_dir, &merged_vfs)?;

        // 5. Save template metadata (root template only)
        if let Some(root_template) = templates.last() {
            // Extract template state from final state
            let template_state =
                cyanprompt::domain::services::template::states::TemplateState::Complete(
                    cyanprompt::domain::models::cyan::Cyan {
                        processors: Vec::new(),
                        plugins: Vec::new(),
                    },
                    final_state.shared_answers.clone(),
                );

            self.template_operator
                .template_history
                .save_template_metadata(target_dir, root_template, &template_state, username)?;
        }

        println!(
            "✅ Project created successfully from {} templates",
            templates.len()
        );
        Ok(session_ids)
    }

    /// Upgrade template composition
    pub fn upgrade_composition(
        &self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!(
            "🔄 Upgrading template composition from version {} to {}",
            previous_version, template.principal.version
        );

        // 1. Get previous template version
        let previous_template =
            self.template_operator
                .get_previous_template(template, username, previous_version)?;

        // 2. Resolve dependencies for both versions
        let previous_templates = self
            .dependency_resolver
            .resolve_dependencies(&previous_template)?;
        let current_templates = self.dependency_resolver.resolve_dependencies(template)?;

        // 3. Execute previous composition
        println!("🏗️ Recreating previous template composition");
        let previous_shared_state = CompositionState {
            shared_answers: previous_answers,
            shared_deterministic_states: previous_states,
            execution_order: Vec::new(),
        };
        let (prev_layered_vfs, _, prev_session_ids) =
            self.execute_composition(&previous_templates, &previous_shared_state)?;

        // 4. Execute current composition
        println!("🏗️ Creating new template composition");
        let current_shared_state = CompositionState {
            shared_answers: previous_shared_state.shared_answers.clone(),
            shared_deterministic_states: previous_shared_state.shared_deterministic_states.clone(),
            execution_order: Vec::new(),
        };
        let (curr_layered_vfs, final_state, curr_session_ids) =
            self.execute_composition(&current_templates, &current_shared_state)?;

        // 5. 3-way merge
        let all_paths = Vec::new();
        let local_vfs = self
            .template_operator
            .vfs
            .load_local_files(target_dir, &all_paths)?;
        let merged_vfs =
            self.template_operator
                .vfs
                .merge(&prev_layered_vfs, &local_vfs, &curr_layered_vfs)?;

        // 6. Write to disk
        self.template_operator
            .vfs
            .write_to_disk(target_dir, &merged_vfs)?;

        // 7. Save updated metadata (root template only)
        if let Some(root_template) = current_templates.last() {
            let template_state =
                cyanprompt::domain::services::template::states::TemplateState::Complete(
                    cyanprompt::domain::models::cyan::Cyan {
                        processors: Vec::new(),
                        plugins: Vec::new(),
                    },
                    final_state.shared_answers.clone(),
                );

            self.template_operator
                .template_history
                .save_template_metadata(target_dir, root_template, &template_state, username)?;
        }

        // 8. Combine all session IDs for cleanup
        let mut all_session_ids = prev_session_ids;
        all_session_ids.extend(curr_session_ids);

        println!("✅ Template composition upgraded successfully");
        Ok(all_session_ids)
    }

    /// Rerun template composition with fresh Q&A
    pub fn rerun_composition(
        &self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!("🔄 Re-running template composition (same version {previous_version})");

        // Same as upgrade but use fresh Q&A for current execution
        let previous_template =
            self.template_operator
                .get_previous_template(template, username, previous_version)?;

        let previous_templates = self
            .dependency_resolver
            .resolve_dependencies(&previous_template)?;
        let current_templates = self.dependency_resolver.resolve_dependencies(template)?;

        // Execute previous composition with saved state
        let previous_shared_state = CompositionState {
            shared_answers: previous_answers,
            shared_deterministic_states: previous_states,
            execution_order: Vec::new(),
        };
        let (prev_layered_vfs, _, prev_session_ids) =
            self.execute_composition(&previous_templates, &previous_shared_state)?;

        // Execute current composition with FRESH Q&A (empty state)
        let fresh_state = CompositionState::new();
        let (curr_layered_vfs, final_state, curr_session_ids) =
            self.execute_composition(&current_templates, &fresh_state)?;

        // 3-way merge and write
        let all_paths = Vec::new();
        let local_vfs = self
            .template_operator
            .vfs
            .load_local_files(target_dir, &all_paths)?;
        let merged_vfs =
            self.template_operator
                .vfs
                .merge(&prev_layered_vfs, &local_vfs, &curr_layered_vfs)?;
        self.template_operator
            .vfs
            .write_to_disk(target_dir, &merged_vfs)?;

        // Save metadata
        if let Some(root_template) = current_templates.last() {
            let template_state =
                cyanprompt::domain::services::template::states::TemplateState::Complete(
                    cyanprompt::domain::models::cyan::Cyan {
                        processors: Vec::new(),
                        plugins: Vec::new(),
                    },
                    final_state.shared_answers.clone(),
                );

            self.template_operator
                .template_history
                .save_template_metadata(target_dir, root_template, &template_state, username)?;
        }

        let mut all_session_ids = prev_session_ids;
        all_session_ids.extend(curr_session_ids);

        println!("✅ Template composition re-run successfully with fresh answers");
        Ok(all_session_ids)
    }

    // =========================================================================
    // Batch VFS Collection Methods (no intermediate disk writes)
    // =========================================================================

    /// Collect VFS outputs for a single template upgrade WITHOUT writing to disk.
    /// This is used for batch processing where we collect all VFS outputs first,
    /// then do a single merge and write at the end.
    pub fn collect_upgrade_vfs(
        &self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    ) -> Result<TemplateVfsCollection, Box<dyn Error + Send>> {
        println!(
            "📦 Collecting VFS for template upgrade: {} from v{} to v{}",
            template.template.name, previous_version, template.principal.version
        );

        // 1. Get previous template version
        let previous_template =
            self.template_operator
                .get_previous_template(template, username, previous_version)?;

        // 2. Resolve dependencies for both versions
        let previous_templates = self
            .dependency_resolver
            .resolve_dependencies(&previous_template)?;
        let current_templates = self.dependency_resolver.resolve_dependencies(template)?;

        // 3. Execute previous composition
        let previous_shared_state = CompositionState {
            shared_answers: previous_answers,
            shared_deterministic_states: previous_states,
            execution_order: Vec::new(),
        };
        let (prev_layered_vfs, _, prev_session_ids) =
            self.execute_composition(&previous_templates, &previous_shared_state)?;

        // 4. Execute current composition
        let current_shared_state = CompositionState {
            shared_answers: previous_shared_state.shared_answers.clone(),
            shared_deterministic_states: previous_shared_state.shared_deterministic_states.clone(),
            execution_order: Vec::new(),
        };
        let (curr_layered_vfs, final_state, curr_session_ids) =
            self.execute_composition(&current_templates, &current_shared_state)?;

        // 5. Combine session IDs
        let mut all_session_ids = prev_session_ids;
        all_session_ids.extend(curr_session_ids);

        println!(
            "✅ Collected VFS for template {} (prev: {} files, curr: {} files)",
            template.template.name,
            prev_layered_vfs.get_paths().len(),
            curr_layered_vfs.get_paths().len()
        );

        Ok(TemplateVfsCollection {
            template_id: template.principal.id.clone(),
            prev_vfs: Some(prev_layered_vfs),
            curr_vfs: curr_layered_vfs,
            session_ids: all_session_ids,
            final_state,
        })
    }

    /// Collect VFS outputs for a single template creation (no previous version).
    /// This is used for batch processing when adding a new template to an existing project.
    pub fn collect_create_vfs(
        &self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        initial_state: Option<&CompositionState>,
    ) -> Result<TemplateVfsCollection, Box<dyn Error + Send>> {
        println!(
            "📦 Collecting VFS for new template: {} (v{})",
            template.template.name, template.principal.version
        );

        // Resolve dependencies
        let templates = self.dependency_resolver.resolve_dependencies(template)?;

        // Execute composition
        let state = initial_state.cloned().unwrap_or_default();
        let (layered_vfs, final_state, session_ids) =
            self.execute_composition(&templates, &state)?;

        println!(
            "✅ Collected VFS for new template {} ({} files)",
            template.template.name,
            layered_vfs.get_paths().len()
        );

        Ok(TemplateVfsCollection {
            template_id: template.principal.id.clone(),
            prev_vfs: None,
            curr_vfs: layered_vfs,
            session_ids,
            final_state,
        })
    }

    /// Layer merge multiple VFS outputs and perform a single 3-way merge with local files.
    /// This is the MERGE phase of the batch processing.
    pub fn layer_and_merge_vfs(
        &self,
        collections: &[TemplateVfsCollection],
        target_dir: &Path,
        is_upgrade: bool,
    ) -> Result<(VirtualFileSystem, Vec<String>), Box<dyn Error + Send>> {
        println!(
            "🔄 Layering {} template VFS outputs and performing 3-way merge",
            collections.len()
        );

        if collections.is_empty() {
            println!("⚠️ No VFS collections to merge");
            return Ok((VirtualFileSystem::new(), Vec::new()));
        }

        // Collect all VFS outputs
        let mut all_prev_vfs = Vec::new();
        let mut all_curr_vfs = Vec::new();
        let mut all_session_ids = Vec::new();

        for collection in collections {
            if let Some(prev_vfs) = &collection.prev_vfs {
                all_prev_vfs.push(prev_vfs.clone());
            }
            all_curr_vfs.push(collection.curr_vfs.clone());
            all_session_ids.extend(collection.session_ids.clone());
        }

        // Layer merge previous VFS outputs (if any)
        let master_prev_vfs = if all_prev_vfs.is_empty() {
            if is_upgrade {
                println!("⚠️ No previous VFS outputs for upgrade - using empty VFS");
            }
            VirtualFileSystem::new()
        } else {
            println!(
                "🔄 Layering {} previous VFS outputs (LWW semantics)",
                all_prev_vfs.len()
            );
            self.vfs_layerer.layer_merge(&all_prev_vfs)?
        };

        // Layer merge current VFS outputs
        let master_curr_vfs = if all_curr_vfs.is_empty() {
            println!("⚠️ No current VFS outputs - nothing to merge");
            return Ok((VirtualFileSystem::new(), all_session_ids));
        } else {
            println!(
                "🔄 Layering {} current VFS outputs (LWW semantics)",
                all_curr_vfs.len()
            );
            self.vfs_layerer.layer_merge(&all_curr_vfs)?
        };

        // Load local files
        let local_vfs = self
            .template_operator
            .vfs
            .load_local_files(target_dir, &[])?;

        // Perform 3-way merge
        println!("🔀 Performing 3-way merge (base=prev, local=local, incoming=curr)");
        let merged_vfs =
            self.template_operator
                .vfs
                .merge(&master_prev_vfs, &local_vfs, &master_curr_vfs)?;

        println!(
            "✅ Batch merge complete (merged VFS has {} files)",
            merged_vfs.get_paths().len()
        );

        Ok((merged_vfs, all_session_ids))
    }

    /// Get a reference to the VFS operations
    pub fn get_vfs(&self) -> &dyn crate::fs::Vfs {
        self.template_operator.vfs.as_ref()
    }

    /// Get a reference to the template history
    pub fn get_template_history(&self) -> &dyn crate::template::TemplateHistory {
        self.template_operator.template_history.as_ref()
    }
}
