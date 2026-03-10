use std::error::Error;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use bollard::Docker;
use clap::Parser;

use cyancoordinator::client::{CyanCoordinatorClient, new_client};
use cyancoordinator::session::DefaultSessionIdGenerator;
use cyanregistry::cli::mapper::read_build_config;
use cyanregistry::http::client::CyanRegistryClient;

use crate::commands::{Cli, Commands, DaemonCommands, PushArgs, PushCommands};
use crate::coord::{start_coordinator, stop_coordinator};
use crate::docker::{BuildOptions, BuildxBuilder};
use crate::run::cyan_run;
use crate::update::cyan_update;
use crate::util::parse_ref;

pub mod commands;
pub mod coord;
pub mod docker;
pub mod errors;
pub mod run;
pub mod update;
pub mod util;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error + Send>> {
    let http_client = new_client()?;
    let http = Rc::new(http_client);

    let cli = Cli::parse();
    let registry = CyanRegistryClient {
        endpoint: cli.registry.to_string(),
        version: "1.0".to_string(),
        client: Rc::clone(&http),
    };
    match cli.command {
        Commands::Build {
            tag,
            config,
            platform,
            builder,
            no_cache,
            dry_run,
            folder,
        } => handle_build(tag, config, folder, platform, builder, no_cache, dry_run),
        Commands::Push(push_arg) => match push_arg.commands {
            PushCommands::Processor { build, image, tag } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    platform,
                    builder,
                    no_cache,
                    dry_run,
                    folder,
                    ..
                } = push_arg;

                let (image_ref, tag_val) = if let Some(build_tag) = build {
                    // Build mode
                    if image.is_some() || tag.is_some() {
                        eprintln!("❌ Error: --build cannot be used with image arguments");
                        return Err(Box::new(std::io::Error::other(
                            "--build cannot be used with image arguments",
                        )) as Box<dyn Error + Send>);
                    }

                    let result = build_for_push(
                        &config,
                        &folder,
                        &build_tag,
                        &["processor"],
                        platform.as_deref(),
                        builder.as_deref(),
                        no_cache,
                        dry_run,
                    )?;

                    let image_ref = format!("{}/{}", result.registry, result.image);
                    (image_ref, build_tag)
                } else {
                    // Push existing mode
                    match (image, tag) {
                        (Some(i), Some(t)) => (i, t),
                        _ => {
                            eprintln!("❌ Error: must provide either --build or image and tag");
                            return Err(Box::new(std::io::Error::other(
                                "must provide either --build or image and tag",
                            )) as Box<dyn Error + Send>);
                        }
                    }
                };

                if dry_run {
                    println!("🏃 Dry-run complete - skipping registry push");
                    return Ok(());
                }

                let res = registry.push_processor(config, token, message, image_ref, tag_val);
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
                build,
                template_image,
                template_tag,
                blob_image,
                blob_tag,
            } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    platform,
                    builder,
                    no_cache,
                    dry_run,
                    folder,
                    ..
                } = push_arg;

                let (blob_ref, blob_tag_val, template_ref, template_tag_val) =
                    if let Some(build_tag) = build {
                        // Build mode
                        if blob_image.is_some()
                            || blob_tag.is_some()
                            || template_image.is_some()
                            || template_tag.is_some()
                        {
                            eprintln!("❌ Error: --build cannot be used with image arguments");
                            return Err(Box::new(std::io::Error::other(
                                "--build cannot be used with image arguments",
                            )) as Box<dyn Error + Send>);
                        }

                        let result = build_for_push(
                            &config,
                            &folder,
                            &build_tag,
                            &["template", "blob"],
                            platform.as_deref(),
                            builder.as_deref(),
                            no_cache,
                            dry_run,
                        )?;

                        let blob_ref = format!("{}/{}", result.registry, result.blob_image);
                        let template_ref = format!("{}/{}", result.registry, result.image);
                        (blob_ref, build_tag.clone(), template_ref, build_tag)
                    } else {
                        // Push existing mode
                        match (blob_image, blob_tag, template_image, template_tag) {
                            (Some(bi), Some(bt), Some(ti), Some(tt)) => (bi, bt, ti, tt),
                            _ => {
                                eprintln!(
                                    "❌ Error: must provide either --build or all image arguments"
                                );
                                return Err(Box::new(std::io::Error::other(
                                    "must provide either --build or all image arguments",
                                ))
                                    as Box<dyn Error + Send>);
                            }
                        }
                    };

                if dry_run {
                    println!("🏃 Dry-run complete - skipping registry push");
                    return Ok(());
                }

                let res = registry.push_template(
                    config,
                    token,
                    message,
                    blob_ref,
                    blob_tag_val,
                    template_ref,
                    template_tag_val,
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
                // Group subcommand does not support --build (no Docker images)
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
            PushCommands::Plugin { build, image, tag } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    platform,
                    builder,
                    no_cache,
                    dry_run,
                    folder,
                    ..
                } = push_arg;

                let (image_ref, tag_val) = if let Some(build_tag) = build {
                    // Build mode
                    if image.is_some() || tag.is_some() {
                        eprintln!("❌ Error: --build cannot be used with image arguments");
                        return Err(Box::new(std::io::Error::other(
                            "--build cannot be used with image arguments",
                        )) as Box<dyn Error + Send>);
                    }

                    let result = build_for_push(
                        &config,
                        &folder,
                        &build_tag,
                        &["plugin"],
                        platform.as_deref(),
                        builder.as_deref(),
                        no_cache,
                        dry_run,
                    )?;

                    let image_ref = format!("{}/{}", result.registry, result.image);
                    (image_ref, build_tag)
                } else {
                    // Push existing mode
                    match (image, tag) {
                        (Some(i), Some(t)) => (i, t),
                        _ => {
                            eprintln!("❌ Error: must provide either --build or image and tag");
                            return Err(Box::new(std::io::Error::other(
                                "must provide either --build or image and tag",
                            )) as Box<dyn Error + Send>);
                        }
                    }
                };

                if dry_run {
                    println!("🏃 Dry-run complete - skipping registry push");
                    return Ok(());
                }

                let res = registry.push_plugin(config, token, message, image_ref, tag_val);
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
            PushCommands::Resolver { build, image, tag } => {
                let PushArgs {
                    config,
                    token,
                    message,
                    platform,
                    builder,
                    no_cache,
                    dry_run,
                    folder,
                    ..
                } = push_arg;

                let (image_ref, tag_val) = if let Some(build_tag) = build {
                    // Build mode
                    if image.is_some() || tag.is_some() {
                        eprintln!("❌ Error: --build cannot be used with image arguments");
                        return Err(Box::new(std::io::Error::other(
                            "--build cannot be used with image arguments",
                        )) as Box<dyn Error + Send>);
                    }

                    let result = build_for_push(
                        &config,
                        &folder,
                        &build_tag,
                        &["resolver"],
                        platform.as_deref(),
                        builder.as_deref(),
                        no_cache,
                        dry_run,
                    )?;

                    let image_ref = format!("{}/{}", result.registry, result.image);
                    (image_ref, build_tag)
                } else {
                    // Push existing mode
                    match (image, tag) {
                        (Some(i), Some(t)) => (i, t),
                        _ => {
                            eprintln!("❌ Error: must provide either --build or image and tag");
                            return Err(Box::new(std::io::Error::other(
                                "must provide either --build or image and tag",
                            )) as Box<dyn Error + Send>);
                        }
                    }
                };

                if dry_run {
                    println!("🏃 Dry-run complete - skipping registry push");
                    return Ok(());
                }

                let res = registry.push_resolver(config, token, message, image_ref, tag_val);
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

/// Get the current platform for Docker builds
/// Maps host OS/arch to Docker platform string
fn get_current_platform() -> Vec<String> {
    let current = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let platform_str = match (os, current) {
        ("linux", "x86_64") => "linux/amd64",
        ("linux", "aarch64") => "linux/arm64",
        ("macos", "x86_64") => "linux/amd64", // Docker on macOS typically uses linux containers
        ("macos", "aarch64") => "linux/arm64",
        ("windows", "x86_64") => "linux/amd64",
        _ => "linux/amd64", // Default fallback
    };
    vec![platform_str.to_string()]
}

/// Resolve platforms for build: CLI override → config (non-empty) → current platform
///
/// # Arguments
/// * `cli_platform` - Optional comma-separated platforms from CLI --platform flag
/// * `config_platforms` - Optional platforms from config file (may be empty vec)
/// * `get_current` - Function to get current platform (for testability)
fn resolve_platforms<F>(
    cli_platform: Option<&str>,
    config_platforms: Option<&Vec<String>>,
    get_current: F,
) -> Vec<String>
where
    F: FnOnce() -> Vec<String>,
{
    if let Some(p) = cli_platform {
        p.split(',').map(|s| s.trim().to_string()).collect()
    } else if let Some(config_platforms) = config_platforms {
        // Treat empty platforms vector same as None - fall back to current platform
        if config_platforms.is_empty() {
            get_current()
        } else {
            config_platforms.clone()
        }
    } else {
        // Default to current platform only
        get_current()
    }
}

/// Handle the build command
fn handle_build(
    tag: String,
    config: String,
    folder: String,
    platform: Option<String>,
    builder: Option<String>,
    no_cache: bool,
    dry_run: bool,
) -> Result<(), Box<dyn Error + Send>> {
    println!("🔨 Building Docker images with tag: {tag}");

    // Change to folder directory before loading config
    // Note: We use set_current_dir here because this is a CLI tool that runs
    // a single operation and exits. The alternative (using absolute paths throughout) would be
    // more complex and error-prone for this use case.
    let folder_path = Path::new(&folder);
    let folder_absolute = folder_path.canonicalize().map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to resolve folder path: {e}"
        ))) as Box<dyn Error + Send>
    })?;
    std::env::set_current_dir(&folder_absolute).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to change directory to {}: {e}",
            folder_absolute.display()
        ))) as Box<dyn Error + Send>
    })?;

    // Pre-flight checks (skip in dry-run mode)
    if !dry_run {
        println!("🔍 Running pre-flight checks...");

        if let Err(e) = BuildxBuilder::check_docker() {
            eprintln!("❌ Error: {e}");
            return Err(e);
        }
        println!("  ✓ Docker daemon is running");

        if let Err(e) = BuildxBuilder::check_buildx() {
            eprintln!("❌ Error: {e}");
            return Err(e);
        }
        println!("  ✓ Docker buildx is available");
    }

    // Load and parse config file (now relative to folder)
    println!("📄 Loading configuration from: {config}");
    let build_config = read_build_config(config.clone())?;

    // After validation, registry and images are guaranteed to be Some
    let registry = build_config.registry.as_ref().unwrap();
    let images = build_config.images.as_ref().unwrap();

    println!("  ✓ Configuration loaded successfully");
    println!("  ✓ Registry: {registry}");

    // Resolve platforms: CLI override → config (non-empty) → current platform
    let platforms = resolve_platforms(
        platform.as_deref(),
        build_config.platforms.as_ref(),
        get_current_platform,
    );

    if !platforms.is_empty() {
        println!("  ✓ Platforms: {}", platforms.join(", "));
    }

    // Create builder
    let mut buildx = BuildxBuilder::new();
    if let Some(ref b) = builder {
        buildx = buildx.with_builder(b);
        println!("  ✓ Using builder: {b}");
    }

    // Track build results
    let mut success_count = 0;
    let mut fail_count = 0;
    let mut images_to_build: Vec<(&str, &cyanregistry::cli::models::build_config::ImageConfig)> =
        Vec::new();

    // Collect images to build
    if let Some(ref img) = images.template {
        images_to_build.push(("template", img));
    }
    if let Some(ref img) = images.blob {
        images_to_build.push(("blob", img));
    }
    if let Some(ref img) = images.processor {
        images_to_build.push(("processor", img));
    }
    if let Some(ref img) = images.plugin {
        images_to_build.push(("plugin", img));
    }
    if let Some(ref img) = images.resolver {
        images_to_build.push(("resolver", img));
    }

    let total_images = images_to_build.len();
    println!("\n📦 Found {total_images} image(s) to build");

    if dry_run {
        println!("🏃 Dry-run mode - showing commands without executing:\n");
    }

    // Build each image
    for (image_type, img_config) in images_to_build {
        let image_name = img_config
            .image
            .as_ref()
            .expect("image field should be validated by mapper");
        println!("\n🔨 Building image: {image_type}");
        println!("  Image name: {image_name}");
        println!("  Dockerfile: {}", img_config.dockerfile);
        println!("  Context: {}", img_config.context);

        let result = buildx.build(BuildOptions {
            registry,
            image_name,
            tag: &tag,
            dockerfile: &img_config.dockerfile,
            context: &img_config.context,
            platforms: &platforms,
            no_cache,
            dry_run,
        });

        match result {
            Ok(_) => {
                println!("  ✅ Successfully built {image_type}");
                success_count += 1;
            }
            Err(e) => {
                eprintln!("  ❌ Failed to build {image_type}: {e}");
                fail_count += 1;
                // Continue building other images even if one fails
            }
        }
    }

    // Print summary
    println!("\n📊 Build Summary:");
    println!("  Total images: {total_images}");
    println!("  Successful: {success_count}");
    println!("  Failed: {fail_count}");

    if fail_count > 0 {
        Err(Box::new(std::io::Error::other(format!(
            "Build failed for {fail_count} image(s)"
        ))) as Box<dyn Error + Send>)
    } else {
        println!("\n✅ All images built successfully!");
        Ok(())
    }
}

