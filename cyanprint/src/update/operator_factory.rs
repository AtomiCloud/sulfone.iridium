use std::rc::Rc;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::fs::{DiskFileLoader, DiskFileWriter, GitLikeMerger, TarGzUnpacker};
use cyancoordinator::operations::TemplateOperator;
use cyancoordinator::operations::composition::{
    CompositionOperator, DefaultDependencyResolver, DefaultVfsLayerer,
};
use cyancoordinator::template::{DefaultTemplateExecutor, DefaultTemplateHistory};
use cyancoordinator::{fs::DefaultVfs, session::SessionIdGenerator};
use cyanregistry::http::client::CyanRegistryClient;

/// Factory for creating composition operators with all required dependencies
pub struct OperatorFactory;

impl OperatorFactory {
    /// Create a composition operator with the given dependencies (handles both single templates and compositions)
    pub fn create_composition_operator(
        session_id_generator: Box<dyn SessionIdGenerator>,
        coord_client: CyanCoordinatorClient,
        registry_client: Rc<CyanRegistryClient>,
        debug: bool,
    ) -> CompositionOperator {
        let unpacker = Box::new(TarGzUnpacker);
        let loader = Box::new(DiskFileLoader);
        let merger = Box::new(GitLikeMerger::new(debug, 50));
        let writer = Box::new(DiskFileWriter);

        let template_history = Box::new(DefaultTemplateHistory::new());
        let template_executor =
            Box::new(DefaultTemplateExecutor::new(coord_client.endpoint.clone()));
        let vfs = Box::new(DefaultVfs::new(unpacker, loader, merger, writer));

        let template_operator = TemplateOperator::new(
            session_id_generator,
            template_executor,
            template_history,
            vfs,
            registry_client.clone(),
        );

        let dependency_resolver = Box::new(DefaultDependencyResolver::new(registry_client.clone()));
        let vfs_layerer = Box::new(DefaultVfsLayerer);

        CompositionOperator::new(template_operator, dependency_resolver, vfs_layerer)
    }
}
