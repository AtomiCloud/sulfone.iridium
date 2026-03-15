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

use crate::commands::{
    Cli, Commands, DaemonCommands, PushArgs, PushCommands, TestCommands, TryCommands,
};
use crate::coord::{start_coordinator, stop_coordinator};
use crate::docker::{BuildOptions, BuildOutput, BuildxBuilder};
use crate::run::cyan_run;
use crate::test_cmd::init::run_init;
use crate::test_cmd::report::write_human_report;
use crate::test_cmd::{
    run_plugin_tests, run_processor_tests, run_resolver_tests, run_template_tests,
};
use crate::try_cmd::{execute_try_command, execute_try_group_command};
use crate::update::UserAborted;
use crate::update::cyan_update;
use crate::util::parse_ref;

pub mod commands;
pub mod coord;
pub mod docker;
pub mod errors;
pub mod git;
pub mod port;
pub mod run;
pub mod test_cmd;
pub mod try_cmd;
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
                    if image.is_some() || tag.is_some() {
                        eprintln!("Error: --build cannot be used with image arguments");
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
                    match (image, tag) {
                        (Some(i), Some(t)) => (i, t),
                        _ => {
                            eprintln!("Error: must provide either --build or image and tag");
                            return Err(Box::new(std::io::Error::other(
                                "must provide either --build or image and tag",
                            )) as Box<dyn Error + Send>);
                        }
                    }
                };

                if dry_run {
                    println!("Dry-run complete - skipping registry push");
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
                        if blob_image.is_some()
                            || blob_tag.is_some()
                            || template_image.is_some()
                            || template_tag.is_some()
                        {
                            eprintln!("Error: --build cannot be used with image arguments");
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
                        match (blob_image, blob_tag, template_image, template_tag) {
                            (Some(bi), Some(bt), Some(ti), Some(tt)) => (bi, bt, ti, tt),
                            _ => {
                                eprintln!(
                                    "Error: must provide either --build or all image arguments"
                                );
                                return Err(Box::new(std::io::Error::other(
                                    "must provide either --build or all image arguments",
                                ))
                                    as Box<dyn Error + Send>);
                            }
                        }
                    };

                if dry_run {
                    println!("Dry-run complete - skipping registry push");
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
                        println!("Pushed template successfully");
                        println!("Template ID: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error: {e:#?}");
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
                println!("Pushing template group (no Docker artifacts)...");
                let res = registry.push_template_without_properties(config, token, message);
                match res {
                    Ok(r) => {
                        println!("Pushed template group successfully");
                        println!("Template ID: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error pushing template group: {e:#?}");
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
                    if image.is_some() || tag.is_some() {
                        eprintln!("Error: --build cannot be used with image arguments");
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
                    match (image, tag) {
                        (Some(i), Some(t)) => (i, t),
                        _ => {
                            eprintln!("Error: must provide either --build or image and tag");
                            return Err(Box::new(std::io::Error::other(
                                "must provide either --build or image and tag",
                            )) as Box<dyn Error + Send>);
                        }
                    }
                };

                if dry_run {
                    println!("Dry-run complete - skipping registry push");
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
                    if image.is_some() || tag.is_some() {
                        eprintln!("Error: --build cannot be used with image arguments");
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
                    match (image, tag) {
                        (Some(i), Some(t)) => (i, t),
                        _ => {
                            eprintln!("Error: must provide either --build or image and tag");
                            return Err(Box::new(std::io::Error::other(
                                "must provide either --build or image and tag",
                            )) as Box<dyn Error + Send>);
                        }
                    }
                };

                if dry_run {
                    println!("Dry-run complete - skipping registry push");
                    return Ok(());
                }

                let res = registry.push_resolver(config, token, message, image_ref, tag_val);
                match res {
                    Ok(r) => {
                        println!("Pushed resolver successfully");
                        println!("Resolver ID: {}", r.id);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Error pushing resolver: {e:#?}");
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
                        "Retrieving template '{}/{}:{}' from registry...",
                        u,
                        n,
                        v.unwrap_or(-1)
                    );
                    let r = registry.get_template(u.clone(), n.clone(), v);
                    println!(
                        "Retrieved template '{}/{}:{}' from registry.",
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
                    println!("Completed successfully");
                    let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());
                    println!("Cleaning up all sessions...");
                    for sid in session_ids {
                        println!("Cleaning up session: {sid}");
                        let _ = coord_client.clean(sid);
                    }
                    println!("Cleaned up all sessions");
                }
                Err(e) => {
                    eprintln!("Error: {e:#?}");
                    println!("No sessions to clean up");
                }
            }
            Ok(())
        }
        Commands::Update {
            path,
            coordinator_endpoint,
            interactive,
            force,
        } => {
            let session_id_generator = Box::new(DefaultSessionIdGenerator);
            let coord_client = CyanCoordinatorClient::new(coordinator_endpoint.clone());
            let registry_ref = Rc::new(registry);

            println!("Updating templates to latest versions");

            let r = cyan_update(
                session_id_generator,
                path,
                coord_client.clone(),
                Rc::clone(&registry_ref),
                cli.debug,
                interactive,
                force,
            );

            match r {
                Ok(session_ids) => {
                    println!("Update completed successfully");
                    println!("Cleaning up all sessions...");
                    for sid in session_ids {
                        println!("Cleaning up session: {sid}");
                        let _ = coord_client.clean(sid);
                    }
                    println!("Cleaned up all sessions");
                }
                Err(e) => {
                    if e.is::<UserAborted>() {
                        return Ok(());
                    }
                    eprintln!("Error during update: {e:#?}");
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
                                    println!("Coordinator started on port {port}");
                                })
                        }
                        DaemonCommands::Stop { port } => {
                            stop_coordinator(docker, port).await.map(|_| {
                                println!("Coordinator stopped");
                            })
                        }
                    }
                });

            result
        }
        Commands::Try { command } => match command {
            TryCommands::Template {
                template_path,
                output_path,
                dev,
                keep_containers,
                disable_daemon_autostart,
                coordinator_endpoint,
            } => {
                let registry_ref = Rc::new(registry);
                let _ = execute_try_command(
                    template_path,
                    output_path,
                    dev,
                    keep_containers,
                    disable_daemon_autostart,
                    cli.registry.clone(),
                    coordinator_endpoint,
                    registry_ref,
                )?;
                Ok(())
            }
            TryCommands::Group {
                template_path,
                output_path,
                disable_daemon_autostart,
                coordinator_endpoint,
            } => {
                let registry_ref = Rc::new(registry);
                execute_try_group_command(
                    template_path,
                    output_path,
                    disable_daemon_autostart,
                    coordinator_endpoint,
                    registry_ref,
                )?;
                Ok(())
            }
        },
        Commands::Test { command } => match command {
            TestCommands::Template {
                path,
                test,
                parallel,
                update_snapshots,
                config,
                output,
                junit,
                coordinator_endpoint,
                disable_daemon_autostart,
            } => {
                println!("Running template tests");
                let results = run_template_tests(
                    &path,
                    test.as_deref(),
                    parallel,
                    update_snapshots,
                    &config,
                    &output,
                    junit.as_deref(),
                    &coordinator_endpoint,
                    disable_daemon_autostart,
                    &registry,
                )?;

                write_human_report(&results);

                let failed = results.iter().filter(|r| !r.passed).count();
                if failed > 0 {
                    Err(
                        Box::new(std::io::Error::other(format!("{failed} test(s) failed")))
                            as Box<dyn Error + Send>,
                    )
                } else {
                    Ok(())
                }
            }
            TestCommands::Processor {
                path,
                test,
                parallel,
                update_snapshots,
                config,
                output,
                junit,
                coordinator_endpoint,
                disable_daemon_autostart,
            } => {
                println!("Running processor tests");
                let results = run_processor_tests(
                    &path,
                    test.as_deref(),
                    parallel,
                    update_snapshots,
                    &config,
                    &output,
                    junit.as_deref(),
                    &coordinator_endpoint,
                    disable_daemon_autostart,
                )?;

                write_human_report(&results);

                let failed = results.iter().filter(|r| !r.passed).count();
                if failed > 0 {
                    Err(
                        Box::new(std::io::Error::other(format!("{failed} test(s) failed")))
                            as Box<dyn Error + Send>,
                    )
                } else {
                    Ok(())
                }
            }
            TestCommands::Plugin {
                path,
                test,
                parallel,
                update_snapshots,
                config,
                output,
                junit,
                coordinator_endpoint,
                disable_daemon_autostart,
            } => {
                println!("Running plugin tests");
                let results = run_plugin_tests(
                    &path,
                    test.as_deref(),
                    parallel,
                    update_snapshots,
                    &config,
                    &output,
                    junit.as_deref(),
                    &coordinator_endpoint,
                    disable_daemon_autostart,
                )?;

                write_human_report(&results);

                let failed = results.iter().filter(|r| !r.passed).count();
                if failed > 0 {
                    Err(
                        Box::new(std::io::Error::other(format!("{failed} test(s) failed")))
                            as Box<dyn Error + Send>,
                    )
                } else {
                    Ok(())
                }
            }
            TestCommands::Resolver {
                path,
                test,
                parallel,
                update_snapshots,
                config,
                output,
                junit,
                coordinator_endpoint,
                disable_daemon_autostart,
            } => {
                println!("Running resolver tests");
                let results = run_resolver_tests(
                    &path,
                    test.as_deref(),
                    parallel,
                    update_snapshots,
                    &config,
                    &output,
                    junit.as_deref(),
                    &coordinator_endpoint,
                    disable_daemon_autostart,
                )?;

                write_human_report(&results);

                let failed = results.iter().filter(|r| !r.passed).count();
                if failed > 0 {
                    Err(
                        Box::new(std::io::Error::other(format!("{failed} test(s) failed")))
                            as Box<dyn Error + Send>,
                    )
                } else {
                    Ok(())
                }
            }
            TestCommands::Init {
                path,
                max_combinations,
                text_seed,
                password_seed,
                date_seed,
                parallel,
                output,
                config,
                coordinator_endpoint,
                interactive,
                disable_daemon_autostart,
            } => {
                println!("Initializing test configuration");
                run_init(
                    &path,
                    max_combinations,
                    text_seed.as_deref(),
                    password_seed.as_deref(),
                    date_seed.as_deref(),
                    parallel,
                    interactive,
                    &output,
                    &config,
                    &coordinator_endpoint,
                    disable_daemon_autostart,
                    &registry,
                )?;
                Ok(())
            }
        },
    }
}

