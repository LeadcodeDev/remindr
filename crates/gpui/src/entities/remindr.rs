use std::path::PathBuf;

use anyhow::{Context, Error};
use serde_json::{from_str, to_string};
use tokio::fs::{create_dir_all, read_to_string, write};

use crate::states::settings_state::Settings;

#[derive(Clone)]
pub struct Remindr;

impl Remindr {
    pub fn new() -> Self {
        Remindr {}
    }

    pub fn get_config_dir(&self, project_name: &str) -> Result<PathBuf, Error> {
        let config_path = if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
            let home = dirs::home_dir().context("Impossible de trouver le home directory")?;
            home.join(".config").join(project_name)
        } else {
            let base = dirs::config_dir()
                .context("Impossible de récupérer le dossier de config Windows")?;
            base.join(project_name)
        };

        Ok(config_path)
    }

    pub async fn init(&self) -> Result<(), Error> {
        let config_path = self.get_config_dir("remindr")?;

        if !config_path.exists() {
            create_dir_all(&config_path)
                .await
                .with_context(|| format!("Failed to create {:?}", config_path))?;

            let settings_file = config_path.join("settings.json");

            let settings = Settings {};

            write(&settings_file, to_string(&settings).unwrap())
                .await
                .with_context(|| {
                    format!("Failed to write default settings to {:?}", settings_file)
                })?;
        }

        Ok(())
    }

    pub async fn load_settings(&self) -> Result<Settings, Error> {
        let config_path = self.get_config_dir("remindr")?;
        let settings_file = config_path.join("settings.json");

        if !settings_file.exists() {
            self.init().await?;
        }

        let settings = read_to_string(&settings_file)
            .await
            .with_context(|| format!("Failed to read settings from {:?}", settings_file))?;

        from_str::<Settings>(&settings).context("Failed to parse settings")
    }
}
