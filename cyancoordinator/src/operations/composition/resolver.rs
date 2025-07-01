use std::error::Error;
use std::rc::Rc;

use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

/// Trait for dependency resolution
pub trait DependencyResolver {
    fn resolve_dependencies(
        &self,
        template: &TemplateVersionRes,
    ) -> Result<Vec<TemplateVersionRes>, Box<dyn Error + Send>>;
}

/// Default implementation that resolves dependencies via registry client
pub struct DefaultDependencyResolver {
    registry_client: Rc<CyanRegistryClient>,
}

impl DefaultDependencyResolver {
    pub fn new(registry_client: Rc<CyanRegistryClient>) -> Self {
        Self { registry_client }
    }

    /// Perform post-order traversal to flatten template dependency tree
    fn flatten_dependencies(
        &self,
        template: &TemplateVersionRes,
        visited: &mut Vec<String>,
    ) -> Result<Vec<TemplateVersionRes>, Box<dyn Error + Send>> {
        let mut flattened = Vec::new();

        // Sort dependencies by ID to ensure deterministic execution order
        let mut sorted_deps = template.templates.clone();
        sorted_deps.sort_by(|a, b| a.id.cmp(&b.id));

        println!(
            "🔄 Processing {} dependencies in deterministic order (sorted by ID)",
            sorted_deps.len()
        );

        // Process dependencies in deterministic order
        for dep in &sorted_deps {
            // Avoid infinite recursion (though user said no circular dependency detection needed)
            if visited.contains(&dep.id) {
                continue;
            }

            // Fetch dependency template
            println!("🔍 Fetching dependency template version ID: {}", dep.id);
            let dep_template = self
                .registry_client
                .get_template_version_by_id(dep.id.clone())?;
            println!(
                "✅ Retrieved dependency: {}/{} (v{})",
                dep_template.template.name,
                dep_template.template.name, // TODO: Need username
                dep_template.principal.version
            );

            visited.push(dep.id.clone());

            // Recursive call for nested dependencies
            let mut nested_deps = self.flatten_dependencies(&dep_template, visited)?;
            flattened.append(&mut nested_deps);

            // Add this dependency after its nested dependencies (post-order)
            flattened.push(dep_template);
        }

        Ok(flattened)
    }
}

impl DependencyResolver for DefaultDependencyResolver {
    fn resolve_dependencies(
        &self,
        template: &TemplateVersionRes,
    ) -> Result<Vec<TemplateVersionRes>, Box<dyn Error + Send>> {
        println!(
            "🔍 Resolving dependencies for template: {} (v{})",
            template.template.name, template.principal.version
        );

        let mut visited = Vec::new();
        let mut flattened = self.flatten_dependencies(template, &mut visited)?;

        // Add root template at the end (post-order)
        flattened.push(template.clone());

        println!(
            "📋 Deterministic template execution order (post-order, sorted by dependency ID):"
        );
        for (i, tmpl) in flattened.iter().enumerate() {
            println!(
                "  {}. {}/{} (v{}) [ID: {}]",
                i + 1,
                tmpl.template.name,
                tmpl.template.name, // TODO: Need username - might need to extract from user_id or use a different approach
                tmpl.principal.version,
                tmpl.principal.id
            );
        }

        Ok(flattened)
    }
}
