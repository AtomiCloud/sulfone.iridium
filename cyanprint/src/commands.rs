use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about = "Next-generation templating platform", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, value_name = "REGISTRY_ENDPOINT", default_value = "https://api.zinc.sulfone.raichu.cluster.atomi.cloud")]
    pub registry: String,

}

#[derive(Subcommand)]
pub enum Commands {
    Push(PushArgs),
    Run {
        template_ref: String,

        path: Option<String>,

        #[arg(short, long, value_name = "COORDINATOR_ENDPOINT", default_value = "http://coord.cyanprint.dev:9000")]
        coordinator_endpoint: String,
    },
}

#[derive(Debug, Args)]
pub struct PushArgs {
    #[command(subcommand)]
    pub commands: PushCommands,
}

#[derive(Debug, Subcommand)]
pub enum PushCommands {
    Template {
        #[arg(short, long, value_name = "CONFIG_PATH", default_value = "cyan.yaml")]
        config: String,

        #[arg(short, long, value_name = "PUBLISH_MESSAGE", default_value = "No description")]
        message: String,

        #[arg(short, long, value_name = "API_TOKEN")]
        token: String,

        blob_image: String,

        blob_tag: String,

        template_image: String,

        template_tag: String,
    },
    Plugin {
        #[arg(short, long, value_name = "CONFIG_PATH", default_value = "cyan.yaml")]
        config: String,

        #[arg(short, long, value_name = "PUBLISH_MESSAGE", default_value = "No description")]
        message: String,

        #[arg(short, long, value_name = "API_TOKEN")]
        token: String,

        image: String,

        tag: String,
    },
    Processor {
        #[arg(short, long, value_name = "CONFIG_PATH", default_value = "cyan.yaml")]
        config: String,

        #[arg(short, long, value_name = "PUBLISH_MESSAGE", default_value = "No description")]
        message: String,

        #[arg(short, long, value_name = "API_TOKEN")]
        token: String,

        image: String,

        tag: String,
    },
}
