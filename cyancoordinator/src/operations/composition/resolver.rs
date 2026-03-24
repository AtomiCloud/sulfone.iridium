use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::rc::Rc;

use cyanprompt::domain::models::answer::Answer;
use cyanregistry::http::client::CyanRegistryClient;
use cyanregistry::http::models::template_res::TemplateVersionRes;

/// A dependency template with its preset answers (declared by the parent)
pub struct ResolvedDependency {
    pub template: TemplateVersionRes,
    pub preset_answers: HashMap<String, Answer>,
}

/// Convert a serde_json::Value to an Answer enum.
/// Returns None for unsupported types (caller should skip).
pub fn serde_json_value_to_answer(value: &serde_json::Value) -> Option<Answer> {
    match value {
        serde_json::Value::String(s) => Some(Answer::String(s.clone())),
        serde_json::Value::Bool(b) => Some(Answer::Bool(*b)),
        serde_json::Value::Array(arr) => {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if strings.len() == arr.len() {
                Some(Answer::StringArray(strings))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Trait for dependency resolution
pub trait DependencyResolver {
    fn resolve_dependencies(
        &self,
        template: &TemplateVersionRes,
    ) -> Result<Vec<ResolvedDependency>, Box<dyn Error + Send>>;
}

/// Flatten dependencies using a custom template fetcher (for testing).
/// This is a standalone function that can be tested directly without needing
/// a real registry client.
#[allow(clippy::type_complexity)]
pub fn flatten_dependencies_with_fetcher(
    template: &TemplateVersionRes,
    visited: &mut HashSet<String>,
    fetch_template: Rc<dyn Fn(String) -> Result<TemplateVersionRes, Box<dyn Error + Send>>>,
) -> Result<Vec<ResolvedDependency>, Box<dyn Error + Send>> {
    let mut flattened: Vec<ResolvedDependency> = Vec::new();
    flatten_impl(template, visited, &mut flattened, &*fetch_template)?;
    Ok(flattened)
}

/// Internal implementation that does the actual flattening.
/// The `flattened` parameter is shared across all recursive calls to enable
/// cross-branch duplicate detection and preset_answers merging.
#[allow(clippy::type_complexity)]
fn flatten_impl(
    template: &TemplateVersionRes,
    visited: &mut HashSet<String>,
    flattened: &mut Vec<ResolvedDependency>,
    fetch_template: &dyn Fn(String) -> Result<TemplateVersionRes, Box<dyn Error + Send>>,
) -> Result<(), Box<dyn Error + Send>> {
    // Sort dependencies by ID to ensure deterministic execution order
    let mut sorted_deps = template.templates.clone();
    sorted_deps.sort_by(|a, b| a.id.cmp(&b.id));

    // Process dependencies in deterministic order
    for dep_ref in &sorted_deps {
        // Extract preset_answers from the dependency ref
        let preset_answers: HashMap<String, Answer> = dep_ref
            .preset_answers
            .iter()
            .filter_map(|(key, value)| {
                serde_json_value_to_answer(value).map(|answer| (key.clone(), answer))
            })
            .collect();

        // R2 Fix: Check if this template_id was already added to flattened (cross-branch).
        // If so, MERGE preset_answers instead of skipping or adding duplicate.
        if let Some(existing) = flattened
            .iter_mut()
            .find(|d| d.template.principal.id == dep_ref.id)
        {
            for (key, answer) in preset_answers {
                existing.preset_answers.entry(key).or_insert(answer);
            }
            continue;
        }

        // Check if we've already processed this template_id to prevent infinite recursion
        if visited.contains(&dep_ref.id) {
            continue;
        }

        // Fetch dependency template
        let dep_template = fetch_template(dep_ref.id.clone())?;

        visited.insert(dep_ref.id.clone());

        // Recursive call for nested dependencies (shares the same flattened vector)
        flatten_impl(&dep_template, visited, flattened, fetch_template)?;

        // Add this dependency after its nested dependencies (post-order)
        flattened.push(ResolvedDependency {
            template: dep_template,
            preset_answers,
        });
    }

    Ok(())
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
    #[allow(clippy::type_complexity)]
    fn flatten_dependencies(
        &self,
        template: &TemplateVersionRes,
        visited: &mut HashSet<String>,
    ) -> Result<Vec<ResolvedDependency>, Box<dyn Error + Send>> {
        // Create a closure that captures self and calls the registry client
        let fetch = |id: String| self.registry_client.get_template_version_by_id(id);
        let mut flattened: Vec<ResolvedDependency> = Vec::new();
        flatten_impl(template, visited, &mut flattened, &fetch)?;
        Ok(flattened)
    }
}

impl DefaultDependencyResolver {
    /// Resolve dependencies with a custom template fetcher (for testing).
    /// This exposes the internal resolve_dependencies logic with injectable fetching.
    #[allow(clippy::type_complexity)]
    pub fn resolve_dependencies_with_fetcher(
        template: &TemplateVersionRes,
        fetch_template: Rc<dyn Fn(String) -> Result<TemplateVersionRes, Box<dyn Error + Send>>>,
    ) -> Result<Vec<ResolvedDependency>, Box<dyn Error + Send>> {
        let mut visited: HashSet<String> = HashSet::new();
        // Mark root as visited BEFORE traversal to prevent cycles back to root
        // from causing the root to be added twice (once from within recursion,
        // once as the final root append).
        visited.insert(template.principal.id.clone());
        let mut flattened: Vec<ResolvedDependency> = Vec::new();
        flatten_impl(template, &mut visited, &mut flattened, &*fetch_template)?;

        // Add root template at the end (post-order) with no preset answers
        flattened.push(ResolvedDependency {
            template: template.clone(),
            preset_answers: HashMap::new(),
        });

        Ok(flattened)
    }
}

impl DependencyResolver for DefaultDependencyResolver {
    fn resolve_dependencies(
        &self,
        template: &TemplateVersionRes,
    ) -> Result<Vec<ResolvedDependency>, Box<dyn Error + Send>> {
        let mut visited: HashSet<String> = HashSet::new();
        // Mark root as visited BEFORE traversal to prevent cycles back to root
        // from causing the root to be added twice (once from within recursion,
        // once as the final root append).
        visited.insert(template.principal.id.clone());
        let mut flattened = self.flatten_dependencies(template, &mut visited)?;

        // Add root template at the end (post-order) with no preset answers
        flattened.push(ResolvedDependency {
            template: template.clone(),
            preset_answers: HashMap::new(),
        });

        Ok(flattened)
    }
}
