pub mod layerer;
pub mod operator;
pub mod resolver;
pub mod state;
#[cfg(test)]
mod test;

// Re-export the main types for easier consumption
pub use layerer::{DefaultVfsLayerer, VfsLayerer};
pub use operator::{CompositionOperator, TemplateVfsCollection};
pub use resolver::{DefaultDependencyResolver, DependencyResolver};
pub use state::CompositionState;
