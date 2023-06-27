use crate::config::ProfileConfig;
use crate::layer;
use helixlauncher_core::launch::instance;

use crate::layer::Profile;

pub async fn run(name: Option<String>, mut config: ProfileConfig) {
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
                    .expect("Error while interactively prompting profile name: ")
                {
                    Some(index) => index,
                    _ => return,
                };
                options.get(index).unwrap().to_owned()
            }
        },
    };
    if config.profiles.contains_key(&name) {
        let profile = config.profiles.get_mut(&name).unwrap();

        profile.name = name.clone();
        profile
            .clone()
            .apply_to_all_variants(|layers, _| println!("{layers:?}"), "".to_string());

        profile.clone().run(profile_dir.join(&profile.name));
        println!("Successfully ran {}", name)
    } else {
        panic!("This profile does not exist")
    }
}

pub async fn create(name: Option<String>, config: &mut ProfileConfig) {
    let name = match name {
        Some(name) => name,
        None => dialoguer::Input::new()
            .with_prompt("Enter profile name")
            .interact_text()
            .expect("Error while interactively prompting profile name: "),
    };
    if config.profiles.contains_key(&name) {
        panic!("Profile already exists")
    }
    let mut new_profile = Profile {
        layers: vec![],
        name: name.clone(),
    };
    if match dialoguer::Confirm::new()
        .with_prompt("Generate minecraft and mod loader layers?")
        .interact_opt()
    {
        Ok(Some(value)) => value,
        Ok(None) => return,
        Err(err) => {
            eprintln!("Unable to prompt wether to generate default layers: {err}, assuming no");
            false
        }
    } {
        let mut instance = layer::Layer::Instance {
            version: dialoguer::Input::new()
                .with_prompt("Minecraft Version")
                .interact_text()
                .expect("Unable to prompt minecraft version"),
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
        {
            Ok(Some(index)) => match modloaders[index] {
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
                                    .unwrap(),
                            ),
                        };
                    } else {
                        panic!("Static, should never happen!")
                    }
                }
            },
            Ok(None) => return,
            Err(err) => Err(err).expect("Unable to prompt mod loader"),
        }
        new_profile.layers.push(instance);
    }
    config.profiles.insert(name.clone(), new_profile);
    config.safe();
    println!("Profile {} was created", name);
}

pub async fn switch(name: Option<String>, config: &mut ProfileConfig) {
    match name {
        Some(string) => {
            if config.profiles.contains_key(&string) {
                config.active_config = Some(string);
                config.safe();
            } else {
                panic!("This profile does not exist")
            }
        }
        None => {
            let options: Vec<String> = config.profiles.clone().into_keys().collect();
            let dialog = dialoguer::FuzzySelect::new()
                .with_prompt("Select profile")
                .item("Clear selected profile, will require re-selecting or manually specifying the profile each run")
                .items(&options)
                .default(0)
                .interact_opt().expect("Profile selection doesnt work");

            match dialog {
                Some(0) => {
                    config.active_config = None;
                    config.safe();
                }
                Some(index) => {
                    config.active_config = Some(options.get(index - 1).unwrap().to_owned());
                    config.safe();
                }
                _ => {}
            }
        }
    }
}
