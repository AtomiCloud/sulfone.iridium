use std::error::Error;
use std::rc::Rc;

use bollard::Docker;
use clap::Parser;

use cyancoordinator::client::{new_client, CyanCoordinatorClient};
use cyancoordinator::session::DefaultSessionIdGenerator;
use cyanregistry::http::client::CyanRegistryClient;

use crate::commands::{Cli, Commands, PushArgs, PushCommands};
use crate::coord::start_coordinator;
use crate::run::cyan_run;
use crate::util::parse_ref;

pub mod commands;
pub mod coord;
pub mod errors;
pub mod run;
pub mod util;

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
            let session_id_generator = DefaultSessionIdGenerator;

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
                    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());
                    let registry_ref = Rc::new(registry);

                    cyan_run(
                        &session_id_generator,
                        path,
                        tv,
                        coord_client,
                        username.clone(),
                        Some(Rc::clone(&registry_ref)),
                    )
                });

            match r {
                Ok(session_ids) => {
                    println!("✅ Completed successfully");
                    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());
                    println!("🧹 Cleaning up all sessions...");
                    for sid in session_ids {
                        println!("🧹 Cleaning up session: {}", sid);
                        let _ = coord_client.clean(sid);
                    }
                    println!("✅ Cleaned up all sessions");
                }
                Err(e) => {
                    eprintln!("🚨 Error: {:#?}", e);
                    println!("✅ No sessions to clean up");
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
