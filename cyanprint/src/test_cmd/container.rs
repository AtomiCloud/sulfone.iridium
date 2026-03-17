//! Docker container management for processor/plugin/resolver tests.
//!
//! This module provides functionality for:
//! - Building Docker images from Dockerfiles
//! - Starting containers with bind mounts
//! - Health checking containers
//! - Cleaning up containers and images

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use bollard::Docker;
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{CreateContainerOptions, ListContainersOptions};
use tokio::runtime::Builder;

use cyanregistry::cli::mapper::read_build_config;

use crate::docker::buildx::{BuildOptions, BuildOutput, BuildxBuilder};
use crate::port::find_available_port;

/// RAII guard that ensures all containers created during a test run are cleaned up.
///
/// On drop, removes all Docker containers labeled with `cyanprint.test.run=<run_id>`.
/// This guarantees cleanup even on panics or early returns.
///
/// Used uniformly across all test entry points (template, plugin, processor, resolver).
pub(crate) struct RunGuard {
    run_id: String,
}

impl RunGuard {
    pub(crate) fn new(run_id: String) -> Self {
        Self { run_id }
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        let Ok(docker) = Docker::connect_with_local_defaults() else {
            return;
        };
        let Ok(rt) = Builder::new_multi_thread().enable_all().build() else {
            return;
        };

        let run_id = self.run_id.clone();
        rt.block_on(async {
            let mut filters = HashMap::new();
            filters.insert(
                "label".to_string(),
                vec![format!("cyanprint.test.run={run_id}")],
            );

            let options = ListContainersOptions {
                all: true,
                filters: Some(filters),
                ..Default::default()
            };

            if let Ok(containers) = docker.list_containers(Some(options)).await {
                for container in containers {
                    if let Some(id) = &container.id {
                        let name = container
                            .names
                            .as_ref()
                            .and_then(|n| n.first())
                            .map(|n| n.trim_start_matches('/').to_string())
                            .unwrap_or_else(|| id.clone());
                        println!("  RunGuard: removing container: {name}");
                        let _ = docker.stop_container(id, None).await;
                        let _ = docker.remove_container(id, None).await;
                    }
                }
            }
        });
    }
}

/// Container handle for running containers.
///
/// Contains a container name, allocated host port, and image reference.
/// Used to manage a container lifecycle during tests.
#[derive(Debug, Clone)]
pub struct ContainerHandle {
    /// Name of a running container
    pub container_name: String,

    /// Port allocated on host (mapped to container's internal port)
    pub host_port: u16,

    /// Docker image reference (for cleanup)
    pub image_ref: String,

    /// Docker client for container operations
    pub docker: Option<Docker>,
}

