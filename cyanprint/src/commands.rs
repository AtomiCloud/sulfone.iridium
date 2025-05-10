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
        default_value = "https://api.zinc.sulfone.raichu.cluster.atomi.cloud"
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
            default_value = "http://coord.cyanprint.dev:9000"
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
            default_value = "http://coord.cyanprint.dev:9000"
        )]
        coordinator_endpoint: String,
    },

    #[command(alias = "d", about = "Starts the CyanPrint Coordinator locally daemon")]
    Daemon {
        #[arg(value_name = "COORDINATOR_VERSION", default_value = "latest")]
        version: String,

        #[arg(value_name = "COORDINATOR_ARCH")]
        architecture: Option<String>,
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
}

#[derive(Debug, Subcommand)]
pub enum PushCommands {
    Template {
        blob_image: String,

        blob_tag: String,

        template_image: String,

        template_tag: String,
    },
    Plugin {
        image: String,

        tag: String,
    },
    Processor {
        image: String,

        tag: String,
    },
}
