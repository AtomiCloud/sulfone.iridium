use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use chrono::Utc;
use reqwest::blocking::Client;
use tokio::runtime::Builder;
use tokio::sync::mpsc;
use uuid::Uuid;

use cyancoordinator::client::CyanCoordinatorClient;
use cyancoordinator::errors::{GenericError, ProblemDetails};
use cyancoordinator::models::req::{BuildReq, MergerReq, StartExecutorReq};
use cyanprompt::domain::models::answer::Answer;
use cyanprompt::domain::services::template::states::TemplateState;
use cyanprompt::http::mapper::cyan_req_mapper;
use cyanregistry::http::models::template_res::TemplateVersionRes;

use crate::new_template_engine;
use crate::util::{
    load_or_create_state_file, save_state_file, TemplateHistoryEntry,
    TemplateState as YamlTemplateState,
};

pub fn cyan_run(
    session_id: String,
    path: Option<String>,
    template: TemplateVersionRes,
    coordinator_endpoint: String,
    username: String,
) -> Result<(), Box<dyn Error + Send>> {
    // handle the target directory
    let path = path.unwrap_or(".".to_string());
    let path_buf = PathBuf::from(&path);
    let p = path_buf.as_path();
    println!("📁 Generating target directory: {:?}", p);
    fs::create_dir_all(p).map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // runtime
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();

    // PHASE 1
    let (tx11, mut rx11) = mpsc::channel(1);
    let t11 = template.clone();
    let c11 = CyanCoordinatorClient {
        endpoint: coordinator_endpoint.clone(),
    };
    let h11 = runtime.spawn_blocking(move || {
        println!("♨️ Warming Templates...");
        let res = c11.warm_template(&t11);
        println!("✅ Template Warmed");
        tx11.blocking_send(res)
    });

    let (tx12, mut rx12) = mpsc::channel(1);
    let t12 = template.clone();
    let c12 = CyanCoordinatorClient {
        endpoint: coordinator_endpoint.clone(),
    };
    let h12 = runtime.spawn_blocking(move || {
        println!("♨️ Warming Processors and Plugins...");
        let res = c12.warn_executor(session_id, &t12);
        println!("✅ Processors and Plugins Warmed");
        tx12.blocking_send(res)
    });

    let _ = runtime.block_on(h11).unwrap();
    let _ = runtime.block_on(h12).unwrap();

    let template_warm = rx11.blocking_recv().unwrap()?;
    let executor_warm = rx12.blocking_recv().unwrap()?;

    if template_warm.status.to_lowercase() != "ok" {
        return Err(Box::new(GenericError::ProblemDetails(ProblemDetails {
            title: "Template Warm Error".to_string(),
            status: 400,
            t: "local".to_string(),
            trace_id: None,
            data: None,
        })));
    }

    // phase 2
    let merger_id = Uuid::new_v4().to_string();
    let session_id = executor_warm.session_id.clone();

    println!("📖 Session: {}, Merger: {}", session_id, merger_id);

    let (tx21, mut rx21) = mpsc::channel(1);

    let merger_req = MergerReq {
        merger_id: merger_id.clone(),
    };

    let start_executor_req = StartExecutorReq {
        session_id: session_id.clone(),
        template: template.clone(),
        write_vol_reference: executor_warm.vol_ref.clone(),
        merger: merger_req,
    };
    let c21 = CyanCoordinatorClient {
        endpoint: coordinator_endpoint.clone(),
    };
    let h21 = runtime.spawn_blocking(move || {
        println!("🚀 Bootstrapping Executor...");
        let res = c21.bootstrap(&start_executor_req);
        tx21.blocking_send(res)
    });

    let (tx22, mut rx22) = mpsc::channel(1);
    let coord_endpoint = coordinator_endpoint.clone();
    let template_id = template.principal.id.clone();
    let h22 = runtime.spawn_blocking(move || {
        println!("🤖 Starting template...");
        let c22 = Rc::new(Client::new());
        let endpoint = format!("{}/proxy/template/{}", coord_endpoint, template_id);
        let prompter = new_template_engine(endpoint.as_str(), c22.clone());
        let state = prompter.start();
        println!("✅ Received all answers!");
        tx22.blocking_send(state)
    });

    let _ = runtime.block_on(h21).unwrap();
    let _ = runtime.block_on(h22).unwrap();

    let executor_started = rx21.blocking_recv().unwrap()?;
    if executor_started.status.to_lowercase() != "ok" {
        return Err(Box::new(GenericError::ProblemDetails(ProblemDetails {
            title: "Executor Start Error".to_string(),
            status: 400,
            t: "local".to_string(),
            trace_id: None,
            data: None,
        })));
    }
    let prompter_state: TemplateState = rx22.blocking_recv().unwrap();

    let res = match prompter_state {
        TemplateState::QnA() => panic!("Should terminate in QnA state"),
        TemplateState::Complete(ref c, _) => {
            println!("✅ Cyan Response obtained");
            Ok(c.clone())
        }
        TemplateState::Err(ref e) => {
            println!("Error: {}", e);
            Err(Box::new(GenericError::ProblemDetails(ProblemDetails {
                title: "🚨 Template Prompting Error".to_string(),
                status: 400,
                t: "local".to_string(),
                trace_id: None,
                data: Some(serde_json::json!({
                    "error": e.to_string(),
                })),
            })) as Box<dyn Error + Send>)
        }
    };
    let cyan = res?;

    // final phase
    let coord_client = CyanCoordinatorClient {
        endpoint: coordinator_endpoint.clone(),
    };
    let br = BuildReq {
        template: template.clone(),
        cyan: cyan_req_mapper(cyan),
        merger_id,
    };
    println!("🚀 Starting build...");
    coord_client.start(path_buf.as_path(), session_id.clone(), &br)?;
    println!("✅ Build completed");

    if let TemplateState::Complete(_, answers) = &prompter_state {
        save_template_metadata(
            path_buf.as_path(),
            &template,
            answers,
            &prompter_state,
            &username,
        )?;
        println!("📝 Template metadata saved to .cyan_state.yaml");
    }

    Ok(())
}

fn save_template_metadata(
    target_dir: &Path,
    template: &TemplateVersionRes,
    answers: &HashMap<String, Answer>,
    _template_state: &TemplateState, // Unused but kept for future extension
    username: &str,
) -> Result<(), Box<dyn Error + Send>> {
    let state_file_path = target_dir.join(".cyan_state.yaml");

    let mut state = load_or_create_state_file(&state_file_path)?;

    let template_key = format!("{}/{}", username, template.template.name);

    let deterministic_states = HashMap::new();

    let history_entry = TemplateHistoryEntry {
        version: template.principal.version,
        time: Utc::now(),
        answers: answers.clone(),
        deterministic_states,
    };

    let template_state_entry = state
        .templates
        .entry(template_key)
        .or_insert(YamlTemplateState {
            active: true,
            history: Vec::new(),
        });

    template_state_entry.history.push(history_entry);

    save_state_file(&state, &state_file_path)?;

    Ok(())
}
