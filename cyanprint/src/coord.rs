use std::error::Error;

use bollard::container::{Config, CreateContainerOptions, LogsOptions};
use bollard::image::CreateImageOptions;
use bollard::models::{HostConfig, Mount, MountTypeEnum, PortBinding};
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

    let mut port_bindings = ::std::collections::HashMap::new();
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
                from_image: img.clone(),
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
                name: setup.to_string(),
                platform: None,
            }),
            Config {
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
        .start_container::<String>(&network, None)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    let mut streams = docker.logs::<String>(
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
        .remove_container(setup, None)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    println!("✅ CyanPrint Coordinator Network Started");
    println!("⚙️ Starting Coordinator...");
    let c = docker
        .create_container(
            Some(CreateContainerOptions {
                name: coord.to_string(),
                platform: None,
            }),
            Config {
                image: Some(img),
                exposed_ports: Some(
                    vec![("9000/tcp".to_string(), ::std::collections::HashMap::new())]
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
        .start_container::<String>(&c.id, None)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;
    Ok(())
}
