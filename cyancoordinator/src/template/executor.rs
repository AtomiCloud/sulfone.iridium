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
    /// When true, the Q&A phase runs non-interactively via the headless driver
    /// ([`TemplateEngine::start_headless`]): a question with no supplied answer
    /// yields a [`TemplateState::NeedInput`] terminal state (and the build phase
    /// is skipped) instead of prompting.
    pub headless: bool,
}

impl DefaultTemplateExecutor {
    pub fn new(coordinator_endpoint: String) -> Self {
        Self {
            coordinator_endpoint,
            headless: false,
        }
    }

    /// Construct an executor whose Q&A phase runs in headless mode when
    /// `headless` is true (see [`DefaultTemplateExecutor::headless`]).
    pub fn new_with_headless(coordinator_endpoint: String, headless: bool) -> Self {
        Self {
            coordinator_endpoint,
            headless,
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
        let headless = self.headless;
        let (tx11, mut rx11) = mpsc::channel(1);
        let t11 = template.clone();
        let endpoint11 = self.coordinator_endpoint.clone();
        let h11 = runtime.spawn_blocking(move || {
            cprogress!(headless, "♨️ Warming Template...");
            let client = CyanCoordinatorClient::new(endpoint11);
            let res = client.warm_template(&t11);
            cprogress!(headless, "✅ Template Warmed");
            tx11.blocking_send(res)
        });

        let (tx12, mut rx12) = mpsc::channel(1);
        let t12 = template.clone();
        let s_id12 = session_id.to_string();
        let endpoint12 = self.coordinator_endpoint.clone();
        let h12 = runtime.spawn_blocking(move || {
            cprogress!(headless, "♨️ Warming Processors and Plugins...");
            let client = CyanCoordinatorClient::new(endpoint12);
            let res = client.warn_executor(s_id12, &t12);
            cprogress!(headless, "✅ Processors and Plugins Warmed");
            tx12.blocking_send(res)
        });

        let _ = runtime.block_on(h11).unwrap();
        let _ = runtime.block_on(h12).unwrap();

        // Read the executor warm result FIRST. `warm_template` and `warn_executor` run
        // concurrently, so one can succeed (creating an executor session) while the other
        // fails. Reading the executor result before the template result means that when
        // `template_warm` errors below, the executor session id is already available to
        // `clean_warmed_session` — otherwise the `?` on the template result would
        // propagate before cleanup runs, leaking the already-created executor session.
        let executor_warm = rx12.blocking_recv().unwrap()?;

        // The warmed executor session was created during Phase 1 (warm/bootstrap). The
        // success and NeedInput paths return `actual_session_id` so the CLI can clean it
        // fire-and-forget. Any later error path (concurrent warm failure, warm-status,
        // bootstrap, Q&A, or build failure) returns `Err` WITHOUT surfacing the session
        // id, so the warmed session would be left dangling (only the 12-hour coordinator
        // auto-cleanup would reclaim it). Clean it best-effort right here on those error
        // paths so a failed run does not leak a coordinator session/volume. Cleanup
        // failures are ignored — they must not mask the real error.
        let clean_warmed_session = || {
            let client = CyanCoordinatorClient::new(self.coordinator_endpoint.clone());
            let _ = client.clean(executor_warm.session_id.clone());
        };

        // `template_warm` failing is the concurrent-warm partial-failure path: the
        // executor warm above already created a session. Clean it before propagating the
        // template-warm error so the half-created session does not leak. (Reading
        // `executor_warm` first guarantees the id is available here.)
        let template_warm = match rx11.blocking_recv().unwrap() {
            Ok(w) => w,
            Err(e) => {
                clean_warmed_session();
                return Err(e);
            }
        };

        if template_warm.status.to_lowercase() != "ok" {
            clean_warmed_session();
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
            cprogress!(headless, "🚀 Bootstrapping Executor...");
            let client = CyanCoordinatorClient::new(endpoint21);
            let res = client.bootstrap(&start_req);
            tx21.blocking_send(res)
        });

        // Phase 3: Get template state
        let (tx22, mut rx22) = mpsc::channel(1);
        let coord_endpoint = self.coordinator_endpoint.clone();
        let template_id = template.principal.id.clone();

        // Setup Template Engine
        let template_endpoint = format!("{coord_endpoint}/proxy/template/{template_id}");
        let answers_clone = answers.cloned();
        let states_clone = deterministic_states.cloned();
        let self_clone = self.clone();

        let h22 = runtime.spawn_blocking(move || {
            if headless {
                // Headless mode prints nothing to stdout here — the single JSON
                // envelope is the only contract output; progress goes to stderr.
                eprintln!("🤖 Running headless template Q&A...");
            } else if answers_clone.is_some() {
                println!("🤖 Using provided answers...");
            } else {
                println!("🤖 Starting interactive template Q&A...");
            }
            let c22 = Rc::new(Client::new());
            let prompter = self_clone.new_template_engine(template_endpoint.as_str(), c22.clone());
            let state = if headless {
                // Headless re-derives deterministic state internally; the caller
                // supplies only answers.
                prompter.start_headless(answers_clone)
            } else {
                prompter.start_with(answers_clone, states_clone)
            };
            if !headless {
                println!("✅ Received all answers!");
            }
            tx22.blocking_send(state)
        });

        let _ = runtime.block_on(h21).unwrap();
        let _ = runtime.block_on(h22).unwrap();

        let executor_started = rx21.blocking_recv().unwrap();
        let executor_started = match executor_started {
            Ok(s) => s,
            Err(e) => {
                // Bootstrap failed after a successful warm — clean the leaked session.
                clean_warmed_session();
                return Err(e);
            }
        };
        if executor_started.status.to_lowercase() != "ok" {
            clean_warmed_session();
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

        // Headless: a NeedInput is a terminal, NON-error outcome. Surface it to the
        // caller without producing an archive — the build phase is skipped because
        // there is no finalized Cyan yet (the caller will emit the question and stop).
        if let TemplateState::NeedInput(_, _) = &prompter_state {
            let actual_session_id = executor_warm.session_id.clone();
            return Ok((Vec::new(), prompter_state, actual_session_id));
        }

        let res = match &prompter_state {
            TemplateState::QnA() => panic!("Should terminate in QnA state"),
            TemplateState::NeedInput(_, _) => {
                unreachable!("NeedInput is handled by the early return above")
            }
            TemplateState::Complete(ref c, _) => {
                cprogress!(headless, "✅ Cyan Response obtained");
                Ok(c.clone())
            }
            TemplateState::Err(ref e) => {
                cprogress!(headless, "Error: {e}");
                // Q&A failed after warm/bootstrap — clean the leaked session before
                // surfacing the error.
                clean_warmed_session();
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

        cprogress!(headless, "🚀 Starting build...");

        // Get the archive data directly
        let host = self.coordinator_endpoint.clone();
        let endpoint = host + "/executor/" + executor_warm.session_id.as_str();
        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| {
                clean_warmed_session();
                Box::new(e) as Box<dyn Error + Send>
            })?;

        let response = http_client
            .post(endpoint)
            .json(&br)
            .send()
            .map_err(|x| {
                clean_warmed_session();
                Box::new(x) as Box<dyn Error + Send>
            })
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
            });

        // A build-phase failure (non-success build response or body-read error) returns
        // `Err` without surfacing the session id — clean the warmed session before
        // propagating so the run does not leak a coordinator session/volume.
        let response = match response {
            Ok(bytes) => bytes,
            Err(e) => {
                clean_warmed_session();
                return Err(e);
            }
        };

        // Return the actual session ID used for this execution
        let actual_session_id = executor_warm.session_id.clone();

        Ok((response, prompter_state, actual_session_id))
    }
}

impl Clone for DefaultTemplateExecutor {
    fn clone(&self) -> Self {
        Self {
            coordinator_endpoint: self.coordinator_endpoint.clone(),
            headless: self.headless,
        }
    }
}
