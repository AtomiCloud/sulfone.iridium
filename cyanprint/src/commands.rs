use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about = "Next-generation templating platform", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(
        short,
        long,
        value_name = "REGISTRY_ENDPOINT",
        default_value = "https://api.zinc.sulfone.raichu.cluster.atomi.cloud",
        env = "CYANPRINT_REGISTRY"
    )]
    pub registry: String,

    #[arg(
        short = 'd',
        long,
        help = "Enable debug output",
        default_value_t = false
    )]
    pub debug: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(alias = "b", about = "Build Docker images using buildx")]
    Build {
        #[arg(value_name = "TAG")]
        tag: String,

        #[arg(short, long, default_value = "cyan.yaml")]
        config: String,

        #[arg(short, long, help = "Target platforms (comma-separated)")]
        platform: Option<String>,

        #[arg(short, long, help = "Buildx builder to use")]
        builder: Option<String>,

        #[arg(long, help = "Don't use cache")]
        no_cache: bool,

        #[arg(long, help = "Show commands without executing")]
        dry_run: bool,

        #[arg(long, default_value = ".", help = "Working directory for the build")]
        folder: String,
    },

    #[command(alias = "p", about = "Publish a CyanPrint artifact")]
    Push(PushArgs),

    #[command(alias = "c", about = "Create a project from a CyanPrint template")]
    Create {
        template_ref: String,

        path: Option<String>,

        #[arg(
            short,
            long,
            value_name = "COORDINATOR_ENDPOINT",
            default_value = "http://coord.cyanprint.dev:9000",
            env = "CYANPRINT_COORDINATOR"
        )]
        coordinator_endpoint: String,
    },

    #[command(
        alias = "u",
        about = "Update all templates in a project to their latest versions"
    )]
    Update {
        #[arg(default_value = ".")]
        path: String,

        #[arg(
            short,
            long,
            value_name = "COORDINATOR_ENDPOINT",
            default_value = "http://coord.cyanprint.dev:9000",
            env = "CYANPRINT_COORDINATOR"
        )]
        coordinator_endpoint: String,

        #[arg(
            short,
            long,
            help = "Enable interactive mode to select specific versions",
            default_value_t = false
        )]
        interactive: bool,

        #[arg(
            long,
            help = "Force update even if git is dirty (skip confirmation prompt)"
        )]
        force: bool,
    },

    #[command(
        alias = "t",
        about = "Try a local template without publishing to the registry"
    )]
    Try {
        template_path: String,

        output_path: String,

        #[arg(long, help = "Enable dev mode for local development")]
        dev: bool,

        #[arg(
            long,
            help = "Preserve template container and blob volume after execution"
        )]
        keep_containers: bool,

        #[arg(long, help = "Skip automatic daemon start")]
        disable_daemon_autostart: bool,

        #[arg(
            short,
            long,
            value_name = "COORDINATOR_ENDPOINT",
            default_value = "http://coord.cyanprint.dev:9000",
            env = "CYANPRINT_COORDINATOR"
        )]
        coordinator_endpoint: String,
    },

    #[command(alias = "d", about = "Manage the CyanPrint Coordinator daemon")]
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
}

#[derive(Subcommand)]
pub enum DaemonCommands {
    #[command(about = "Start the CyanPrint Coordinator daemon")]
    Start {
        #[arg(value_name = "COORDINATOR_VERSION", default_value = "latest")]
        version: String,

        #[arg(
            short,
            long,
            value_name = "PORT",
            help = "Port to host the daemon container",
            default_value = "9000"
        )]
        port: u16,

        #[arg(
            short,
            long,
            value_name = "REGISTRY_ENDPOINT",
            default_value = "https://api.zinc.sulfone.raichu.cluster.atomi.cloud",
            help = "Registry endpoint for the coordinator to use",
            env = "CYANPRINT_REGISTRY"
        )]
        registry: Option<String>,
    },

    #[command(about = "Stop the CyanPrint Coordinator daemon and cleanup")]
    Stop {
        #[arg(
            short,
            long,
            value_name = "PORT",
            help = "Port where daemon is running",
            default_value = "9000"
        )]
        port: u16,
    },
}

#[derive(Debug, Args)]
pub struct PushArgs {
    #[command(subcommand)]
    pub commands: PushCommands,

    #[arg(short, long, value_name = "CONFIG_PATH", default_value = "cyan.yaml")]
    pub config: String,

    #[arg(
        short,
        long,
        value_name = "PUBLISH_MESSAGE",
        default_value = "No description"
    )]
    pub message: String,

    #[arg(short, long, value_name = "API_TOKEN", env = "CYAN_TOKEN")]
    pub token: String,

    #[arg(long, help = "Target platforms for build (comma-separated)")]
    pub platform: Option<String>,

    #[arg(long, help = "Buildx builder to use for build")]
    pub builder: Option<String>,

    #[arg(long, help = "Don't use cache during build")]
    pub no_cache: bool,

    #[arg(long, help = "Show build commands without executing")]
    pub dry_run: bool,

    #[arg(
        long,
        default_value = ".",
        help = "Working directory for the build (used with --build)"
    )]
    pub folder: String,
}