/// Build and start a Docker container for testing.
///
/// This function:
/// - Builds a Docker image from a Dockerfile specified in cyan.yaml
/// - Creates and starts a container with appropriate bind mounts
/// - Performs a health check on the container
///
/// # Arguments
///
/// * `artifact_path` - Path to the artifact directory (containing cyan.yaml)
/// * `artifact_type` - Type of the artifact: "processor", "plugin", or "resolver"
/// * `bind_mounts` - Optional bind mounts in format [(host_path, container_path, read_only)]
/// * `internal_port` - Internal port that the container listens on (5551 for processor, 5552 for plugin, 5553 for resolver)
///
/// # Returns
///
/// Returns a [`ContainerHandle`] with the container name, allocated host port, and image reference.
///
/// # Errors
///
/// Returns an error if:
/// - cyan.yaml is not found or invalid
/// - No build configuration exists for the specified artifact type
/// - Docker build fails
/// - Container creation or startup fails
/// - Health check fails
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::container::build_and_start_container;
///
/// let handle = build_and_start_container(
///     "/path/to/processor",
///     "processor",
///     Some(vec![("/host/input", "/workspace/input", true)]),
///     5551,
/// ).unwrap();
///
/// println!("Container running on port {}", handle.host_port);
/// ```
pub fn build_and_start_container(
    artifact_path: &str,
    artifact_type: &str,
    bind_mounts: Option<Vec<(String, String, bool)>>,
    internal_port: u16,
    run_id: Option<&str>,
) -> Result<ContainerHandle, Box<dyn Error + Send>> {
    // Read the build configuration
    let config_path = PathBuf::from(artifact_path).join("cyan.yaml");
    let build_config = read_build_config(config_path.to_string_lossy().to_string())?;

    let registry = build_config.registry.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No registry configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let images = build_config.images.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No images configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    // Get the image config for the artifact type
    let image_config = match artifact_type {
        "processor" => images.processor.as_ref(),
        "plugin" => images.plugin.as_ref(),
        "resolver" => images.resolver.as_ref(),
        _ => None,
    };

    let image_config = image_config.ok_or_else(|| {
        Box::new(std::io::Error::other(format!(
            "No {artifact_type} image configuration found in cyan.yaml"
        ))) as Box<dyn Error + Send>
    })?;

    // Check if Dockerfile exists
    let dockerfile_path = PathBuf::from(artifact_path).join(&image_config.dockerfile);
    if !dockerfile_path.exists() {
        return Err(Box::new(std::io::Error::other(format!(
            "Dockerfile not found at {}",
            dockerfile_path.display()
        ))) as Box<dyn Error + Send>);
    }

    let context_path = PathBuf::from(artifact_path).join(&image_config.context);
    if !context_path.exists() {
        return Err(Box::new(std::io::Error::other(format!(
            "Build context not found at {}",
            context_path.display()
        ))) as Box<dyn Error + Send>);
    }

    let image_name = image_config.image.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other(format!(
            "{artifact_type} image name not specified in build config"
        ))) as Box<dyn Error + Send>
    })?;

    // Build image using BuildxBuilder with absolute paths
    println!("  Building {artifact_type} image...");
    let builder = BuildxBuilder::new();
    builder.build(BuildOptions {
        registry,
        image_name,
        tag: "latest",
        dockerfile: dockerfile_path.to_string_lossy().as_ref(),
        context: context_path.to_string_lossy().as_ref(),
        platforms: &[],
        no_cache: false,
        dry_run: false,
        output: BuildOutput::Load,
    })?;

    let image_ref = format!("{registry}/{image_name}:latest");
    println!("  {artifact_type} image built: {image_ref}");

    // Connect to Docker early so we can clean up on failure
    let docker =
        Docker::connect_with_local_defaults().map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Helper closure to remove the built image on error
    let cleanup_image = |docker: &Docker, image_ref: &str| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build();
        if let Ok(rt) = rt {
            let _ = rt.block_on(async {
                docker
                    .remove_image(
                        image_ref,
                        None::<bollard::query_parameters::RemoveImageOptions>,
                        None::<bollard::auth::DockerCredentials>,
                    )
                    .await
            });
        }
    };

    // Helper closure to stop+remove a container on error
    let cleanup_partial_container = |docker: &Docker, container_name: &str| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build();
        if let Ok(rt) = rt {
            rt.block_on(async {
                let _ = docker.stop_container(container_name, None).await;
                let _ = docker.remove_container(container_name, None).await;
            });
        }
    };

    // Find an available port
    let (port_range_start, port_range_end) = match artifact_type {
        "processor" => (5500, 5599),
        "plugin" => (5600, 5699),
        "resolver" => (5700, 5799),
        _ => (5500, 5599),
    };
    let host_port = match find_available_port(port_range_start, port_range_end) {
        Some(port) => port,
        None => {
            cleanup_image(&docker, &image_ref);
            return Err(Box::new(std::io::Error::other(format!(
                "No available port in range {port_range_start}-{port_range_end}"
            ))) as Box<dyn Error + Send>);
        }
    };

    let id = uuid::Uuid::new_v4().to_string().replace('-', "");
    let container_name = format!("cyan-{artifact_type}-{id}-test");

    // Build bind mounts
    let mut binds_vec = Vec::new();
    if let Some(mounts) = bind_mounts {
        for (host_path, container_path, read_only) in mounts {
            let ro_suffix = if read_only { ":ro" } else { "" };
            binds_vec.push(format!("{host_path}:{container_path}{ro_suffix}"));
        }
    }

    // Create port binding
    let port_binding = format!("{internal_port}/tcp");
    let mut port_bindings_map = HashMap::new();
    port_bindings_map.insert(
        port_binding.clone(),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(host_port.to_string()),
        }]),
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| {
            cleanup_image(&docker, &image_ref);
            Box::new(e) as Box<dyn Error + Send>
        })?;

    let container_created = runtime.block_on(async {
        // Image already built, create container
        println!("  Creating container {container_name}...");
        let config = ContainerCreateBody {
            image: Some(image_ref.clone()),
            exposed_ports: Some(vec![port_binding.clone()]),
            labels: Some({
                let mut labels = HashMap::new();
                labels.insert("cyanprint.test".to_string(), "true".to_string());
                if let Some(run_id) = run_id {
                    labels.insert("cyanprint.test.run".to_string(), run_id.to_string());
                }
                labels
            }),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings_map),
                binds: Some(binds_vec),
                network_mode: Some("cyanprint".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let options = Some(CreateContainerOptions {
            name: Some(container_name.clone()),
            ..Default::default()
        });

        docker
            .create_container(options, config)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        // Start container
        println!("  Starting container...");
        docker
            .start_container(&container_name, None)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

        Result::<(), Box<dyn Error + Send>>::Ok(())
    });

    if let Err(e) = container_created {
        cleanup_partial_container(&docker, &container_name);
        cleanup_image(&docker, &image_ref);
        return Err(e);
    }

    println!("  Container started on port {host_port}");

    // Health check
    if let Err(e) = health_check_container(host_port, 30, 2) {
        cleanup_partial_container(&docker, &container_name);
        cleanup_image(&docker, &image_ref);
        return Err(e);
    }

    Ok(ContainerHandle {
        container_name,
        host_port,
        image_ref,
        docker: Some(docker),
    })
}

