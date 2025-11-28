use google_gmail1::api::Message;
use crate::error::Error;
use maildir::Maildir;
use std::path::Path;
use std::collections::HashMap;

pub struct MaildirManager {
    maildir: Maildir,
    last_sync_id: u64,
}

impl MaildirManager {
    // create maildir manager
    pub fn new(maildir_path: String) -> Result<Self, Error> {
        
        let maildir = Maildir::from(maildir_path);

        // create maildir directories
        maildir.create_dirs()
            .map_err(|e| Error::Other(format!("Failed to create maildir directories: {}", e)))?;

        let last_sync_id = Self::create_or_read_sync_state_file(maildir.path()).map_err(|e| Error::Other(format!("Failed to create sync_state.json file: {}", e)))?;
        
        Ok(Self { 
            maildir,
            last_sync_id: last_sync_id,
        })
    }

    fn create_or_read_sync_state_file(maildir_path: &Path) -> Result<u64, Error> {
        let sync_state_file = maildir_path.join("sync_state.json");
        if !sync_state_file.exists() {
            // sync state file does not exist, create it with default values
            let content = "{\"last_sync_id\": 0}";
            std::fs::write(sync_state_file, content).map_err(
                |e| Error::Other(format!("Failed to create sync_state.json file: {}", e)))?;
            return Ok(0);

        } else {
            // sync state file exists, read it
            let content = std::fs::read_to_string(sync_state_file).map_err(
                |e| Error::Other(format!("Failed to read sync_state.json file: {}", e)))?;
            let sync_state: HashMap<String, u64> = serde_json::from_str(&content).map_err(
                |e| Error::Other(format!("Failed to parse sync_state.json file: {}", e)))?;
            return Ok(sync_state["last_sync_id"]);
        }
    }


    pub fn get_last_sync_id(&self) -> u64 {
        self.last_sync_id
    }


    // save message to maildir
    pub fn save_message(&self, message: Message, maildir_subdir: String) -> Result<(), Error> {
        if maildir_subdir == "cur" {
            self.maildir.store_cur_with_flags(&message.raw.unwrap(), "").map_err(
                |e| Error::Other(format!("Failed to store message in cur: {}", e)))?;
        } else if maildir_subdir == "new" {
            self.maildir.store_new(&message.raw.unwrap()).map_err(
                |e| Error::Other(format!("Failed to store message in new: {}", e)))?;
        } else {
            return Err(Error::Other(format!("Invalid maildir subdirectory: {}", maildir_subdir)));
        }
        Ok(())
    }
    
}
