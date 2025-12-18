pub mod hosts;

use color_eyre::{eyre::Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

pub use hosts::{Connection, HostConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub flake_path: Option<String>,

    #[serde(default)]
    pub extra_args: Vec<String>,

    #[serde(default)]
    pub hosts: HashMap<String, HostConfig>,
}

impl Config {
    /// Get the XDG config directory path for renix
    pub fn config_dir() -> Result<PathBuf> {
        let config_home = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME environment variable not set");
                PathBuf::from(home).join(".config")
            });

        Ok(config_home.join("renix"))
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load config from file, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            // Create config directory if it doesn't exist
            let config_dir = Self::config_dir()?;
            fs::create_dir_all(&config_dir).wrap_err("Failed to create config directory")?;

            // Create default config
            let default_config = Self::default();
            default_config.save()?;

            return Ok(default_config);
        }

        let contents = fs::read_to_string(&config_path).wrap_err("Failed to read config file")?;

        let config: Config = toml::from_str(&contents).wrap_err("Failed to parse config file")?;

        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let config_dir = Self::config_dir()?;

        fs::create_dir_all(&config_dir).wrap_err("Failed to create config directory")?;

        let contents = toml::to_string_pretty(self).wrap_err("Failed to serialize config")?;

        fs::write(&config_path, contents).wrap_err("Failed to write config file")?;

        Ok(())
    }

    /// Merge discovered configurations from a flake with existing config
    /// - Keeps existing connection info for known hosts
    /// - Auto-assigns localhost to configs matching current hostname
    /// - Marks new configs as unconfigured
    pub fn merge_discovered_configs(
        &mut self,
        discovered: HashSet<String>,
        current_hostname: &str,
    ) -> Result<()> {
        for config_name in discovered {
            // Skip if already configured
            if self.hosts.contains_key(&config_name) {
                continue;
            }

            // Auto-assign localhost if name matches hostname
            if config_name == current_hostname {
                self.hosts.insert(config_name, HostConfig::local());
            } else {
                // Mark as unconfigured
                self.hosts.insert(config_name, HostConfig::unconfigured());
            }
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            flake_path: None,
            extra_args: vec![],
            hosts: HashMap::new(),
        }
    }
}
