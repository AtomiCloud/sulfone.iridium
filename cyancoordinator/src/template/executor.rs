use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;

use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::services::repo::{CyanHttpRepo, CyanRepo};
use cyanprompt::domain::services::template::engine::TemplateEngine;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanprompt::http::client::CyanClient;
use cyanprompt::http::mapper::cyan_req_mapper;
use cyanregistry::http::models::template_res::TemplateVersionRes;
use reqwest::blocking::Client;
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::client::CyanCoordinatorClient;
use crate::errors::GenericError;
use crate::models::req::{BuildReq, MergerReq, StartExecutorReq};

pub trait TemplateExecutor {
    /// Execute a template with the given parameters, returning archive data for the template
    /// Returns: (archive_data, template_state, actual_session_id)
    fn execute_template(
        &self,
        template: &TemplateVersionRes,
        session_id: &str,
        answers: Option<&HashMap<String, Answer>>,
        deterministic_states: Option<&HashMap<String, String>>,
    ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn Error + Send>>;
}

pub struct DefaultTemplateExecutor {
    pub coordinator_endpoint: String,
}

impl DefaultTemplateExecutor {
    pub fn new(coordinator_endpoint: String) -> Self {
        Self {
            coordinator_endpoint,
        }
    }

    fn new_template_engine(&self, template_endpoint: &str, client: Rc<Client>) -> TemplateEngine {
        let cyan_client = CyanClient {
            endpoint: template_endpoint.to_string(),
            client: client.clone(),
        };

        let repo: Rc<dyn CyanRepo> = Rc::new(CyanHttpRepo {
            client: cyan_client,
        });

        TemplateEngine { client: repo }
    }
}

impl TemplateExecutor for DefaultTemplateExecutor {
    fn execute_template(
        &self,
        template: &TemplateVersionRes,
        session_id: &str,
        answers: Option<&HashMap<String, Answer>>,
        deterministic_states: Option<&HashMap<String, String>>,
    ) -> Result<(Vec<u8>, TemplateState, String), Box<dyn Error + Send>> {
        // Create runtime
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();

        // Phase 1: Warm template and executor
        let (tx11, mut rx11) = mpsc::channel(1);
        let t11 = template.clone();
        let endpoint11 = self.coordinator_endpoint.clone();
        let h11 = runtime.spawn_blocking(move || {
            println!("♨️ Warming Template...");
            let client = CyanCoordinatorClient::new(endpoint11);
            let res = client.warm_template(&t11);
            println!("✅ Template Warmed");
            tx11.blocking_send(res)
        });

        let (tx12, mut rx12) = mpsc::channel(1);
        let t12 = template.clone();
        let s_id12 = session_id.to_string();
        let endpoint12 = self.coordinator_endpoint.clone();
        let h12 = runtime.spawn_blocking(move || {
            println!("♨️ Warming Processors and Plugins...");
            let client = CyanCoordinatorClient::new(endpoint12);
            let res = client.warn_executor(s_id12, &t12);
            println!("✅ Processors and Plugins Warmed");
            tx12.blocking_send(res)
        });

        let _ = runtime.block_on(h11).unwrap();
        let _ = runtime.block_on(h12).unwrap();

        let template_warm = rx11.blocking_recv().unwrap()?;
        let executor_warm = rx12.blocking_recv().unwrap()?;

        if template_warm.status.to_lowercase() != "ok" {
            return Err(Box::new(GenericError::ProblemDetails(
                crate::errors::ProblemDetails {
                    title: "Template Warm Error".to_string(),
                    status: 400,
                    t: "local".to_string(),
                    trace_id: None,
                    data: None,
                },
            )));
        }

        // Phase 2: Setup merger and executor
        let merger_id = Uuid::new_v4().to_string();
        let merger_req = MergerReq {
            merger_id: merger_id.clone(),
        };

        let start_executor_req = StartExecutorReq {
            session_id: executor_warm.session_id.clone(),
            template: template.clone(),
            write_vol_reference: executor_warm.vol_ref.clone(),
            merger: merger_req,
        };

        let (tx21, mut rx21) = mpsc::channel(1);
        let endpoint21 = self.coordinator_endpoint.clone();
        let start_req = start_executor_req.clone();
        let h21 = runtime.spawn_blocking(move || {
            println!("🚀 Bootstrapping Executor...");
            let client = CyanCoordinatorClient::new(endpoint21);
            let res = client.bootstrap(&start_req);
            tx21.blocking_send(res)
        });

        // Phase 3: Get template state
        let (tx22, mut rx22) = mpsc::channel(1);
        let coord_endpoint = self.coordinator_endpoint.clone();
        let template_id = template.principal.id.clone();

        // Setup Template Engine
        let template_endpoint = format!("{}/proxy/template/{}", coord_endpoint, template_id);
        let answers_clone = answers.cloned();
        let states_clone = deterministic_states.cloned();
        let self_clone = self.clone();

        let h22 = runtime.spawn_blocking(move || {
            if answers_clone.is_some() {
                println!("🤖 Using provided answers...");
            } else {
                println!("🤖 Starting interactive template Q&A...");
            }
            let c22 = Rc::new(Client::new());
            let prompter = self_clone.new_template_engine(template_endpoint.as_str(), c22.clone());
            let state = prompter.start_with(answers_clone, states_clone);
            println!("✅ Received all answers!");
            tx22.blocking_send(state)
        });

        let _ = runtime.block_on(h21).unwrap();
        let _ = runtime.block_on(h22).unwrap();

        let executor_started = rx21.blocking_recv().unwrap()?;
        if executor_started.status.to_lowercase() != "ok" {
            return Err(Box::new(GenericError::ProblemDetails(
                crate::errors::ProblemDetails {
                    title: "Executor Start Error".to_string(),
                    status: 400,
                    t: "local".to_string(),
                    trace_id: None,
                    data: None,
                },
            )));
        }
        let prompter_state: TemplateState = rx22.blocking_recv().unwrap();

        let res = match &prompter_state {
            TemplateState::QnA() => panic!("Should terminate in QnA state"),
            TemplateState::Complete(ref c, _) => {
                println!("✅ Cyan Response obtained");
                Ok(c.clone())
            }
            TemplateState::Err(ref e) => {
                println!("Error: {}", e);
                Err(Box::new(GenericError::ProblemDetails(
                    crate::errors::ProblemDetails {
                        title: "🚨 Template Prompting Error".to_string(),
                        status: 400,
                        t: "local".to_string(),
                        trace_id: None,
                        data: Some(serde_json::json!({
                            "error": e.to_string(),
                        })),
                    },
                )) as Box<dyn Error + Send>)
            }
        };
        let cyan = res?;

        // Final phase: Build
        let br = BuildReq {
            template: template.clone(),
            cyan: cyan_req_mapper(cyan),
            merger_id,
        };

        println!("🚀 Starting build...");

        // Get the archive data directly
        let host = self.coordinator_endpoint.clone();
        let endpoint = host + "/executor/" + executor_warm.session_id.as_str();
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        let response = http_client
            .post(endpoint)
            .json(&br)
            .send()
            .map_err(|x| Box::new(x) as Box<dyn Error + Send>)
            .and_then(|x| {
                if x.status().is_success() {
                    // Get the raw bytes
                    let bytes = x
                        .bytes()
                        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                    Ok(bytes.to_vec())
                } else {
                    let r: Result<crate::errors::ProblemDetails, Box<dyn Error + Send>> =
                        x.json().map_err(|e| Box::new(e) as Box<dyn Error + Send>);
                    match r {
                        Ok(ok) => {
                            Err(Box::new(GenericError::ProblemDetails(ok)) as Box<dyn Error + Send>)
                        }
                        Err(err) => Err(err),
                    }
                }
            })?;

        // Return the actual session ID used for this execution
        let actual_session_id = executor_warm.session_id.clone();

        Ok((response, prompter_state, actual_session_id))
    }
}

impl Clone for DefaultTemplateExecutor {
    fn clone(&self) -> Self {
        Self {
            coordinator_endpoint: self.coordinator_endpoint.clone(),
        }
    }
}
