use std::{env, path::PathBuf};

mod command;
mod config;
pub mod layer;

use clap::{Parser, Subcommand};

#[tokio::main]
async fn main() {
    let args = McProdTest::parse();
    let profile_dir = args.profile_dir.unwrap_or_else(||env::current_dir().expect("No working directory provided by environment, provide a profile directory using --profile_dir"));

    let mut config = config::ProfileConfig::read_or_create(profile_dir.join("profiles.json"));
    match args.subcommand {
        Commands::Run { name } => command::run(name, config).await,
        Commands::Create { name } => command::create(name, &mut config).await,
        Commands::Add {} => {}
        Commands::Switch { name } => command::switch(name, &mut config).await,
        Commands::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schemars::schema_for!(config::ProfileConfig))
                    .unwrap()
            )
        }
    }
}

#[derive(Parser)]
struct McProdTest {
    #[clap(subcommand)]
    pub subcommand: Commands,
    #[clap(long, short)]
    pub threads: Option<usize>,
    #[arg(default_value_t = 3)]
    #[clap(long, short)]
    pub max_running_profiles: usize,
    #[clap(long, short)]
    pub profile_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    Run { name: Option<String> },
    Create { name: Option<String> },
    Add {},
    Switch { name: Option<String> },
    Schema,
}
