use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use cyanprompt::domain::services::template::states::TemplateState;
use reqwest::blocking::Client;

use cyanregistry::http::models::template_res::TemplateVersionRes;

use crate::errors::{GenericError, ProblemDetails};
use crate::fs::FileSystemWriter;
use crate::models::req::{BuildReq, StartExecutorReq};
use crate::models::res::{ExecutorWarmRes, StandardRes};
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

    pub fn start(
        &self,
        full_dir: &Path,
        session_id: String,
        build_req: &BuildReq,
        template_state: Option<(&TemplateState, &str)>,
    ) -> Result<(), Box<dyn Error + Send>> {
        let host = (self.endpoint).to_string().to_owned();
        let endpoint = host + "/executor/" + session_id.as_str();
        let http_client = new_client()?;
        let response = http_client
            .post(endpoint)
            .json(build_req)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    Ok(x)
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
            })?;

        std::fs::create_dir_all(full_dir).map_err(|x| {
            Box::new(GenericError::ProblemDetails(ProblemDetails {
                title: "Local Error, unable to create directory".to_string(),
                status: 400,
                t: "local".to_string(),
                trace_id: None,
                data: Some(serde_json::json!({
                    "error": x.to_string(),
                })),
            })) as Box<dyn Error + Send>
        })?;

        // Use the FileSystemWriter to process the archive
        let fs_writer = FileSystemWriter::default();
        fs_writer.process(response, full_dir).map_err(|x| {
            Box::new(GenericError::ProblemDetails(ProblemDetails {
                title: "Failed to process archive".to_string(),
                status: 400,
                t: "local".to_string(),
                trace_id: None,
                data: Some(serde_json::json!({
                    "error": x.to_string(),
                })),
            })) as Box<dyn Error + Send>
        })?;

        // If template state is provided, save metadata directly
        if let Some((state, username)) = template_state {
            if let TemplateState::Complete(_, answers) = state {
                // Directly call state_manager methods instead of using a separate method
                self.state_manager.save_template_metadata(
                    full_dir,
                    &build_req.template,
                    answers,
                    state,
                    username,
                )?;
                println!("📝 Template metadata saved to .cyan_state.yaml");
            }
        }

        Ok(())
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
}
