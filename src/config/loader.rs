use super::models::AppConfig;
use crate::{Error, Result};
use directories::ProjectDirs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn get_default_config_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "linear-motion", "linear-motion")
            .ok_or_else(|| Error::Other("Could not determine user directories".to_string()))?;

        Ok(project_dirs.config_dir().join("config.json"))
    }

    pub fn ensure_config_dir() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "linear-motion", "linear-motion")
            .ok_or_else(|| Error::Other("Could not determine user directories".to_string()))?;

        let config_dir = project_dirs.config_dir();
        std::fs::create_dir_all(config_dir)?;

        Ok(config_dir.to_path_buf())
    }

    pub fn get_default_database_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "linear-motion", "linear-motion")
            .ok_or_else(|| Error::Other("Could not determine user directories".to_string()))?;

        Ok(project_dirs.data_dir().join("sync-tool.db"))
    }

    pub fn ensure_data_dir() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "linear-motion", "linear-motion")
            .ok_or_else(|| Error::Other("Could not determine user directories".to_string()))?;

        let data_dir = project_dirs.data_dir();
        std::fs::create_dir_all(data_dir)?;

        Ok(data_dir.to_path_buf())
    }
    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
        let path = path.as_ref();
        debug!("Loading configuration from: {}", path.display());

        if !path.exists() {
            return Err(Error::Other(format!(
                "Configuration file {} not found. Run 'sync-tool init' first.",
                path.display()
            )));
        }

        let content = tokio::fs::read_to_string(path).await?;
        let config: AppConfig = serde_json::from_str(&content)?;

        // Validate the configuration
        config.validate()?;

        debug!("configuration loaded successfully from {}", path.display());
        debug!(
            "config: {} sync sources, database: {:?}",
            config.sync_sources.len(),
            config.database_path()
        );

        Ok(config)
    }

    pub fn load_from_file_sync<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
        let path = path.as_ref();
        debug!("Loading configuration from: {}", path.display());

        if !path.exists() {
            return Err(Error::Other(format!(
                "Configuration file {} not found. Run 'sync-tool init' first.",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&content)?;

        // Validate the configuration
        config.validate()?;

        info!("Configuration loaded successfully from {}", path.display());
        debug!(
            "Config: {} sync sources, database: {:?}",
            config.sync_sources.len(),
            config.database_path()
        );

        Ok(config)
    }

    pub fn validate_json_file<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(Error::Other(format!(
                "Configuration file {} not found",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = serde_json::from_str(&content)
            .map_err(|e| Error::Other(format!("JSON parsing error: {}", e)))?;

        config.validate()?;

        Ok(())
    }
}
