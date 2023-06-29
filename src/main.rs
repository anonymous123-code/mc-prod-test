use std::{env, path::PathBuf};

mod command;
mod config;
pub mod layer;

use anyhow::{Context, Ok, Result};
use clap::{Parser, Subcommand};
use helixlauncher_core::auth::account;

#[tokio::main]
async fn main() -> Result<()> {
    let args = McProdTest::parse();
    let profile_dir = args.profile_dir.map_or_else(||env::current_dir().context("No working directory provided by environment, provide a profile directory using --profile_dir"), |it|->Result<PathBuf>{Ok(it)})?;

    let mut profile_config =
        config::ProfileConfig::read_or_create(profile_dir.join("profiles.json"))?;
    let mut accounts = account::get_accounts(&profile_dir.join("accounts.json"))?;
    return match args.subcommand {
        Commands::Profile {
            command: ProfileCommands::Run { name },
        } => command::profile::run(name, profile_config).await,
        Commands::Profile {
            command: ProfileCommands::Create { name },
        } => command::profile::create(name, &mut profile_config).await,
        Commands::Profile {
            command: ProfileCommands::Switch { name },
        } => command::profile::switch(name, &mut profile_config).await,
        Commands::Account {
            command: AccountCommands::List,
        } => command::account::list(&accounts).await,
        Commands::Account {
            command: AccountCommands::Add,
        } => command::account::add(&mut accounts).await,
        Commands::Account {
            command: AccountCommands::Switch { name },
        } => command::account::switch(name, &mut accounts).await,
        // Commands::Login => {todo!()}
        Commands::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schemars::schema_for!(config::ProfileConfig))
                    .unwrap()
            );
            Ok(())
        }
    };
}

#[derive(Parser)]
#[clap(infer_subcommands = true)]
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
    /// Commands related to profiles
    Profile {
        #[clap(subcommand)]
        command: ProfileCommands,
    },
    /// Commands related to account management
    Account {
        #[clap(subcommand)]
        command: AccountCommands,
    },
    /// Prints a json schema for the profile json to stdout
    Schema,
}

#[derive(Subcommand)]
enum AccountCommands {
    Switch { name: Option<String> },
    Add,
    List,
}

#[derive(Subcommand)]
#[clap(infer_subcommands = false)] // Have to do this and manually specify aliases because of the select-switch conflict. See https://github.com/clap-rs/clap/issues/4815
enum ProfileCommands {
    /// Run the given profile, prompts if none is given
    #[clap(alias("r"))]
    Run {
        /// The name of the profile which is ran. This will take precedence over the selected profile
        name: Option<String>,
    },
    /// Create a new profile
    #[clap(alias("add"), alias("new"), alias("a"), alias("n"), alias("c"))]
    Create { name: Option<String> },
    /// Select the default profile executed when run is used
    #[clap(alias("select"), alias("s"))]
    Switch { name: Option<String> },
}