fn handle_build(
    tag: String,
    config: String,
    folder: String,
    platform: Option<String>,
    builder: Option<String>,
    no_cache: bool,
    dry_run: bool,
) -> Result<(), Box<dyn Error + Send>> {
    println!("Building Docker images with tag: {tag}");

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

    if !dry_run {
        println!("Running pre-flight checks...");

        if let Err(e) = BuildxBuilder::check_docker() {
            eprintln!("Error: {e}");
            return Err(e);
        }
        println!("  Docker daemon is running");

        if let Err(e) = BuildxBuilder::check_buildx() {
            eprintln!("Error: {e}");
            return Err(e);
        }
        println!("  Docker buildx is available");
    }

    println!("Loading configuration from: {config}");
    let build_config = read_build_config(config.clone())?;

    let registry = build_config.registry.as_ref().unwrap();
    let images = build_config.images.as_ref().unwrap();

    println!("  Configuration loaded successfully");
    println!("  Registry: {registry}");

    let platforms = resolve_platforms(
        platform.as_deref(),
        build_config.platforms.as_ref(),
        get_current_platform,
    );

    if !platforms.is_empty() {
        println!("  Platforms: {}", platforms.join(", "));
    }

    let mut buildx = BuildxBuilder::new();
    if let Some(ref b) = builder {
        buildx = buildx.with_builder(b);
        println!("  Using builder: {b}");
    }

    let mut success_count = 0;
    let mut fail_count = 0;
    let mut images_to_build: Vec<(&str, &cyanregistry::cli::models::build_config::ImageConfig)> =
        Vec::new();

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
    println!("\nFound {total_images} image(s) to build");

    if dry_run {
        println!("Dry-run mode - showing commands without executing:\n");
    }

    for (image_type, img_config) in images_to_build {
        let image_name = img_config
            .image
            .as_ref()
            .expect("image field should be validated by mapper");
        println!("\nBuilding image: {image_type}");
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
            output: BuildOutput::Push,
        });

        match result {
            Ok(_) => {
                println!("  Successfully built {image_type}");
                success_count += 1;
            }
            Err(e) => {
                eprintln!("  Failed to build {image_type}: {e}");
                fail_count += 1;
            }
        }
    }

    println!("\nBuild Summary:");
    println!("  Total images: {total_images}");
    println!("  Successful: {success_count}");
    println!("  Failed: {fail_count}");

    if fail_count > 0 {
        Err(Box::new(std::io::Error::other(format!(
            "Build failed for {fail_count} image(s)"
        ))) as Box<dyn Error + Send>)
    } else {
        println!("\nAll images built successfully!");
        Ok(())
    }
}

