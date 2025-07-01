use cyanprompt::domain::models::answer::Answer;

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::rc::Rc;

use crate::fs::{Vfs, VirtualFileSystem};
use crate::session::SessionIdGenerator;
use crate::template::{TemplateExecutor, TemplateHistory};
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

pub mod composition;

/// Trait defining operations that can be performed on templates
pub trait TemplateOperations {
    /// Create a new project from a template
    fn create_new(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>;

    /// Rerun an existing template with the same version
    fn rerun(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>;

    /// Upgrade a template to a new version
    fn upgrade(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    ) -> Result<Vec<String>, Box<dyn Error + Send>>;
}

/// Implementation of TemplateOperations that handles template operations
pub struct TemplateOperator {
    pub session_id_generator: Box<dyn SessionIdGenerator>,
    pub template_executor: Box<dyn TemplateExecutor>,
    pub template_history: Box<dyn TemplateHistory>,
    pub vfs: Box<dyn Vfs>,
    pub registry_client: Rc<CyanRegistryClient>,
}

impl TemplateOperator {
    /// Create a new TemplateOperator with the given dependencies
    pub fn new(
        session_id_generator: Box<dyn SessionIdGenerator>,
        template_executor: Box<dyn TemplateExecutor>,
        template_history: Box<dyn TemplateHistory>,
        vfs: Box<dyn Vfs>,
        registry_client: Rc<CyanRegistryClient>,
    ) -> Self {
        Self {
            session_id_generator,
            template_executor,
            template_history,
            vfs,
            registry_client,
        }
    }

    /// Helper method to get previous template version
    fn get_previous_template(
        &self,
        template: &TemplateVersionRes,
        username: &str,
        previous_version: i64,
    ) -> Result<TemplateVersionRes, Box<dyn Error + Send>> {
        let registry = &self.registry_client;

        // Fetch the actual previous version from registry
        let template_name = template.template.name.clone();
        println!(
            "🔍 Fetching template '{}/{}:{}' from registry...",
            username, template_name, previous_version
        );
        let prev_template =
            registry.get_template(username.to_string(), template_name, Some(previous_version))?;
        println!("✅ Retrieved previous template version from registry");
        Ok(prev_template)
    }
}

impl TemplateOperations for TemplateOperator {
    fn create_new(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!("✨ Creating a new project from template");

        // Generate a new session ID
        let new_session_id = self.session_id_generator.generate();

        // Execute the template with fresh QA session
        let (archive_data, template_state, actual_session_id) = self
            .template_executor
            .execute_template(template, &new_session_id, None, None)?;

        // Unpack the archive into VFS
        let incoming_vfs = self.vfs.unpack_archive(archive_data)?;

        // Create an empty base VFS for comparison
        let base_vfs = VirtualFileSystem::new();

        // Load any existing files that match paths in incoming_vfs
        let paths = incoming_vfs.get_paths();
        let local_vfs = self.vfs.load_local_files(target_dir, &paths)?;

        // Merge with base=empty, local=target folder, incoming=VFS
        let merged_vfs = self.vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?;

        // Write the merged VFS to disk
        self.vfs.write_to_disk(target_dir, &merged_vfs)?;

        // Save template metadata
        self.template_history.save_template_metadata(
            target_dir,
            template,
            &template_state,
            username,
        )?;

        println!("✅ Project created successfully");

        // Return the session ID for cleanup
        Ok(vec![actual_session_id])
    }

    fn rerun(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!("🔄 Re-running template (same version {})", previous_version);

        // Generate session IDs for both executions
        let prev_session_id = self.session_id_generator.generate();
        let curr_session_id = self.session_id_generator.generate();

        // Get the previous template using our helper method
        let previous_template = self.get_previous_template(template, username, previous_version)?;

        // First, recreate the previous template VFS state using saved answers and states
        println!("🏗️ Recreating previous template state");
        let (prev_archive_data, _, prev_actual_session_id) =
            self.template_executor.execute_template(
                &previous_template,
                &prev_session_id,
                Some(&previous_answers),
                Some(&previous_states),
            )?;

        // Second, execute the template with fresh Q&A
        println!("🏗️ Running template with new answers");
        let (curr_archive_data, template_state, curr_actual_session_id) =
            self.template_executor.execute_template(
                template,
                &curr_session_id,
                None, // No answers - user will provide fresh answers
                None,
            )?;

        // Unpack the archive into base VFS
        let base_vfs = self.vfs.unpack_archive(prev_archive_data)?;

        // Unpack the archive into incoming VFS
        let incoming_vfs = self.vfs.unpack_archive(curr_archive_data)?;

        // Load the current state of files from target directory
        let all_paths = Vec::new();
        let local_vfs = self.vfs.load_local_files(target_dir, &all_paths)?;

        // Perform 3-way merge with base=prev template, local=target folder, incoming=current template
        let merged_vfs = self.vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?;

        // Write the merged VFS to disk
        self.vfs.write_to_disk(target_dir, &merged_vfs)?;

        // Save updated template metadata
        self.template_history.save_template_metadata(
            target_dir,
            template,
            &template_state,
            username,
        )?;

        println!("✅ Project recreated successfully with new answers");

        // Return both session IDs for cleanup
        Ok(vec![prev_actual_session_id, curr_actual_session_id])
    }

    fn upgrade(
        &self,
        template: &TemplateVersionRes,
        target_dir: &Path,
        username: &str,
        previous_version: i64,
        previous_answers: HashMap<String, Answer>,
        previous_states: HashMap<String, String>,
    ) -> Result<Vec<String>, Box<dyn Error + Send>> {
        println!(
            "🔄 Upgrading from version {} to {}",
            previous_version, template.principal.version
        );

        // First, execute the old template version using saved answers to get the base VFS
        println!("🏗️ Recreating previous template version");

        // Generate session IDs for both executions
        let prev_session_id = self.session_id_generator.generate();
        let curr_session_id = self.session_id_generator.generate();

        // Get the previous template using our helper method
        let previous_template = self.get_previous_template(template, username, previous_version)?;

        let (prev_archive_data, _, prev_actual_session_id) =
            self.template_executor.execute_template(
                &previous_template,
                &prev_session_id,
                Some(&previous_answers),
                Some(&previous_states),
            )?;

        // Unpack the archive into base VFS
        let base_vfs = self.vfs.unpack_archive(prev_archive_data)?;

        // Second, execute the new template version using saved answers where possible
        println!("🏗️ Creating new template version");
        let (curr_archive_data, template_state, curr_actual_session_id) =
            self.template_executor.execute_template(
                template,
                &curr_session_id,
                Some(&previous_answers),
                Some(&previous_states),
            )?;

        // Unpack the archive into incoming VFS
        let incoming_vfs = self.vfs.unpack_archive(curr_archive_data)?;

        // Get all paths that should be considered for the local VFS (union of base and incoming)
        let all_paths = Vec::new();

        // Load the current state of files from target directory
        let local_vfs = self.vfs.load_local_files(target_dir, &all_paths)?;

        // Perform 3-way merge with base=prev template, local=target folder, incoming=current template
        let merged_vfs = self.vfs.merge(&base_vfs, &local_vfs, &incoming_vfs)?;

        // Write the merged VFS to disk
        self.vfs.write_to_disk(target_dir, &merged_vfs)?;

        // Save updated template metadata
        self.template_history.save_template_metadata(
            target_dir,
            template,
            &template_state,
            username,
        )?;

        println!("✅ Project upgraded successfully");

        // Return both session IDs for cleanup
        Ok(vec![prev_actual_session_id, curr_actual_session_id])
    }
}
