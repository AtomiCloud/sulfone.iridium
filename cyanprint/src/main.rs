use std::error::Error;
use std::rc::Rc;

use bollard::Docker;
use clap::Parser;
use reqwest::blocking::Client;

use cyancoordinator::client::{new_client, CyanCoordinatorClient};
use cyanprompt::domain::services::repo::{CyanHttpRepo, CyanRepo};
use cyanprompt::domain::services::template::engine::TemplateEngine;
use cyanprompt::http::client::CyanClient;
use cyanregistry::http::client::CyanRegistryClient;

use crate::commands::{Cli, Commands, PushArgs, PushCommands};
use crate::coord::start_coordinator;
use crate::run::cyan_run;
use crate::util::{generate_session_id, parse_ref};

pub mod commands;

pub mod util;

pub mod errors;

pub mod run;

pub mod coord;

fn new_template_engine(endpoint: &str, client: Rc<Client>) -> TemplateEngine {
    let client: Rc<dyn CyanRepo> = Rc::new(CyanHttpRepo {
        client: CyanClient {
            endpoint: endpoint.to_string(),
            client,
        },
    });
    TemplateEngine { client }
}

fn main() -> Result<(), Box<dyn Error + Send>> {
    let http_client = new_client()?;
    let http = Rc::new(http_client);

    let cli = Cli::parse();
    let registry = CyanRegistryClient {
        endpoint: cli.registry.to_string(),
        version: "1.0".to_string(),
        client: Rc::clone(&http),
    };
    match cli.command {
        Commands::Push(push_arg) => match push_arg.commands {
            PushCommands::Processor { image, tag } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    ..
                } = push_arg;
                let res = registry.push_processor(config, token, message, image, tag);
                match res {
                    Ok(r) => {
                        println!("Pushed processor successfully");
                        println!("id: {}", r.id)
                    }
                    Err(e) => {
                        eprintln!("Error: {:#?}", e)
                    }
                }
                Ok(())
            }
            PushCommands::Template {
                template_image,
                template_tag,
                blob_image,
                blob_tag,
            } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    ..
                } = push_arg;
                let res = registry.push_template(
                    config,
                    token,
                    message,
                    blob_image,
                    blob_tag,
                    template_image,
                    template_tag,
                );
                match res {
                    Ok(r) => {
                        println!("Pushed template successfully");
                        println!("id: {}", r.id)
                    }
                    Err(e) => {
                        eprintln!("Error: {:#?}", e)
                    }
                }
                Ok(())
            }
            PushCommands::Plugin { image, tag } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    ..
                } = push_arg;
                let res = registry.push_plugin(config, token, message, image, tag);
                match res {
                    Ok(r) => {
                        println!("Pushed plugin successfully");
                        println!("id: {}", r.id)
                    }
                    Err(e) => {
                        eprintln!("Error: {:#?}", e)
                    }
                }
                Ok(())
            }
        },
        Commands::Create {
            template_ref,
            path,
            coordinator_endpoint,
        } => {
            let session_id = generate_session_id();
            let username = parse_ref(template_ref.clone())
                .map(|(u, _, _)| u)
                .unwrap_or_else(|_| "unknown".to_string());

            let r = parse_ref(template_ref)
                .and_then(|(u, n, v)| {
                    println!(
                        "🚘 Retrieving template '{}/{}:{}' from registry...",
                        u,
                        n,
                        v.unwrap_or(-1)
                    );
                    let r = registry.get_template(u.clone(), n.clone(), v);
                    println!(
                        "✅ Retrieved template '{}/{}:{}' from registry.",
                        u,
                        n,
                        v.unwrap_or(-1)
                    );
                    r
                })
                .and_then(|tv| {
                    cyan_run(
                        session_id.clone(),
                        path,
                        tv,
                        coordinator_endpoint.clone(),
                        username.clone(),
                    )
                });

            match r {
                Ok(o) => {
                    println!("✅ Completed: {:#?}", o);
                    let coord_client = CyanCoordinatorClient {
                        endpoint: coordinator_endpoint.clone(),
                    };
                    println!("🧹 Cleaning up...");
                    let _ = coord_client.clean(session_id);
                    println!("✅ Cleaned up");
                }
                Err(e) => {
                    eprintln!("🚨 Error: {:#?}", e);
                    let coord_client = CyanCoordinatorClient {
                        endpoint: coordinator_endpoint.clone(),
                    };
                    println!("🧹 Cleaning up...");
                    let _ = coord_client.clean(session_id);
                    println!("✅ Cleaned up");
                }
            }
            Ok(())
        }
        Commands::Daemon {
            version,
            architecture,
        } => {
            let docker = Docker::connect_with_local_defaults()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let arch = architecture
                        .unwrap_or(
                            if cfg!(target_arch = "arm") || cfg!(target_arch = "aarch64") {
                                "arm".to_string()
                            } else {
                                "amd".to_string()
                            },
                        )
                        .to_string();

                    let img = "ghcr.io/atomicloud/sulfone.boron/sulfone-boron".to_string()
                        + "-"
                        + arch.as_str()
                        + ":"
                        + version.as_str();
                    let r = start_coordinator(docker, img).await;
                    match r {
                        Ok(_) => {
                            println!("✅ Coordinator started");
                        }
                        Err(e) => {
                            eprintln!("🚨 Error: {:#?}", e);
                        }
                    }
                });

            Ok(())
        }
    }
}
