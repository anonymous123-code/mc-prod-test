use helixlauncher_core::launch::instance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, path::PathBuf};

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
pub struct Profile {
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
#[serde(rename_all="snake_case")]
pub enum Layer {
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
#[serde(rename_all="snake_case")]
pub enum ResolvedLayer {
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
}

impl Layer {
    fn resolve(self, previous_layers: &[ResolvedLayer]) -> Vec<ResolvedLayer> {
        match self {
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
    pub fn print_tree(self) {
        Self::print_tree_rec(&[], &mut self.layers.into())
    }

    fn print_tree_rec(prev: &[ResolvedLayer], coming: &mut VecDeque<Layer>) {
        if coming.len() == 0 {
            println!("{prev:?}");
            return;
        }
        let mut resolved = coming.pop_front().unwrap().resolve(prev);
        while resolved.len() == 0 {
            resolved = coming.pop_front().unwrap().resolve(prev);
        }
        for resolved_layer in resolved {
            Self::print_tree_rec(&[prev, &[resolved_layer]].concat(), &mut coming.clone())
        }
    }
}
