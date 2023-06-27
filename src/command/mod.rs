use crate::config::ProfileConfig;
use crate::layer;
use anyhow::{Context, Ok, Result, bail};
use helixlauncher_core::launch::instance;

use crate::layer::Profile;

pub async fn run(name: Option<String>, mut config: ProfileConfig) -> Result<()> {
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

    profile.name = name.clone();
    profile.clone().apply_to_all_variants(
        |layers, _| {
            println!("{layers:?}");
            Ok(())
        },
        "".to_string(),
    )?;

    profile.clone().run(profile_dir.join(&profile.name))?;
    println!("Successfully ran {}", name);
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