struct PushBuildResult {
    registry: String,
    image: String,
    blob_image: String,
}

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
    println!("Building images for push with tag: {tag}");

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

    if !dry_run {
        if let Err(e) = BuildxBuilder::check_docker() {
            eprintln!("Error: {e}");
            return Err(e);
        }
        if let Err(e) = BuildxBuilder::check_buildx() {
            eprintln!("Error: {e}");
            return Err(e);
        }
    }

    println!("Loading configuration from: {config_path}");
    let build_config = read_build_config(config_path.to_string())?;

    let registry = build_config.registry.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No registry configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    let images = build_config.images.as_ref().ok_or_else(|| {
        Box::new(std::io::Error::other("No images configured in cyan.yaml"))
            as Box<dyn Error + Send>
    })?;

    println!("  Registry: {registry}");

    let platforms = resolve_platforms(
        platform,
        build_config.platforms.as_ref(),
        get_current_platform,
    );

    let mut buildx = BuildxBuilder::new();
    if let Some(b) = builder {
        buildx = buildx.with_builder(b);
        println!("  Using builder: {b}");
    }

    if dry_run {
        println!("Dry-run mode - showing commands without executing:\n");
    }

    let mut result = PushBuildResult {
        registry: registry.clone(),
        image: String::new(),
        blob_image: String::new(),
    };

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

        println!("\nBuilding image: {image_type}");
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
            output: BuildOutput::Push,
        });

        if let Err(e) = build_result {
            eprintln!("  Failed to build {image_type}: {e}");
            return Err(e);
        }

        println!("  Successfully built {image_type}");

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

