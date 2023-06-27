use helixlauncher_core::launch::instance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    error::Error,
    fmt::Display,
    fs, io,
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

impl Profile {
    pub fn apply_to_all_variants<F: Fn(&[ResolvedLayer], String) -> B, B>(
        self,
        apply: F,
        name: String,
    ) -> Vec<B> {
        Self::apply_to_all_variants_rec(&[], &mut self.layers.into(), name, &apply)
    }

    pub fn run(self, base_directory: PathBuf) {
        let name = self.name.clone() + "_";
        self.apply_to_all_variants(
            |resolved_layers, name| {
                let mut instance = Either::Second(base_directory.join(name));
                for resolved in resolved_layers {
                    match resolved
                        .apply(&instance)
                        .expect("Error while running profile")
                    {
                        Some(new_instance) => instance = Either::First(new_instance),
                        _ => {}
                    }
                }
            },
            name,
        );
    }

    fn apply_to_all_variants_rec<F: Fn(&[ResolvedLayer], String) -> B, B>(
        prev: &[ResolvedLayer],
        coming: &mut VecDeque<Layer>,
        name: String,
        apply: &F,
    ) -> Vec<B> {
        if coming.len() == 0 {
            return vec![apply(prev, name)];
        }

        let mut resolved = coming.pop_front().unwrap().resolve(prev);
        while resolved.len() == 0 {
            resolved = coming.pop_front().unwrap().resolve(prev);
        }

        resolved
            .into_iter()
            .enumerate()
            .flat_map(|(index, resolved_layer)| {
                Self::apply_to_all_variants_rec(
                    &[prev, &[resolved_layer]].concat(),
                    &mut coming.clone(),
                    format!("{name}{index:02}"),
                    apply,
                )
            })
            .collect()
    }
}

impl ResolvedLayer {
    pub fn apply(
        &self,
        instance: &Either<instance::Instance, PathBuf>,
    ) -> Result<Option<instance::Instance>, Box<dyn Error>> {
        let path = match instance {
            Either::First(instance) => &instance.path,
            Either::Second(path) => path,
        };
        return match self {
            Self::DeleteDirectory(target) => {
                let full_target_path = path.join(target);
                if full_target_path.is_dir() {
                    fs::remove_dir_all(path)?;
                }
                Ok(None)
            }
            Self::Instance {
                version,
                loader,
                loader_version,
            } => Ok(Some(
                instance::Instance::new(
                    path.file_name().unwrap().to_string_lossy().to_string(),
                    version.clone(),
                    instance::InstanceLaunchConfig::default(),
                    path.parent().unwrap(),
                    *loader,
                    loader_version.clone(),
                )
                .unwrap(),
            )),
            Self::DirectoryOverlay { source } => {
                copy_dir_all(path.join(source), path)?;
                Ok(None)
            }
            Self::ExecuteCommand(cmd) => {
                if run_cmd(cmd, &path).status.success() {
                    return Ok(None);
                }
                return Err(EvaluationError {})?;
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

        fn run_cmd(cmd: &String, path: &PathBuf) -> Output {
            if cfg!(target_os = "windows") {
                Command::new("cmd")
                    .current_dir(path)
                    .args(["/C", cmd])
                    .output()
                    .expect("failed to execute process")
            } else {
                Command::new("sh")
                    .current_dir(path)
                    .arg("-c")
                    .arg(cmd)
                    .output()
                    .expect("failed to execute process")
            }
        }
        fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
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

pub enum Either<T, U> {
    First(T),
    Second(U),
}

#[derive(Debug)]
pub struct EvaluationError {}

impl Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EvaluationError")?;
        Ok(())
    }
}

impl Error for EvaluationError {}
