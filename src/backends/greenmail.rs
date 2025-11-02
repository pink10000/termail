extern crate imap;

use super::{Backend, Command, Error};
use crate::auth::Credentials;
use crate::config::BackendConfig;

pub struct GreenmailBackend {
    host: String,
    port: u16,
    ssl: bool,
    credentials: Credentials,
}

impl GreenmailBackend {
    pub fn new(config: &BackendConfig) -> Self {
        let credentials = config.auth_credentials.clone()
            .expect("Greenmail backend requires credentials in configuration");
        
        Self {
            host: config.host.clone(),
            port: config.port,
            ssl: config.ssl,
            credentials,
        }
    }
}

impl Backend for GreenmailBackend {

    /// This function needs to get manually updated as we implement more 
    /// features for the backend.
    fn check_command_support(&self, cmd: &Command) -> Result<bool, Error> {
        match cmd {
            Command::FetchInbox { count } => Ok(*count > 0),
        }
    }

    fn do_command(&self, cmd: Command) -> Result<Option<String>, Error> {
        match cmd {
            Command::FetchInbox { count } => {
                if count == 1 {
                    self.fetch_inbox_top()
                } else {
                    Err(Error::Unimplemented { 
                        backend: "greenmail".to_string(), 
                        feature: format!("fetching {} emails", count) 
                    })
                }
            }
        }
    }

    fn fetch_inbox_top(&self) -> Result<Option<String>, Error> {
        let domain = self.host.as_str();
        
        // For local testing with self-signed certificates, we need to accept invalid certs
        // while still maintaining TLS encryption
        let tls = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .unwrap();
    
        // we pass in the domain twice to check that the server's TLS
        // certificate is valid for the domain we're connecting to.
        let client = imap::connect((domain, self.port), domain, &tls).unwrap();
    
        // the client we have here is unauthenticated.
        // to do anything useful with the e-mails, we need to log in
        let mut imap_session = client
            .login(&self.credentials.username, &self.credentials.password)
            .map_err(|e| e.0)?;
    
        // we want to fetch the first email in the INBOX mailbox
        imap_session.select("INBOX")?;
    
        // fetch message number 1 in this mailbox, along with its RFC822 field.
        // RFC 822 dictates the format of the body of e-mails
        let messages = imap_session.fetch("1", "RFC822")?;
        let message = if let Some(m) = messages.iter().next() {
            m
        } else {
            return Ok(None);
        };
    
        // extract the message's body
        let body = message.body().expect("message did not have a body!");
        let body = std::str::from_utf8(body)
            .expect("message was not valid utf-8")
            .to_string();
    
        // be nice to the server and log out
        imap_session.logout()?;
    
        Ok(Some(body))
    }

    fn fetch_inbox_top_n(&self, _n: usize) -> Result<Vec<String>, Error> {
        Ok(vec![])
    }

}