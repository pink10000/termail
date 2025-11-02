use super::{Backend, Command, Error};
use crate::config::BackendConfig;

pub struct GmailBackend {
    host: String,
    port: u16,
    ssl: bool,
    oauth2_client_secret_file: Option<String>,
}

impl GmailBackend {
    pub fn new(config: &BackendConfig) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            ssl: config.ssl,
            oauth2_client_secret_file: config.oauth2_client_secret_file.clone(),
        }
    }
}

impl Backend for GmailBackend {
    fn check_command_support(&self, _cmd: &Command) -> Result<bool, Error> {
        Err(Error::Unimplemented {
            backend: "gmail".to_string(),
            feature: "all commands".to_string(),
        })
    }

    fn do_command(&self, cmd: Command) -> Result<Option<String>, Error> {
        Err(Error::Unimplemented {
            backend: "gmail".to_string(),
            feature: format!("{:?}", cmd),
        })
    }

    fn fetch_inbox_top(&self) -> Result<Option<String>, Error> {
        Err(Error::Unimplemented {
            backend: "gmail".to_string(),
            feature: "fetch_inbox_top".to_string(),
        })
    }

    fn fetch_inbox_top_n(&self, _n: usize) -> Result<Vec<String>, Error> {
        Err(Error::Unimplemented {
            backend: "gmail".to_string(),
            feature: "fetch_inbox_top_n".to_string(),
        })
    }
}

