use cyanprompt::domain::models::answer::Answer;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use crate::fs::VirtualFileSystem;
use crate::operations::TemplateOperator;

use super::layerer::VfsLayerer;
use super::resolver::DependencyResolver;
use super::state::CompositionState;

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

    /// Get a reference to the VFS operations
    pub fn get_vfs(&self) -> &dyn crate::fs::Vfs {
        self.template_operator.vfs.as_ref()
    }

    /// Get a reference to the template history
    pub fn get_template_history(&self) -> &dyn crate::template::TemplateHistory {
        self.template_operator.template_history.as_ref()
    }

    // =========================================================================
    // Unified Batch Processing Methods (v2 spec)
    // =========================================================================

    /// Execute a single template spec and return VFS + final state + session IDs.
    /// This is the core primitive - pure function, no side effects.
    /// Dependencies are resolved in post-order and layered internally.
    /// Returns the final CompositionState which contains answers after Q&A.
    pub fn execute_template(
        &self,
        template: &cyanregistry::http::models::template_res::TemplateVersionRes,
        answers: &HashMap<String, Answer>,
        deterministic_states: &HashMap<String, String>,
    ) -> Result<(VirtualFileSystem, CompositionState, Vec<String>), Box<dyn Error + Send>> {
        let templates = self.dependency_resolver.resolve_dependencies(template)?;

        let shared_state = CompositionState {
            shared_answers: answers.clone(),
            shared_deterministic_states: deterministic_states.clone(),
            execution_order: Vec::new(),
        };

        let (vfs, final_state, session_ids) =
            self.execute_composition(&templates, &shared_state)?;
        Ok((vfs, final_state, session_ids))
    }

    /// Layer merge a list of VFS into one (LWW semantics).
    pub fn layer_merge(
        &self,
        vfs_list: &[VirtualFileSystem],
    ) -> Result<VirtualFileSystem, Box<dyn Error + Send>> {
        self.vfs_layerer.layer_merge(vfs_list)
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
}