fn get_current_platform() -> Vec<String> {
    let current = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let platform_str = match (os, current) {
        ("linux", "x86_64") => "linux/amd64",
        ("linux", "aarch64") => "linux/arm64",
        ("macos", "x86_64") => "linux/amd64",
        ("macos", "aarch64") => "linux/arm64",
        ("windows", "x86_64") => "linux/amd64",
        _ => "linux/amd64",
    };
    vec![platform_str.to_string()]
}

fn resolve_platforms<F>(
    cli_platform: Option<&str>,
    config_platforms: Option<&Vec<String>>,
    get_current: F,
) -> Vec<String>
where
    F: FnOnce() -> Vec<String>,
{
    if let Some(p) = cli_platform {
        p.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    } else if let Some(config_platforms) = config_platforms {
        if config_platforms.is_empty() {
            get_current()
        } else {
            config_platforms.clone()
        }
    } else {
        get_current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_platforms_cli_override() {
        let result = resolve_platforms(
            Some("linux/amd64,linux/arm64"),
            Some(&vec!["linux/386".to_string()]),
            || vec!["fallback".to_string()],
        );
        assert_eq!(result, vec!["linux/amd64", "linux/arm64"]);
    }

    #[test]
    fn test_resolve_platforms_cli_single() {
        let result = resolve_platforms(Some("linux/amd64"), None, || vec!["fallback".to_string()]);
        assert_eq!(result, vec!["linux/amd64"]);
    }

    #[test]
    fn test_resolve_platforms_cli_with_spaces() {
        let result = resolve_platforms(Some("linux/amd64 , linux/arm64"), None, || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["linux/amd64", "linux/arm64"]);
    }

    #[test]
    fn test_resolve_platforms_config_platforms() {
        let config_platforms = vec!["linux/amd64".to_string(), "linux/arm64".to_string()];
        let result = resolve_platforms(None, Some(&config_platforms), || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["linux/amd64", "linux/arm64"]);
    }

    #[test]
    fn test_resolve_platforms_empty_config_falls_back() {
        let config_platforms: Vec<String> = vec![];
        let result = resolve_platforms(None, Some(&config_platforms), || {
            vec!["linux/current".to_string()]
        });
        assert_eq!(result, vec!["linux/current"]);
    }

    #[test]
    fn test_resolve_platforms_no_config_falls_back() {
        let result = resolve_platforms(None, None, || vec!["linux/current".to_string()]);
        assert_eq!(result, vec!["linux/current"]);
    }

    #[test]
    fn test_resolve_platforms_priority_order() {
        let config_platforms = vec!["config-platform".to_string()];

        let result = resolve_platforms(Some("cli-platform"), Some(&config_platforms), || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["cli-platform"]);

        let result = resolve_platforms(None, Some(&config_platforms), || {
            vec!["fallback".to_string()]
        });
        assert_eq!(result, vec!["config-platform"]);

        let result = resolve_platforms(None, None, || vec!["fallback".to_string()]);
        assert_eq!(result, vec!["fallback"]);
    }
}