/// Health check a container by polling its HTTP endpoint.
///
/// Sends GET requests to `http://localhost:{port}/` until successful or timeout.
///
/// # Arguments
///
/// * `port` - Port to check
/// * `max_retries` - Maximum number of retries (default 30)
/// * `retry_delay_secs` - Delay between retries in seconds (default 2)
///
/// # Returns
///
/// Returns `Ok(())` when health check succeeds.
///
/// # Errors
///
/// Returns an error if health check fails after all retries.
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::container::health_check_container;
///
/// // Wait up to 30 seconds for container to be healthy
/// health_check_container(5551, 30, 2).unwrap();
/// ```
pub fn health_check_container(
    port: u16,
    max_retries: u32,
    retry_delay_secs: u64,
) -> Result<(), Box<dyn Error + Send>> {
    use reqwest::blocking::Client;

    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    let url = format!("http://localhost:{port}/");

    for attempt in 0..max_retries {
        match client.get(&url).send() {
            Ok(_) => {
                // Any HTTP response means the container is up and listening.
                // The root path may return 404 since cyan SDK containers only
                // serve on their specific API endpoints (/api/plug, etc.).
                println!("  Container health check passed");
                return Ok(());
            }
            Err(_) => {
                // Connection error - retry
            }
        }

        if attempt < max_retries - 1 {
            println!(
                "  Container not ready, retrying in {}s... ({}/{})",
                retry_delay_secs,
                attempt + 1,
                max_retries
            );
            thread::sleep(Duration::from_secs(retry_delay_secs));
        }
    }

    Err(Box::new(std::io::Error::other(format!(
        "Container health check failed after {max_retries} attempts"
    ))) as Box<dyn Error + Send>)
}