/// Result of building images for push --build mode
struct PushBuildResult {
    /// Registry URL from config
    registry: String,
    /// Image name for single-image builds (processor, plugin, resolver)
    image: String,
    /// Image name for template blob (only for template builds)
    blob_image: String,
}

/// Build specific images for push --build mode
///
/// # Arguments
/// * `config_path` - Path to cyan.yaml
/// * `folder` - Working directory for the build
/// * `tag` - Tag to use for built images
/// * `image_names` - List of image types to build (e.g., ["template", "blob"])
/// * `platform` - Optional platform override
/// * `builder` - Optional builder override
/// * `no_cache` - Whether to disable cache
/// * `dry_run` - Whether to show commands without executing
#[allow(clippy::too_many_arguments)]
fn build_for_push(
    config_path: &str,
    folder: &str,
    tag: &str,
    image_names: &[&str],
    platform: Option<&str>,
    builder: Option<&str>,
    no_cache: bool,
    dry_run: bool,
) -> Result<PushBuildResult, Box<dyn Error + Send>> {
    println!("🔨 Building images for push with tag: {tag}");

    // Change to folder directory before loading config
    // Note: We use set_current_dir here because this is a CLI tool that runs
    // a single operation and exits. The alternative (using absolute paths throughout) would be
    // more complex and error-prone for this use case.
    let folder_path = Path::new(folder);
    let folder_absolute = folder_path.canonicalize().map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to resolve folder path: {e}"
        ))) as Box<dyn Error + Send>
    })?;
    std::env::set_current_dir(&folder_absolute).map_err(|e| {
        Box::new(std::io::Error::other(format!(
            "Failed to change directory to {}: {e}",
            folder_absolute.display()
        ))) as Box<dyn Error + Send>
    })?;

    // Pre-flight checks (skip in dry-run mode)
    if !dry_run {
        if let Err(e) = BuildxBuilder::check_docker() {
            eprintln!("❌ Error: {e}");
            return Err(e);
        }
        if let Err(e) = BuildxBuilder::check_buildx() {
            eprintln!("❌ Error: {e}");
            return Err(e);
        }
    }

    // Load config
    println!("📄 Loading configuration from: {config_path}");
    let build_config = read_build_config(config_path.to_string())?;

    let registry = build_config.registry.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No registry configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let images = build_config.images.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No images configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    println!("  ✓ Registry: {registry}");

    // Resolve platforms
    let platforms = resolve_platforms(
        platform,
        build_config.platforms.as_ref(),
        get_current_platform,
    );

    // Create builder
    let mut buildx = BuildxBuilder::new();
    if let Some(b) = builder {
        buildx = buildx.with_builder(b);
        println!("  ✓ Using builder: {b}");
    }

    if dry_run {
        println!("🏃 Dry-run mode - showing commands without executing:\n");
    }

    let mut result = PushBuildResult {
        registry: registry.clone(),
        image: String::new(),
        blob_image: String::new(),
    };

    // Build each requested image
    for image_type in image_names {
        let img_config = match *image_type {
            "template" => images.template.as_ref(),
            "blob" => images.blob.as_ref(),
            "processor" => images.processor.as_ref(),
            "plugin" => images.plugin.as_ref(),
            "resolver" => images.resolver.as_ref(),
            _ => None,
        };

        let img_config = match img_config {
            Some(c) => c,
            None => {
                return Err(Box::new(std::io::Error::other(format!(
                    "No {image_type} image configuration found in cyan.yaml"
                ))) as Box<dyn Error + Send>);
            }
        };

        let image_name = img_config
            .image
            .as_ref()
            .expect("image field should be validated by mapper");

        println!("\n🔨 Building image: {image_type}");
        println!("  Image name: {image_name}");
        println!("  Dockerfile: {}", img_config.dockerfile);
        println!("  Context: {}", img_config.context);

        let build_result = buildx.build(BuildOptions {
            registry,
            image_name,
            tag,
            dockerfile: &img_config.dockerfile,
            context: &img_config.context,
            platforms: &platforms,
            no_cache,
            dry_run,
        });

        if let Err(e) = build_result {
            eprintln!("  ❌ Failed to build {image_type}: {e}");
            return Err(e);
        }

        println!("  ✅ Successfully built {image_type}");

        // Store image names for reference construction
        match *image_type {
            "template" => {
                result.image = img_config
                    .image
                    .clone()
                    .expect("image field should be validated by mapper")
            }
            "blob" => {
                result.blob_image = img_config
                    .image
                    .clone()
                    .expect("image field should be validated by mapper")
            }
            "processor" | "plugin" | "resolver" => {
                result.image = img_config
                    .image
                    .clone()
                    .expect("image field should be validated by mapper")
            }
            _ => {}
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_platforms_cli_override() {
        // CLI platform should take highest priority
        let result = resolve_platforms(
            Some("linux/amd64,linux/arm64"),
            Some(&vec!["linux/386".to_string()]),
            || vec!["fallback".to_string()],
        );
        assert_eq!(result, vec!["linux/amd64", "linux/arm64"]);
    }

    #[test]
    fn test_resolve_platforms_cli_single() {
        // Single platform from CLI
        let result = resolve_platforms(Some("linux/amd64"), None, || vec!["fallback".to_string()]);
        assert_eq!(result, vec!["linux/amd64"]);
    }

    #[test]
    fn test_resolve_platforms_cli_with_spaces() {
        // CLI platforms with extra spaces should be trimmed
        let result = resolve_platforms(Some("linux/amd64 , linux/arm64"), None, || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["linux/amd64", "linux/arm64"]);
    }

    #[test]
    fn test_resolve_platforms_config_platforms() {
        // Config platforms used when no CLI override
        let config_platforms = vec!["linux/amd64".to_string(), "linux/arm64".to_string()];
        let result = resolve_platforms(None, Some(&config_platforms), || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["linux/amd64", "linux/arm64"]);
    }

    #[test]
    fn test_resolve_platforms_empty_config_falls_back() {
        // Empty config platforms should fall back to current platform
        let config_platforms: Vec<String> = vec![];
        let result = resolve_platforms(None, Some(&config_platforms), || {
            vec!["linux/current".to_string()]
        });
        assert_eq!(result, vec!["linux/current"]);
    }

    #[test]
    fn test_resolve_platforms_no_config_falls_back() {
        // No config platforms should fall back to current platform
        let result = resolve_platforms(None, None, || vec!["linux/current".to_string()]);
        assert_eq!(result, vec!["linux/current"]);
    }

    #[test]
    fn test_resolve_platforms_priority_order() {
        // Test priority: CLI > config > fallback
        let config_platforms = vec!["config-platform".to_string()];

        // CLI overrides config
        let result = resolve_platforms(Some("cli-platform"), Some(&config_platforms), || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["cli-platform"]);

        // Config used when no CLI
        let result = resolve_platforms(None, Some(&config_platforms), || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["config-platform"]);

        // Fallback used when no CLI or config
        let result = resolve_platforms(None, None, || vec!["fallback".to_string()]);
        assert_eq!(result, vec!["fallback"]);
    }
}
