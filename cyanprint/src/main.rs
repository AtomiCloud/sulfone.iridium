use std::error::Error;
use std::rc::Rc;

use bollard::Docker;
use clap::Parser;

use cyancoordinator::client::{CyanCoordinatorClient, new_client};
use cyancoordinator::session::DefaultSessionIdGenerator;
use cyanregistry::http::client::CyanRegistryClient;

use crate::commands::{Cli, Commands, DaemonCommands, PushArgs, PushCommands};
use crate::coord::{start_coordinator, stop_coordinator};
use crate::run::cyan_run;
use crate::update::cyan_update;
use crate::util::parse_ref;

pub mod commands;
pub mod coord;
pub mod errors;
pub mod run;
pub mod update;
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
                        println!("id: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error: {e:#?}");
                        Err(e)
                    }
                }
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
                        println!("✅ Pushed template successfully");
                        println!("📦 Template ID: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {e:#?}");
                        Err(e)
                    }
                }
            }
            PushCommands::Group => {
                let PushArgs {
                    config,
                    token,
                    message,
                    ..
                } = push_arg;
                println!("🔗 Pushing template group (no Docker artifacts)...");
                let res = registry.push_template_without_properties(config, token, message);
                match res {
                    Ok(r) => {
                        println!("✅ Pushed template group successfully");
                        println!("📦 Template ID: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("❌ Error pushing template group: {e:#?}");
                        Err(e)
                    }
                }
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
                        println!("id: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error: {e:#?}");
                        Err(e)
                    }
                }
            }
            PushCommands::Resolver { image, tag } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    ..
                } = push_arg;
                let res = registry.push_resolver(config, token, message, image, tag);
                match res {
                    Ok(r) => {
                        println!("✅ Pushed resolver successfully");
                        println!("📦 Resolver ID: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("❌ Error pushing resolver: {e:#?}");
                        Err(e)
                    }
                }
            }
        },
        Commands::Create {
            template_ref,
            path,
            coordinator_endpoint,
        } => {
            let session_id_generator = Box::new(DefaultSessionIdGenerator);

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
                        session_id_generator,
                        path,
                        tv,
                        coord_client,
                        username.clone(),
                        Rc::clone(&registry_ref),
                        cli.debug,
                    )
                });

            match r {
                Ok(session_ids) => {
                    println!("✅ Completed successfully");
                    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());
                    println!("🧹 Cleaning up all sessions...");
                    for sid in session_ids {
                        println!("🧹 Cleaning up session: {sid}");
                        let _ = coord_client.clean(sid);
                    }
                    println!("✅ Cleaned up all sessions");
                }
                Err(e) => {
                    eprintln!("🚨 Error: {e:#?}");
                    println!("✅ No sessions to clean up");
                }
            }
            Ok(())
        }
        Commands::Update {
            path,
            coordinator_endpoint,
            interactive,
        } => {
            let session_id_generator = Box::new(DefaultSessionIdGenerator);
            let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());
            let registry_ref = Rc::new(registry);

            println!("🔄 Updating templates to latest versions");

            let r = cyan_update(
                session_id_generator,
                path,
                coord_client.clone(),
                Rc::clone(&registry_ref),
                cli.debug,
                interactive,
            );

            match r {
                Ok(session_ids) => {
                    println!("✅ Update completed successfully");
                    println!("🧹 Cleaning up all sessions...");
                    for sid in session_ids {
                        println!("🧹 Cleaning up session: {sid}");
                        let _ = coord_client.clean(sid);
                    }
                    println!("✅ Cleaned up all sessions");
                }
                Err(e) => {
                    eprintln!("🚨 Error during update: {e:#?}");
                }
            }
            Ok(())
        }
        Commands::Daemon { command } => {
            let docker = Docker::connect_with_local_defaults()
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            let result = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    match command {
                        DaemonCommands::Start {
                            version,
                            port,
                            registry,
                        } => {
                            let img = "ghcr.io/atomicloud/sulfone.boron/sulfone-boron".to_string()
                                + ":"
                                + version.as_str();
                            start_coordinator(docker, img, port, registry)
                                .await
                                .map(|_| {
                                    println!("✅ Coordinator started on port {port}");
                                })
                        }
                        DaemonCommands::Stop { port } => {
                            stop_coordinator(docker, port).await.map(|_| {
                                println!("✅ Coordinator stopped");
                            })
                        }
                    }
                });

            result
        }
    }
}
