use std::collections::HashMap;
use std::error::Error;

use bollard::models::{
    ContainerCreateBody, ContainerSummaryStateEnum, HostConfig, Mount, MountTypeEnum, PortBinding,
};
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptions, ListContainersOptions, LogsOptions,
    RemoveContainerOptions, StartContainerOptions,
};
use bollard::Docker;
use futures_util::stream::StreamExt;
use futures_util::stream::TryStreamExt;

pub async fn start_coordinator(
    docker: Docker,
    img: String,
    port: u16,
) -> Result<(), Box<dyn Error + Send>> {
    let setup = "cyanprint-coordinator-setup";
    let coord = "cyanprint-coordinator";

    // Check if coordinator is already running
    println!("🔍 Checking if coordinator is already running...");
    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: true, // Include both running and stopped containers
            filters: {
                let mut filters = HashMap::new();
                filters.insert("name".to_string(), vec![coord.to_string()]);
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
            println!("✅ Coordinator is already running on port {}.", port);
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
                    println!("✅ Removed stopped container: {}", id);
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
    println!("🔧 Using image to configure the coordinator: {}", img);
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
                name: Some(setup.to_string()),
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
        setup,
        Some(LogsOptions {
            follow: true,
            stdout: true,
            stderr: false,
            ..Default::default()
        }),
    );
    while let Some(msg) = streams.next().await {
        println!("{:#?}", msg);
    }
    docker
        .remove_container(setup, None::<RemoveContainerOptions>)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    println!("✅ CyanPrint Coordinator Network Started");
    println!("⚙️ Starting Coordinator...");
    let c = docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(coord.to_string()),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(img),
                exposed_ports: Some(
                    vec![("9000/tcp".to_string(), HashMap::new())]
                        .into_iter()
                        .collect(),
                ),
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
