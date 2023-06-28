use anyhow::{Context, Ok, Result, bail};
use either::Either;
use helixlauncher_core::{launch::{instance, prepared::{LaunchOptions, prepare_launch}, asset::merge_components}, config::Config};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::process::Child;
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
pub struct Profile {
    #[serde(skip)]
    pub name: String,
    pub layers: Vec<Layer>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(remote = "instance::Modloader")]
#[serde(rename_all = "lowercase")]
pub enum ModloaderDef {
    Quilt,
    Fabric,
    Forge,
    Vanilla,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    DeleteDirectory(PathBuf),
    Instance {
        version: String,
        #[serde(with = "ModloaderDef")]
        loader: instance::Modloader,
        loader_version: Option<String>,
    },
    DirectoryOverlay {
        source: PathBuf,
    },
    ModrinthPack {
        id: String,
        version: Option<String>,
    },
    LaunchClient {
        account_name: Option<String>,
        world_name: Option<String>,
    },
    ExecuteCommand(String),
    Variants(Vec<Layer>),
    IfPresent {
        check_for: ResolvedLayer,
        include: Box<Layer>,
    },
    IfNotPresent {
        check_for: ResolvedLayer,
        include: Box<Layer>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedLayer {
    DeleteDirectory(PathBuf),
    Instance {
        version: String,
        #[serde(with = "ModloaderDef")]
        loader: instance::Modloader,
        loader_version: Option<String>,
    },
    DirectoryOverlay {
        source: PathBuf,
    },
    ModrinthPack {
        id: String,
        version: Option<String>,
    },
    ExecuteCommand(String),
    LaunchClient {
        account_name: Option<String>,
        world_name: Option<String>,
    },
}

impl Layer {
    fn resolve(self, previous_layers: &[ResolvedLayer]) -> Vec<ResolvedLayer> {
        match self {
            Self::DeleteDirectory(path) => vec![ResolvedLayer::DeleteDirectory(path)],
            Self::Instance {
                version,
                loader,
                loader_version,
            } => vec![ResolvedLayer::Instance {
                version,
                loader,
                loader_version,
            }],
            Self::DirectoryOverlay { source } => {
                vec![ResolvedLayer::DirectoryOverlay { source: source }]
            }
            Self::ModrinthPack { id, version } => vec![ResolvedLayer::ModrinthPack { id, version }],
            Self::ExecuteCommand(command) => vec![ResolvedLayer::ExecuteCommand(command)],
            Self::LaunchClient {
                account_name,
                world_name,
            } => vec![ResolvedLayer::LaunchClient {
                account_name,
                world_name,
            }],
            Self::Variants(variants) => variants
                .into_iter()
                .flat_map(|e| e.resolve(previous_layers))
                .collect(),
            Self::IfPresent { check_for, include } => {
                if previous_layers.contains(&check_for) {
                    include.resolve(previous_layers)
                } else {
                    vec![]
                }
            }
            Self::IfNotPresent { check_for, include } => {
                if !previous_layers.contains(&check_for) {
                    include.resolve(previous_layers)
                } else {
                    vec![]
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Variant {
    layers: Vec<ResolvedLayer>,
    name: String,
}

pub struct PreparedVariant {
    instance: instance::Instance,
    launch_options: LaunchOptions
}

impl PreparedVariant {
    pub async fn run(self) -> Result<Child> {
        let config = Config::new_with_data_dir("dev.helixlauncher.HelixLauncher", "HelixLauncher", self.instance.path.clone().parent().unwrap().join(".helix_config"))?;
        let merged_components = merge_components(&config, &self.instance.config.components).await?;
        Ok(prepare_launch(&config, &self.instance, &merged_components, self.launch_options).await?.launch(true).await?)
    }
}

impl Variant {
    pub fn prepare(self, base_directory: PathBuf) -> Result<PreparedVariant> {
        let mut instance = Either::Right(base_directory.join(self.name));
        let mut launch_options = LaunchOptions::default();
        for resolved in self.layers {
            match resolved
                .apply(&instance, launch_options)
                .context("Error while preparing profile")?
            {
                (Some(new_instance), new_launch_options) => {
                    instance = Either::Left(new_instance);
                    launch_options = new_launch_options;
                }
                (None, new_launch_options) => {
                    launch_options = new_launch_options;
                }
            }
        }
        Ok(PreparedVariant { instance: instance.left().context("No instance was generated by profile")?, launch_options })
    }
}

impl Profile {
    pub fn get_variants(self, name: String) -> Vec<Variant> {
        Self::get_variants_rec(&[], &mut self.layers.into(), name)
    }

    fn get_variants_rec(
        prev: &[ResolvedLayer],
        coming: &mut VecDeque<Layer>,
        name: String,
    ) -> Vec<Variant> {
        if coming.len() == 0 {
            return vec![Variant {
                layers: prev.to_vec(),
                name,
            }];
        }

        let mut resolved = coming.pop_front().unwrap().resolve(prev);
        while resolved.len() == 0 && coming.len() != 0 {
            resolved = coming.pop_front().unwrap().resolve(prev);
        }

        resolved
            .into_iter()
            .enumerate()
            .flat_map(|(index, resolved_layer)| {
                Self::get_variants_rec(
                    &[prev, &[resolved_layer]].concat(),
                    &mut coming.clone(),
                    format!("{name}{index:02}"),
                )
            })
            .collect()
    }
}

impl ResolvedLayer {
    pub fn apply(
        &self,
        instance: &Either<instance::Instance, PathBuf>,
        launch_options: LaunchOptions,
    ) -> Result<(Option<instance::Instance>, LaunchOptions)> {
        let path = match instance {
            Either::Left(instance) => &instance.path,
            Either::Right(path) => path,
        };
        return match self {
            Self::DeleteDirectory(target) => {
                let full_target_path = path.join(target);
                if full_target_path.is_dir() {
                    fs::remove_dir_all(path)?;
                }
                Ok((None, launch_options))
            }
            Self::Instance {
                version,
                loader,
                loader_version,
            } => Ok((
                Some(
                    instance::Instance::new(
                        path.file_name().unwrap().to_string_lossy().to_string(),
                        version.clone(),
                        instance::InstanceLaunchConfig::default(),
                        path.parent().unwrap(),
                        *loader,
                        loader_version.clone(),
                    )
                    .context("Error while trying to create instance")?,
                ),
                launch_options,
            )),
            Self::DirectoryOverlay { source } => {
                copy_dir_all(path.join(source), path)?;
                Ok((None, launch_options))
            }
            Self::ExecuteCommand(cmd) => {
                if run_cmd(cmd, &path)?.status.success() {
                    return Ok((None, launch_options));
                }
                bail!("Command `{cmd}` didnt ran propely");
            }
            Self::ModrinthPack { id: _, version: _ } => {
                todo!("Modpack support")
            }
            Self::LaunchClient {
                account_name: _,
                world_name: _,
            } => {
                todo!();
                /*
                match instance {
                    Either::First(instance) => {
                        join!(prepared::prepare_launch(Config::new(appid, name), instance, components, launch_options).await);
                        Ok(None)
                    }
                    Either::Second(_) => Err()
                }*/
            }
        };

        fn run_cmd(cmd: &String, path: &PathBuf) -> Result<Output> {
            if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .current_dir(path)
                    .args(["/C", cmd])
                    .output()
                    .context("failed to execute process")
            } else {
                Command::new("sh")
                    .current_dir(path)
                    .arg("-c")
                    .arg(cmd)
                    .output()
                    .context("failed to execute process")
            }
        }
        fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
            fs::create_dir_all(&dst)?;
            for entry in fs::read_dir(src)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                if ty.is_dir() {
                    copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
                } else {
                    fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
                }
            }
            Ok(())
        }
    }
}
