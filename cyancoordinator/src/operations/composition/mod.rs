pub mod layerer;
pub mod operator;
pub mod resolver;
pub mod state;
#[cfg(test)]
mod test;

// Re-export the main types for easier consumption
pub use layerer::{DefaultVfsLayerer, ResolverAwareLayerer, VfsLayerer};
pub use operator::CompositionOperator;
pub use resolver::{
    DefaultDependencyResolver, DependencyResolver, ResolvedDependency,
    flatten_dependencies_with_fetcher, serde_json_value_to_answer,
};
pub use state::CompositionState;
