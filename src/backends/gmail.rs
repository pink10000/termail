use super::{Backend, Command, Error};

pub struct GmailBackend;

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

