use google_gmail1::api::Message;
use crate::error::Error;
use maildir::Maildir;
use std::path::Path;
use std::collections::HashMap;
use serde::Serialize;
use serde::Deserialize;
use std::path::PathBuf;


#[derive(Serialize, Deserialize, Clone)]
pub struct SyncState {
    pub last_sync_id: u64,
    pub sync_state_path: PathBuf,
    pub message_id_to_maildir_id: HashMap<String, String>,
}

pub struct MaildirManager {
    maildir: Maildir,
    sync_state: SyncState,
}

impl MaildirManager {
    // create maildir manager
    pub fn new(maildir_path: String) -> Result<Self, Error> {
        
        let maildir = Maildir::from(maildir_path);

        // create maildir directories
        maildir.create_dirs()
            .map_err(|e| Error::Other(format!("Failed to create maildir directories: {}", e)))?;

        let sync_state_path = maildir.path().join("sync_state.json");
        let sync_state = Self::initialize_sync_state(&sync_state_path)?;
        
        Ok(Self { 
            maildir,
            sync_state: sync_state,
        })
    }

    fn initialize_sync_state(sync_state_path: &Path) -> Result<SyncState, Error> {
        let sync_state: SyncState;

        if !sync_state_path.exists() {
            // sync state file does not exist, create it with default values, write to file then return SyncState
            sync_state = SyncState {
                last_sync_id: 0,
                sync_state_path: sync_state_path.to_path_buf(),
                message_id_to_maildir_id: HashMap::new(),
            };

            Self::save_sync_state_to_file(sync_state_path, &sync_state)?;

        } else {
            // sync state file exists, read it, and parse it into a SyncState struct
            sync_state = Self::load_sync_state_from_file(sync_state_path)?;
        }

        Ok(sync_state)
    }


    pub fn get_last_sync_id(&self) -> u64 {
        self.sync_state.last_sync_id
    }

    pub fn get_sync_state_path(&self) -> PathBuf {
        self.sync_state.sync_state_path.clone()
    }

    pub fn has_synced_emails(&self) -> Result<bool, Error> {
        // Check if message_id_map has any entries
        let state = MaildirManager::load_sync_state_from_file(&self.sync_state.sync_state_path)?;
        Ok(!state.message_id_to_maildir_id.is_empty())
    }

    // load sync state from file and parse it into a SyncState struct
    pub fn load_sync_state_from_file(sync_state_path: &Path) -> Result<SyncState, Error> {
        let content = std::fs::read_to_string(sync_state_path).map_err(
            |e| Error::Other(format!("Failed to read sync state file: {}", e)))?;
        let sync_state = serde_json::from_str(&content).map_err(
            |e| Error::Other(format!("Failed to parse sync state file: {}", e)))?;

        Ok(sync_state)
    }

    // serialize SyncState struct and save it to file
    pub fn save_sync_state_to_file(sync_state_path: &Path, sync_state: &SyncState) -> Result<(), Error> {
        let content = serde_json::to_string_pretty(&sync_state)
                .map_err(|e| Error::Other(format!("Failed to serialize sync state: {}", e)))?;
        std::fs::write(sync_state_path, content)
            .map_err(|e| Error::Other(format!("Failed to write sync state to file: {}", e)))?;

        Ok(())
    }


    // save message to maildir
    pub fn save_message(&self, message: &Message, maildir_subdir: String) -> Result<String, Error> {

        let raw_content = message.raw.clone().unwrap();
        
        // save message to correct maildir subdirectory
        if maildir_subdir == "cur" {
            return self.maildir.store_cur_with_flags(&raw_content, "")
                .map_err(|e| Error::Other(format!("Failed to store message in cur: {}", e)));
        } else if maildir_subdir == "new" {
            return self.maildir.store_new(&raw_content)
                .map_err(|e| Error::Other(format!("Failed to store message in new: {}", e)));
        } else {
            return Err(Error::Other(format!("Invalid maildir subdirectory: {}", maildir_subdir)));
        }
    }
    
}
