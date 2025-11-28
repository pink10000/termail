// This file contains the configuration, and the configuration file parsing logic.
// It should use the definitions in the backends module to define the configuration and define correctness. 
// If the configuration is invalid, it should immediately fail.

use crate::error::Error;
use crate::backends::BackendType;
use crate::auth::{Credentials};
use crate::backends::Backend;
use crate::Args;

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TermailConfig {
    pub cli: bool,
    pub default_backend: BackendType,
    pub email_fetch_count: usize,
    pub editor: String, 
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BackendConfig {
    pub auth_credentials: Option<Credentials>,
    pub host: String,
    pub port: u16,
    pub ssl: bool,
    pub oauth2_client_secret_file: Option<String>,
    // The labels to filter out from the list of labels
    // The labels are case-sensitive.
    pub filter_labels: Option<Vec<String>>,
    pub maildir_path: String
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
    pub fn load(config_file_path: Option<PathBuf>) -> Result<Self, Error> {
        let config_file = match config_file_path {
            Some(p) => fs::read_to_string(p)
                .map_err(|e| Error::Config(e.to_string())),
            None => std::fs::read_to_string("config.toml")
                .or_else(|_| fs::read_to_string("~/.config/termail/config.toml"))
                .or_else(|_| fs::read_to_string("/etc/termail/config.toml"))
                .map_err(|e| Error::Other(e.to_string())),
        };

        let config: Config = match config_file {
            Ok(c) => toml::from_str(c.as_str()).map_err(|e| Error::Config(e.to_string()))?,
            Err(e) => return Err(e),
        };
                
        // Validate backend configurations
        for (be_type, be_config) in config.backends.clone().into_iter() {
            match be_type {
                BackendType::GreenMail => {
                    if be_config.oauth2_client_secret_file != None {
                        Error::Config("Greenmail does not support OAuth2. Remove it from your config.".to_string());
                    }
                },
                BackendType::Gmail => {
                    if be_config.oauth2_client_secret_file == None {
                        Error::Config("Gmail requires OAuth2.".to_string());
                    }
                },
            }
        }
        Ok(config)

    }

    pub fn merge(&mut self, args: &Args) -> &mut Self {
        // If --cli flag was passed, override config
        if args.cli {
            self.termail.cli = true;
        }
        // If --backend was specified, override config
        if let Some(backend) = args.backend {
            self.termail.default_backend = backend;
        }
        self
    }

    pub fn get_backend(&self) -> Box<dyn Backend> {
        let selected_backend = self.termail.default_backend;
        
        let backend_config = self.backends.get(&selected_backend)
            .expect(&format!("No configuration found for backend '{}'", selected_backend));
        
        selected_backend.get_backend(backend_config, &self.termail.editor)
    }

    pub fn get_backend_config(&self, backend_type: &BackendType) -> Option<&BackendConfig> {
        self.backends.get(backend_type)
    }
}