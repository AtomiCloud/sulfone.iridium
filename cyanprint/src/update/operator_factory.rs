use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::operations::composition::{CompositionOperator, DefaultDependencyResolver};
use cyancoordinator::template::{DefaultTemplateExecutor, DefaultTemplateHistory};
use cyancoordinator::{fs::DefaultVfs, session::SessionIdGenerator};
use cyanregistry::http::client::CyanRegistryClient;

/// Whether the merger's debug output should be enabled.
///
/// The merger emits its debug output via plain `println!`, which would pollute the
/// single-JSON-on-stdout headless contract. So debug is enabled only when `--debug`
/// is set AND we are NOT in headless mode. This mirrors the create path (`run.rs`),
/// which gates the same merger the same way; the `update` path builds its merger here.
fn merger_debug_enabled(debug: bool, headless: bool) -> bool {
    debug && !headless
}

/// Factory for creating composition operators with all required dependencies
pub struct OperatorFactory;

impl OperatorFactory {
    /// Create a composition operator with the given dependencies (handles both single templates and compositions)
    pub fn create_composition_operator(
        session_id_generator: Box<dyn SessionIdGenerator>,
        coord_client: CyanCoordinatorClient,
        registry_client: Rc<CyanRegistryClient>,
        debug: bool,
        cache_config: cyancoordinator::cache::CacheConfig,
        headless: bool,
    ) -> CompositionOperator {
        let unpacker = Box::new(TarGzUnpacker);
        let loader = Box::new(DiskFileLoader);
        // Disable merger debug under headless so its `println!` debug output never
        // pollutes the single-JSON-on-stdout contract; interactive `--debug` is
        // unchanged. See [`merger_debug_enabled`].
        let merger = Box::new(GitLikeMerger::new(
            merger_debug_enabled(debug, headless),
            50,
        ));
        let writer = Box::new(DiskFileWriter);

        let template_history = Box::new(DefaultTemplateHistory::new());
        let template_executor = Box::new(DefaultTemplateExecutor::new_with_headless(
            coord_client.endpoint.clone(),
            headless,
        ));
        let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));

        let template_operator = TemplateOperator::new(
            session_id_generator,
            template_executor,
            template_history,
            vfs,
            registry_client.clone(),
        );

        let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client.clone()));

        // Use with_client to enable resolver-aware layering
        let mut operator =
            CompositionOperator::with_client(template_operator, dependency_resolver, coord_client);
        // Inject the per-node execution cache (honors --no-output-cache / --cache-dir / env).
        operator.set_cache(cyancoordinator::cache::Cache::new(cache_config));
        operator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The `update` path builds its merger through this factory. The merger's debug
    // output is plain `println!`, so under headless it MUST be suppressed to keep the
    // single-JSON-on-stdout contract; on the interactive path `--debug` is honored.
    // The regression that leaked before was the factory passing `debug` through
    // unconditionally — this asserts the exact gate the construction at
    // `create_composition_operator` uses (`GitLikeMerger::new(merger_debug_enabled(...))`),
    // deterministically and without depending on process-global stdout (an fd-capture
    // approach is racy under cargo's parallel harness, which writes test-progress lines
    // to the real fd 1).
    #[test]
    fn merger_debug_is_suppressed_under_headless() {
        // Headless suppresses merger debug regardless of `--debug`.
        assert!(!merger_debug_enabled(true, true), "headless + debug → off");
        assert!(
            !merger_debug_enabled(false, true),
            "headless + no debug → off"
        );
    }

    #[test]
    fn merger_debug_follows_debug_flag_when_interactive() {
        // Interactive: the merger debug exactly follows `--debug`.
        assert!(
            merger_debug_enabled(true, false),
            "interactive + debug → on"
        );
        assert!(
            !merger_debug_enabled(false, false),
            "interactive + no debug → off"
        );
    }
}