/// Cleanup a container and its image.
///
/// Stops and removes container, then removes Docker image.
///
/// # Arguments
///
/// * `handle` - Container handle to clean up
///
/// # Returns
///
/// Returns `Ok(())` when cleanup completes successfully.
///
/// # Errors
///
/// Returns an error if cleanup fails. Container stop errors are ignored,
/// but container/image removal errors are propagated immediately.
///
/// # Example
///
/// ```no_run
/// use cyanprint::test_cmd::container::{build_and_start_container, cleanup_container};
///
/// let handle = build_and_start_container(
///     "/path/to/processor",
///     "processor",
///     None,
///     5551,
/// ).unwrap();
///
/// // ... use the container ...
///
/// cleanup_container(&handle).unwrap();
/// ```
pub fn cleanup_container(handle: &ContainerHandle) -> Result<(), Box<dyn Error + Send>> {
    let docker = handle.docker.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("Docker client not available")) as Box<dyn Error + Send>
    })?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Box::new(e) as Box<dyn Error + Send>)?;

    // Stop and remove container
    println!("  Removing container {}", handle.container_name);
    let container_err = runtime
        .block_on(async {
            let _ = docker.stop_container(&handle.container_name, None).await;

            docker
                .remove_container(&handle.container_name, None)
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })
        .err();

    // Remove image (always attempt, even if container removal failed)
    println!("  Removing image {}", handle.image_ref);
    let image_err = runtime
        .block_on(async {
            docker
                .remove_image(
                    &handle.image_ref,
                    None::<bollard::query_parameters::RemoveImageOptions>,
                    None::<bollard::auth::DockerCredentials>,
                )
                .await
                .map_err(|e| Box::new(e) as Box<dyn Error + Send>)
        })
        .err();

    // Return combined error if either step failed
    match (container_err, image_err) {
        (Some(e), None) => Err(e),
        (None, Some(e)) => Err(e),
        (Some(e1), Some(e2)) => Err(Box::new(std::io::Error::other(format!(
            "Container cleanup failed: {e1}; Image cleanup failed: {e2}"
        ))) as Box<dyn Error + Send>),
        (None, None) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_handle_creation() {
        let handle = ContainerHandle {
            container_name: "test-container".to_string(),
            host_port: 5551,
            image_ref: "test/image:latest".to_string(),
            docker: None,
        };

        assert_eq!(handle.container_name, "test-container");
        assert_eq!(handle.host_port, 5551);
        assert_eq!(handle.image_ref, "test/image:latest");
    }

    #[test]
    fn test_container_name_generation() {
        // Container names should follow pattern: cyan-{artifact_type}-{uuid}-test
        let artifact_type = "processor";
        let id = uuid::Uuid::new_v4().to_string().replace('-', "");
        let container_name = format!("cyan-{artifact_type}-{id}-test");

        assert!(container_name.starts_with("cyan-processor-"));
        assert!(container_name.ends_with("-test"));
        assert!(container_name.len() > "cyan-processor--test".len());
    }

    #[test]
    fn test_bind_mount_path_read_only() {
        let host_path = "/host/input";
        let container_path = "/workspace/input";
        let read_only = true;

        let bind = format!(
            "{}:{}{}",
            host_path,
            container_path,
            if read_only { ":ro" } else { "" }
        );

        assert_eq!(bind, "/host/input:/workspace/input:ro");
        assert!(bind.contains(":ro"));
    }

    #[test]
    fn test_bind_mount_path_read_write() {
        let host_path = "/host/output";
        let container_path = "/workspace/output";
        let read_only = false;

        let bind = format!(
            "{}:{}{}",
            host_path,
            container_path,
            if read_only { ":ro" } else { "" }
        );

        assert_eq!(bind, "/host/output:/workspace/output");
        assert!(!bind.contains(":ro"));
    }
}
