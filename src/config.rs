// This file contains the configuration, and the configuration file parsing logic.
// It should use the definitions in the backends module to define the configuration and define correctness. 
// If the configuration is invalid, it should immediately fail.

use crate::error::Error;
use crate::backends::BackendType;
use crate::auth::{AuthProvider, Credentials};
use crate::Args;

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TermailConfig {
    pub cli: bool,
    pub default_backend: BackendType,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BackendConfig {
    pub backend_type: BackendType,
    auth_provider: AuthProvider,
    auth_credentials: Option<Credentials>,
    host: String,
    port: u16,
    ssl: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {    
    pub termail: TermailConfig,
    pub backends: HashMap<BackendType, BackendConfig>,
}

impl Config {
    /// Reads a config file from the following locations in order:
    /// 1. The config file path provided by the user
    /// 2. The current directory
    /// 3. `~/.config/termail/config.toml`
    /// 4. `/etc/termail/config.toml`
    pub fn load(config_file_path: Option<PathBuf>) -> Self {
        let config_file = match config_file_path {
            Some(p) => fs::read_to_string(p)
                .map_err(|e| Error::Config(e.to_string())),
            None => std::fs::read_to_string("config.toml")
                .or_else(|_| fs::read_to_string("~/.config/termail/config.toml"))
                .or_else(|_| fs::read_to_string("/etc/termail/config.toml"))
                .map_err(|e| Error::Config(e.to_string())),
        };

        let config: Result<Config, Error> = toml::from_str(config_file.unwrap().as_str())
            .map_err(|e| Error::Config(e.to_string()));
        return match config {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error loading config: {}", e);
                std::process::exit(1);
            }
        }
    }

    pub fn merge(&mut self, args: &Args) -> &mut Self {
        self.termail.cli = args.cli;
        self
    }

}