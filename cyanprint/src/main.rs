use std::error::Error;
use std::rc::Rc;

use clap::Parser;
use reqwest::blocking::Client;

use cyancoordinator::client::{CyanCoordinatorClient, new_client};
use cyanprompt::domain::services::extension::engine::ExtensionEngine;
use cyanprompt::domain::services::repo::{CyanHttpRepo, CyanRepo};
use cyanprompt::domain::services::template::engine::TemplateEngine;
use cyanprompt::http::client::CyanClient;
use cyanregistry::http::client::CyanRegistryClient;

use crate::commands::{Cli, Commands, PushCommands};
use crate::run::cyan_run;
use crate::util::{generate_session_id, parse_ref};

pub mod commands;

pub mod util;

pub mod errors;

pub mod run;

fn new_template_engine(endpoint: &str, client: Rc<Client>) -> TemplateEngine {
    let client: Rc<dyn CyanRepo> = Rc::new(CyanHttpRepo {
        client: CyanClient {
            endpoint: endpoint.to_string(),
            client,
        },
    });
    TemplateEngine { client }
}

fn new_extension_engine(endpoint: &str, client: Rc<Client>) -> ExtensionEngine {
    let client: Rc<dyn CyanRepo> = Rc::new(CyanHttpRepo {
        client: CyanClient {
            endpoint: endpoint.to_string(),
            client,
        },
    });
    ExtensionEngine { client }
}

fn main() -> Result<(), Box<dyn Error + Send>> {
    let http_client = new_client()?;
    let http = Rc::new(http_client);

    let registry_endpoint = "https://api.zinc.sulfone.raichu.cluster.atomi.cloud";


    let registry = CyanRegistryClient {
        endpoint: registry_endpoint.to_string(),
        version: "1.0".to_string(),
        client: Rc::clone(&http),
    };

    let cli = Cli::parse();

    match cli.command {
        Commands::Push(p) => {
            match p.commands {
                PushCommands::Processor { config, token, message, image, sha } => {
                    let res = registry
                        .push_processor(config, token, message, image, sha);
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
                PushCommands::Template { config, token, message, template_image, template_sha, blob_image, blob_sha } => {
                    let res = registry
                        .push_template(config, token, message, blob_image, blob_sha, template_image, template_sha);
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
                PushCommands::Plugin { config, token, message, image, sha } => {
                    let res = registry
                        .push_plugin(config, token, message, image, sha);
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
            }
        }
        Commands::Run { template_ref, path, coordinator_endpoint } => {
            let session_id = generate_session_id();
            let r = parse_ref(template_ref)
                .and_then(|(u, n, v)| {
                    println!("🚘 Retrieving template '{}/{}:{}' from registry...", u, n, v.unwrap_or(-1));
                    let r = registry.get_template(u.clone(), n.clone(), v);
                    println!("✅ Retrieved template '{}/{}:{}' from registry.", u, n, v.unwrap_or(-1));
                    r
                })
                .and_then(|tv| cyan_run(session_id.clone(), path, tv, coordinator_endpoint.clone()));

            match r {
                Ok(o) => {
                    println!("✅ Completed: {:#?}", o);
                    let coord_client = CyanCoordinatorClient { endpoint: coordinator_endpoint.clone() };
                    println!("🧹 Cleaning up...");
                    let _ = coord_client.clean(session_id);
                    println!("✅ Cleaned up");
                }
                Err(e) => {
                    eprintln!("🚨 Error: {:#?}", e);
                    let coord_client = CyanCoordinatorClient { endpoint: coordinator_endpoint.clone() };
                    println!("🧹 Cleaning up...");
                    let _ = coord_client.clean(session_id);
                    println!("✅ Cleaned up");
                    let _ = new_extension_engine(registry_endpoint, Rc::clone(&http));
                }
            }
            Ok(())
        }
    }
}
