use std::collections::HashMap;
use std::error::Error;

use bollard::Docker;
use bollard::models::{
    ContainerCreateBody, ContainerSummaryStateEnum, HostConfig, Mount, MountTypeEnum, PortBinding,
};
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, StartContainerOptions,
};
use futures_util::stream::StreamExt;
use futures_util::stream::TryStreamExt;

pub async fn stop_coordinator(docker: Docker, port: u16) -> Result<(), Box<dyn Error + Send>> {
    let coord_filter = "^cyanprint-coordinator$";

    // 1. Call DELETE /cleanup on the Boron container
    println!("🧹 Calling cleanup endpoint on coordinator...");
    let client = crate::CyanCoordinatorClient::new(format!("http://localhost:{port}"));
    // Note: client.cleanup() uses blocking HTTP client, so we wrap it in spawn_blocking
    // to avoid blocking the tokio runtime
    let cleanup_result = tokio::task::spawn_blocking(move || client.cleanup()).await;
    match cleanup_result {
        Ok(Ok(res)) => {
            // Check for error/non-ok status before claiming success
            let has_error = res.error.as_deref().map(|e| !e.is_empty()).unwrap_or(false);
            let is_ok_status = res
                .status
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("ok"));
            if has_error || !is_ok_status {
                eprintln!(
                    "⚠️ Cleanup returned non-ok status: {:?}, error: {:?}",
                    res.status, res.error
                );
            } else {
                println!("✅ Cleanup completed");
            }
            if let Some(ref containers) = res.containers_removed {
                if !containers.is_empty() {
                    println!("   Removed containers: {containers:?}");
                }
            }
            if let Some(ref images) = res.images_removed {
                if !images.is_empty() {
                    println!("   Removed images: {images:?}");
                }
            }
            if let Some(ref volumes) = res.volumes_removed {
                if !volumes.is_empty() {
                    println!("   Removed volumes: {volumes:?}");
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("⚠️ Cleanup endpoint failed: {e}");
            // Continue to container removal anyway
        }
        Err(e) => {
            // Propagate the JoinError instead of panicking
            return Err(Box::new(e) as Box<dyn Error + Send>);
        }
    }

    // 2. Find and remove the coordinator container
    println!("🔍 Looking for coordinator container...");
    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters: {
                let mut filters = HashMap::new();
                filters.insert("name".to_string(), vec![coord_filter.to_string()]);
                Some(filters)
            },
            ..Default::default()
        }))
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    if containers.is_empty() {
        println!("✅ No coordinator container found");
        return Ok(());
    }

    for container in containers {
        if let Some(id) = &container.id {
            println!("🗑️ Removing container: {id}");
            docker
                .remove_container(
                    id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
            println!("✅ Container removed");
        }
    }

    Ok(())
}

pub async fn start_coordinator(
    docker: Docker,
    img: String,
    port: u16,
    registry: Option<String>,
) -> Result<(), Box<dyn Error + Send>> {
    let setup_name = "cyanprint-coordinator-setup";
    let coord_name = "cyanprint-coordinator";
    let coord_filter = "^cyanprint-coordinator$";

    // Check if coordinator is already running
    println!("🔍 Checking if coordinator is already running...");
    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: true, // Include both running and stopped containers
            filters: {
                let mut filters = HashMap::new();
                filters.insert("name".to_string(), vec![coord_filter.to_string()]);
                Some(filters)
            },
            ..Default::default()
        }))
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    if !containers.is_empty() {
        // Check if any container is running
        let running_containers: Vec<_> = containers
            .iter()
            .filter(|c| {
                c.state
                    .as_ref()
                    .is_some_and(|s| *s == ContainerSummaryStateEnum::RUNNING)
            })
            .collect();

        if !running_containers.is_empty() {
            println!("✅ Coordinator is already running on port {port}.");
            return Ok(());
        } else {
            // Container exists but is not running, remove it
            println!("🧹 Found stopped coordinator container, removing it...");
            for container in containers {
                if let Some(id) = &container.id {
                    docker
                        .remove_container(
                            id,
                            Some(RemoveContainerOptions {
                                force: false,
                                ..Default::default()
                            }),
                        )
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
                    println!("✅ Removed stopped container: {id}");
                }
            }
        }
    }

    let mount = Mount {
        target: Some(if cfg!(windows) {
            String::from("//var/run/docker.sock")
        } else {
            String::from("/var/run/docker.sock")
        }),
        source: Some(if cfg!(windows) {
            String::from("//var/run/docker.sock")
        } else {
            String::from("/var/run/docker.sock")
        }),
        typ: Some(MountTypeEnum::BIND),
        consistency: Some(String::from("default")),
        ..Default::default()
    };

    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        String::from("9000/tcp"),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(port.to_string()),
        }]),
    );
    println!("🔧 Using image to configure the coordinator: {img}");
    println!("⏬ Pulling image...");
    docker
        .clone()
        .create_image(
            Some(CreateImageOptions {
                from_image: Some(img.clone()),
                ..Default::default()
            }),
            None,
            None,
        )
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    println!("✅ Image pulled");

    println!("⚙️ Setting up Coordinator Network...");
    let network = docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(setup_name.to_string()),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(img.clone()),
                cmd: Some(vec!["setup".to_string()]),
                host_config: Some(HostConfig {
                    mounts: Some(vec![mount.clone()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?
        .id;
    docker
        .start_container(&network, None::<StartContainerOptions>)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    let mut streams = docker.logs(
        setup_name,
        Some(LogsOptions {
            follow: true,
            stdout: true,
            stderr: false,
            ..Default::default()
        }),
    );
    while let Some(msg) = streams.next().await {
        println!("{msg:#?}");
    }
    docker
        .remove_container(setup_name, None::<RemoveContainerOptions>)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    println!("✅ CyanPrint Coordinator Network Started");
    println!("⚙️ Starting Coordinator...");
    let mut coordinator_cmd = vec![];
    if let Some(registry_url) = registry {
        coordinator_cmd.push("start".to_string());
        coordinator_cmd.push("--registry".to_string());
        coordinator_cmd.push(registry_url);
    }

    let c = docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(coord_name.to_string()),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(img),
                cmd: if coordinator_cmd.is_empty() {
                    None
                } else {
                    Some(coordinator_cmd)
                },
                exposed_ports: Some(vec!["9000/tcp".to_string()]),
                host_config: Some(HostConfig {
                    mounts: Some(vec![mount]),
                    port_bindings: Some(port_bindings),
                    network_mode: Some("cyanprint".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    docker
        .start_container(&c.id, None::<StartContainerOptions>)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    Ok(())
}
