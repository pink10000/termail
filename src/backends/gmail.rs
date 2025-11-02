use super::{Backend, Command, Error};
use crate::config::BackendConfig;
use google_gmail1::{Gmail, hyper_rustls, hyper_util, yup_oauth2};

pub struct GmailBackend {
    host: String,
    port: u16,
    ssl: bool,
    oauth2_client_secret_file: Option<String>,
    hub: Option<Box<dyn std::any::Any>>,
}

impl GmailBackend {
    pub fn new(config: &BackendConfig) -> Self {
        Self {
            host: config.host.clone(),
            port: config.port,
            ssl: config.ssl,
            oauth2_client_secret_file: config.oauth2_client_secret_file.clone(),
            hub: None
        }
    }

    async fn authenticate_async(&mut self) -> Result<(), Error> {
        let secret_file = self.oauth2_client_secret_file.as_ref()
            .ok_or_else(|| Error::Config(
                "No OAuth2 client secret file configured for Gmail backend".to_string()
            ))?;

        let secret = yup_oauth2::read_application_secret(secret_file)
            .await
            .map_err(|e| Error::Config(format!("Failed to read OAuth2 secret file: {}", e)))?;

        // Set up the OAuth2 authenticator with installed flow (opens browser)
        let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
            secret,
            yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
        )
        .persist_tokens_to_disk("tokencache.json")
        .build()
        .await
        .map_err(|e| Error::Config(format!("Failed to build authenticator: {}", e)))?;
        
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| Error::Config(format!("Failed to load native roots: {}", e)))?
            .https_or_http()
            .enable_http1()
            .build();

        let client = hyper_util::client::legacy::Client::builder(
            hyper_util::rt::TokioExecutor::new()
        ).build(https);

        self.hub = Some(Box::new(Gmail::new(client, auth)));
        Ok(())
    }
}

impl Backend for GmailBackend {
    fn needs_oauth(&self) -> bool {
        true
    }

    fn authenticate(&mut self) -> Result<(), Error> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| Error::Config(format!("Failed to create tokio runtime: {}", e)))?;
        
        rt.block_on(self.authenticate_async())
    }

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

