use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use reqwest::blocking::Client;

use cyanregistry::http::models::template_res::TemplateVersionRes;

use crate::conflict_file_resolver::ResolverInput;
use crate::conflict_file_resolver::ResolverOutput;
use crate::errors::{GenericError, ProblemDetails};
use crate::models::req::{StartExecutorReq, TrySetupReq};
use crate::models::res::{CleanupRes, ExecutorWarmRes, StandardRes};
use crate::state::{DefaultStateManager, StateManager};

#[derive(Clone)]
pub struct CyanCoordinatorClient {
    pub endpoint: String,
    pub state_manager: Arc<dyn StateManager + Send + Sync>,
}

pub fn new_client() -> Result<Client, Box<dyn Error + Send>> {
    Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
}

impl CyanCoordinatorClient {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            state_manager: Arc::new(DefaultStateManager::new()),
        }
    }

    pub fn with_state_manager(
        endpoint: String,
        state_manager: Arc<dyn StateManager + Send + Sync>,
    ) -> Self {
        Self {
            endpoint,
            state_manager,
        }
    }

    pub fn clean(&self, session_id: String) -> Result<StandardRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/executor/" + session_id.as_str();
        let http_client = new_client()?;
        http_client
            .delete(endpoint)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    pub fn cleanup(&self) -> Result<CleanupRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/cleanup";
        let http_client = new_client()?;
        http_client
            .delete(endpoint)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    pub fn bootstrap(
        &self,
        start_executor_req: &StartExecutorReq,
    ) -> Result<StandardRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/executor";
        let http_client = new_client()?;
        http_client
            .post(endpoint)
            .json(start_executor_req)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }
    pub fn warn_executor(
        &self,
        session_id: String,
        template: &TemplateVersionRes,
    ) -> Result<ExecutorWarmRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/executor/" + session_id.as_str() + "/warm";
        let http_client = new_client()?;
        http_client
            .post(endpoint)
            .json(template)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }
    pub fn warm_template(
        &self,
        template: &TemplateVersionRes,
    ) -> Result<StandardRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/template/warm";
        let http_client = new_client()?;
        http_client
            .post(endpoint)
            .json(template)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    /// Resolve files using a resolver service
    ///
    /// Calls POST /proxy/resolver/{cyan_id}/api/resolve with the resolver input
    pub fn resolve_files(
        &self,
        cyan_id: &str,
        input: &ResolverInput,
    ) -> Result<ResolverOutput, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        // Sanitize cyan_id to prevent path injection
        let sanitized_cyan_id: String = cyan_id
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let endpoint = host + "/proxy/resolver/" + &sanitized_cyan_id + "/api/resolve";
        let http_client = new_client()?;
        http_client
            .post(endpoint)
            .json(input)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    /// Setup a try executor session
    ///
    /// Calls POST /executor/try to create a new try session
    pub fn try_setup(
        &self,
        req: &TrySetupReq,
    ) -> Result<crate::models::res::TrySetupRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/executor/try";
        let http_client = new_client()?;
        http_client
            .post(endpoint)
            .json(req)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }

    /// Cleanup a try executor session
    ///
    /// Calls DELETE /executor/{session_id} to clean up a try session
    pub fn try_cleanup(&self, session_id: &str) -> Result<StandardRes, Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/executor/" + session_id;
        let http_client = new_client()?;
        http_client
            .delete(endpoint)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>)
                } else {
                    let r: Result<ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })
    }
}
