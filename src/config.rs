use std::{collections::HashMap, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ProfileConfig {
    pub profiles: HashMap<String, crate::layer::Profile>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_config: Option<String>,

    #[serde(skip)]
    path: PathBuf,
}

impl ProfileConfig {
    pub fn read_or_create(path: PathBuf) -> Self {
        match fs::read_to_string(&path) {
            Ok(content) => {
                let mut it = Self::from(content.as_str()).expect("Profile config format invalid");
                it.path = path;
                it
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fs::write(
                    &path,
                    serde_json::to_string_pretty(&Self {
                        profiles: HashMap::new(),
                        active_config: None,
                        path: path.clone(),
                    })
                    .unwrap(),
                )
                .expect("Could not find or create config file");
                Self {
                    profiles: HashMap::new(),
                    active_config: None,
                    path,
                }
            }
            Err(e) => Err(e).expect("Could not read or create config file"),
        }
    }

    pub fn safe(&self) {
        fs::write(&self.path, serde_json::to_string_pretty(self).unwrap())
            .expect("Error saving config, state might be broken");
    }

    fn from(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}
