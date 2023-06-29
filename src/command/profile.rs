use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use crate::layer;
use crate::{config::ProfileConfig, layer::PreparedVariant};
use anyhow::{bail, Context, Ok, Result, ensure};
use futures::future::try_join_all;
use helixlauncher_core::auth::account::AccountConfig;
use helixlauncher_core::launch::instance;
use indicatif::ProgressBar;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::layer::Profile;

pub async fn run(
    name: Option<String>,
    mut config: ProfileConfig,
    account_config: AccountConfig,
) -> Result<()> {
    let profile_dir = config.path.parent().unwrap();
    let name = match name {
        Some(name) => name,
        None => match config.active_config {
            Some(name) => name,
            None => {
                let options: Vec<String> = config.profiles.clone().into_keys().collect();
                let index = match dialoguer::FuzzySelect::new()
                    .with_prompt("Select profile")
                    .items(&options)
                    .interact_opt()
                    .context("Error while prompting profile name")?
                {
                    Some(index) => index,
                    _ => return Ok(()),
                };
                options.get(index).unwrap().to_owned()
            }
        },
    };
    let profile = config
        .profiles
        .get_mut(&name)
        .context("Profile does not exist")?;
    let variants = profile.clone().get_variants(name.clone() + "_");
    let setup_bar = Arc::new(ProgressBar::new(variants.len().try_into().unwrap()));
    profile.name = name.clone();
    let variants = variants.into_iter().map(|it: layer::Variant| {
        let path = profile_dir.join(&profile.name);
        let setup_bar = setup_bar.clone();
        async move {
            let result: PreparedVariant = it.setup(path).await?;
            setup_bar.inc(1);
            Ok::<PreparedVariant>(result)
        }
    });
    let variants = futures::future::try_join_all(variants).await?;
    setup_bar.finish();

    let prepare_bar = Arc::new(ProgressBar::new(variants.len().try_into().unwrap()));
    prepare_bar.enable_steady_tick(Duration::from_secs(1));
    profile.name = name.clone();
    let variants = variants.into_iter().map(|it| {
        let prepare_bar = prepare_bar.clone();
        let account_config = account_config.clone();
        async move {
            let result = it.run(account_config).await?;
            prepare_bar.inc(1);
            Ok(result)
        }
    });
    let variants = futures::future::try_join_all(variants).await?;
    prepare_bar.finish();

    let launch_bar = Arc::new(ProgressBar::new(variants.len().try_into().unwrap()));
    launch_bar.enable_steady_tick(Duration::from_secs(1));
    for variant in variants {
        let mut variant = variant;
        variant.stderr = Stdio::piped();
        variant.stdout = Stdio::piped();
        let mut child = variant.launch().await?;
        let stderr = tokio::task::spawn({
            let launch_bar = launch_bar.clone();
            let stderr = child.stderr.take().unwrap();
            async move {
                let mut stderr_reader = BufReader::new(stderr).lines();
                while let Some(line) = stderr_reader.next_line().await? {
                    launch_bar.suspend(|| println!("{line}"))
                }
                Ok(())
            }
        });
        let stdout = tokio::task::spawn({
            let launch_bar = launch_bar.clone();
            let stdout = child.stdout.take().unwrap();
            async move {
                let mut stderr_reader = BufReader::new(stdout).lines();
                while let Some(line) = stderr_reader.next_line().await? {
                    launch_bar.suspend(|| eprintln!("{line}"))
                }
                Ok(())
            }
        });
        let run = tokio::task::spawn({
            async move {
                ensure!(child.wait().await?.success(), "Error while running instance");
                Ok(())
            }
        });
        try_join_all(vec![run, stderr, stdout]).await?;
        launch_bar.inc(1);
    }
    launch_bar.finish();
    return Ok(());
}

pub async fn create(name: Option<String>, config: &mut ProfileConfig) -> Result<()> {
    let name = match name {
        Some(name) => name,
        None => dialoguer::Input::new()
            .with_prompt("Enter profile name")
            .interact_text()
            .context("Error while rompting profile name")?,
    };
    if config.profiles.contains_key(&name) {
        bail!("Profile already exists");
    }
    let mut new_profile = Profile {
        layers: vec![],
        name: name.clone(),
    };
    if match dialoguer::Confirm::new()
        .with_prompt("Generate minecraft and mod loader layers?")
        .interact_opt()
    {
        core::result::Result::Ok(Some(value)) => value,
        core::result::Result::Ok(None) => return Ok(()),
        Err(err) => {
            eprintln!("Unable to prompt wether to generate default layers: {err}, assuming no");
            false
        }
    } {
        let mut instance = layer::Layer::Instance {
            version: dialoguer::Input::new()
                .with_prompt("Minecraft Version")
                .interact_text()
                .context("Unable to prompt minecraft version")?,
            loader: instance::Modloader::Vanilla,
            loader_version: None,
        };
        let modloaders = [
            instance::Modloader::Vanilla,
            instance::Modloader::Quilt,
            instance::Modloader::Fabric,
            instance::Modloader::Forge,
        ];
        match dialoguer::FuzzySelect::new()
            .items(&modloaders)
            .interact_opt()
            .context("Unable to prompt mod loader")?
        {
            Some(index) => match modloaders[index] {
                instance::Modloader::Vanilla => {}
                loader => {
                    if let layer::Layer::Instance { version, .. } = instance {
                        instance = layer::Layer::Instance {
                            version,
                            loader,
                            loader_version: Some(
                                dialoguer::Input::new()
                                    .with_prompt(format!("Select {loader} version"))
                                    .interact_text()
                                    .context("Unable to prompt loader version")?,
                            ),
                        };
                    } else {
                        panic!("Static, should never happen!")
                    }
                }
            },
            None => return Ok(()),
        }
        new_profile.layers.push(instance);
    }
    config.profiles.insert(name.clone(), new_profile);
    config.safe()?;
    println!("Profile {} was created", name);
    Ok(())
}

pub async fn switch(name: Option<String>, config: &mut ProfileConfig) -> Result<()> {
    match name {
        Some(string) => {
            if config.profiles.contains_key(&string) {
                config.active_config = Some(string);
                config.safe()?;
            } else {
                bail!("This profile does not exist");
            }
        }
        None => {
            let options: Vec<String> = config.profiles.clone().into_keys().collect();
            let dialog = dialoguer::FuzzySelect::new()
                .with_prompt("Select profile")
                .item("Clear selected profile, will require re-selecting or manually specifying the profile each run")
                .items(&options)
                .default(0)
                .interact_opt().context("Profile selection doesnt work")?;

            match dialog {
                Some(0) => {
                    config.active_config = None;
                    config.safe()?;
                }
                Some(index) => {
                    config.active_config = Some(options.get(index - 1).unwrap().to_owned());
                    config.safe()?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