#[derive(Debug, Subcommand)]
pub enum PushCommands {
    Template {
        #[arg(
            long,
            help = "Build with tag before pushing (requires build config in cyan.yaml)"
        )]
        build: Option<String>,

        blob_image: Option<String>,

        blob_tag: Option<String>,

        template_image: Option<String>,

        template_tag: Option<String>,
    },
    #[command(about = "Push a template group (meta-template that combines other templates)")]
    Group,
    Plugin {
        #[arg(
            long,
            help = "Build with tag before pushing (requires build config in cyan.yaml)"
        )]
        build: Option<String>,

        image: Option<String>,

        tag: Option<String>,
    },
    Processor {
        #[arg(
            long,
            help = "Build with tag before pushing (requires build config in cyan.yaml)"
        )]
        build: Option<String>,

        image: Option<String>,

        tag: Option<String>,
    },
    #[command(about = "Push a conflict resolver artifact")]
    Resolver {
        #[arg(
            long,
            help = "Build with tag before pushing (requires build config in cyan.yaml)"
        )]
        build: Option<String>,

        image: Option<String>,

        tag: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_start_default_values() {
        let cli = Cli::try_parse_from(["cyanprint", "daemon", "start"]);
        assert!(cli.is_ok());
        if let Commands::Daemon { command } = cli.unwrap().command {
            if let DaemonCommands::Start {
                version,
                port,
                registry: _,
            } = command
            {
                assert_eq!(version, "latest");
                assert_eq!(port, 9000);
                // registry is ignored as it depends on CYANPRINT_REGISTRY env var
            } else {
                panic!("Expected DaemonCommands::Start");
            }
        } else {
            panic!("Expected Commands::Daemon");
        }
    }

    #[test]
    fn test_daemon_start_custom_values() {
        let cli = Cli::try_parse_from([
            "cyanprint",
            "daemon",
            "start",
            "1.5.0",
            "--port",
            "8080",
            "--registry",
            "https://custom.com",
        ]);
        assert!(cli.is_ok());
        if let Commands::Daemon { command } = cli.unwrap().command {
            if let DaemonCommands::Start {
                version,
                port,
                registry,
            } = command
            {
                assert_eq!(version, "1.5.0");
                assert_eq!(port, 8080);
                assert_eq!(registry, Some("https://custom.com".to_string()));
            } else {
                panic!("Expected DaemonCommands::Start");
            }
        } else {
            panic!("Expected Commands::Daemon");
        }
    }

    #[test]
    fn test_daemon_stop_default_port() {
        let cli = Cli::try_parse_from(["cyanprint", "daemon", "stop"]);
        assert!(cli.is_ok());
        if let Commands::Daemon { command } = cli.unwrap().command {
            if let DaemonCommands::Stop { port } = command {
                assert_eq!(port, 9000);
            } else {
                panic!("Expected DaemonCommands::Stop");
            }
        } else {
            panic!("Expected Commands::Daemon");
        }
    }

    #[test]
    fn test_daemon_stop_custom_port() {
        let cli = Cli::try_parse_from(["cyanprint", "daemon", "stop", "--port", "8080"]);
        assert!(cli.is_ok());
        if let Commands::Daemon { command } = cli.unwrap().command {
            if let DaemonCommands::Stop { port } = command {
                assert_eq!(port, 8080);
            } else {
                panic!("Expected DaemonCommands::Stop");
            }
        } else {
            panic!("Expected Commands::Daemon");
        }
    }

    #[test]
    fn test_daemon_requires_subcommand() {
        let result = Cli::try_parse_from(["cyanprint", "daemon"]);
        assert!(result.is_err(), "daemon without subcommand should fail");
    }

    #[test]
    fn test_build_command_with_folder() {
        let cli = Cli::try_parse_from(["cyanprint", "build", "v1", "--folder", "./e2e/plugin2"]);
        assert!(cli.is_ok());
        if let Commands::Build {
            tag,
            config,
            platform,
            builder,
            no_cache,
            dry_run,
            folder,
        } = cli.unwrap().command
        {
            assert_eq!(tag, "v1");
            assert_eq!(config, "cyan.yaml");
            assert_eq!(folder, "./e2e/plugin2");
            assert!(platform.is_none());
            assert!(builder.is_none());
            assert!(!no_cache);
            assert!(!dry_run);
        } else {
            panic!("Expected Commands::Build");
        }
    }

    #[test]
    fn test_build_command_default_folder() {
        let cli = Cli::try_parse_from(["cyanprint", "build", "v1"]);
        assert!(cli.is_ok());
        if let Commands::Build { folder, .. } = cli.unwrap().command {
            assert_eq!(folder, ".");
        } else {
            panic!("Expected Commands::Build");
        }
    }
}
