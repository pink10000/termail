use google_gmail1::api::Message;
use crate::error::Error;
use crate::core::email::{EmailMessage, EmailSender, MimeType};
use maildir::Maildir;
use std::path::Path;
use std::collections::HashMap;
use serde::Serialize;
use serde::Deserialize;
use std::path::PathBuf;
use mailparse::*;


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

    pub fn delete_message(&self, maildir_id: String) -> Result<(), Error> {
        
        // delete message from maildir
        self.maildir.delete(&maildir_id)?;
        
        Ok(())
    }

    pub fn maildir_move_new_to_cur(&self, maildir_id: &String) -> Result<(), Error> {
        self.maildir.move_new_to_cur(&maildir_id)?;
        Ok(())
    }

    // since this function deletes the message from cur, we need to return the new maildir_id
    // so that the calling function can update the sync state with the new maildir_id
    pub fn maildir_move_cur_to_new(&self, maildir_id: &String) -> Result<String, Error> {
        // find message in cur
        let mail_entry = self.maildir.find(maildir_id.as_str())
            .ok_or_else(|| Error::Other(format!("Message not found: {}", maildir_id)))?;
        
        let path = mail_entry.path();
        
        // Read the raw message content from the file
        let raw_content = std::fs::read(path)
            .map_err(|e| Error::Other(format!("Failed to read message: {}", e)))?;
        
        // delete message from cur
        self.maildir.delete(&maildir_id)?;
        
        // move message to new
        let new_maildir_id = self.maildir.store_new(&raw_content)
            .map_err(|e| Error::Other(format!("Failed to store in new: {}", e)))?;
        
        Ok(new_maildir_id)
    }

    pub fn get_message_directory(&self, maildir_id: &String) -> Result<String, Error> {
        let mail_entry = self.maildir.find(maildir_id.as_str())
            .ok_or_else(|| Error::Other(format!("Message not found: {}", maildir_id)))?;
        let path = mail_entry.path();
        if path.to_string_lossy().contains("/new/") {
            Ok("new".to_string())
        } else if path.to_string_lossy().contains("/cur/") {
            Ok("cur".to_string())
        } else {
            Err(Error::Other(format!("Message path doesn't contain new or cur: {:?}", path)))
        }
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

    /// Parses an RFC822 email format into termail's EmailMessage struct using the `mailparse` crate.
    /// # Arguments
    /// * `raw_content` - The raw content of the email in RFC822 format.
    /// * `maildir_id` - The ID of the email in the maildir.
    pub fn parse_rfc822_email(&self, raw_content: &[u8], maildir_id: String) -> Result<EmailMessage, Error> {        
        let parsed = parse_mail(raw_content)
            .map_err(|e| Error::Other(format!("Failed to parse email: {}", e)))?;

        let mut email = EmailMessage::new();
        email.id = maildir_id; // TODO we want the gmail ID here not maildir id
        // fine rn since we are not doing any actions from the TUI that we want to sync up

        // extract headers using mailparse (automatically decodes MIME encoded-words)
        email.subject = parsed.headers.get_first_value("Subject").unwrap_or_default();
        email.from = EmailSender::from(parsed.headers.get_first_value("From").unwrap_or_default());
        email.to = parsed.headers.get_first_value("To").unwrap_or_default();
        email.date = parsed.headers.get_first_value("Date").unwrap_or_default();

        self.print_email_mime_tree(&raw_content);

        // extract body and mime type from parts
        let (body, mime_type) = if !parsed.subparts.is_empty() {
            let mut body = String::new();
            let mut mime_type = Default::default();
            
            for part in &parsed.subparts {
                if let Ok(text) = part.get_body() {
                    body.push_str(&text);
                }

                if let Some(part_mime) = &part.headers.get_first_header("Content-Type") {
                    let part_mime = part_mime.get_value().to_lowercase();
                    if part_mime.contains("html") {
                        mime_type = MimeType::TextHtml;
                    }
                }
            }
            
            (body, mime_type)
        } else {
            // fallback
            let body = parsed.get_body()
                .unwrap_or_else(|_| String::new());
            (body, MimeType::TextPlain)
        };

        email.body = body;
        email.mime_type = mime_type;

        Ok(email)
    }

    // list all emails from maildir (both new and cur directories)
    pub fn list_emails(&self, count: usize) -> Result<Vec<EmailMessage>, Error> {
        let mut emails = Vec::new();
        let maildir_path = self.maildir.path();

        // collect entries from both new and cur directories
        let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();

        // read from "new" directory (unread messages)
        let new_dir = maildir_path.join("new");
        if new_dir.exists() {
            
            let new_entries = std::fs::read_dir(&new_dir)
                .map_err(|e| Error::Other(format!("Failed to read new directory: {}", e)))?;

            for entry in new_entries {
                let entry = entry.map_err(|e| Error::Other(format!("Failed to read directory entry: {}", e)))?;
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    entries.push((filename, path));
                }
            }
        }

        // read from "cur" directory (read messages)
        let cur_dir = maildir_path.join("cur");
        if cur_dir.exists() {

            let cur_entries = std::fs::read_dir(&cur_dir)
                .map_err(|e| Error::Other(format!("Failed to read cur directory: {}", e)))?;

            for entry in cur_entries {
                let entry = entry.map_err(|e| Error::Other(format!("Failed to read directory entry: {}", e)))?;
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    entries.push((filename, path));
                }
            }
        }

        // sort by filename (which contains timestamp) - oldest first (reverse order)
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        // take only the requested count
        for (maildir_id, path) in entries.into_iter().take(count) {
            // If we want to do actions from the TUI (delete, mark as read, archive, add label) then we will need to transalte maildir_id -> gmail_id
            let maildir_id_clone = maildir_id.clone();
            let raw_content = std::fs::read(&path)
                .map_err(|e| Error::Other(format!("Failed to read maildir entry {}: {}", maildir_id_clone, e)))?;

            match self.parse_rfc822_email(&raw_content, maildir_id) {
                Ok(email) => emails.push(email),
                Err(e) => eprintln!("Failed to parse email: {}", e),
            }
        }

        Ok(emails)
    }
    
    fn print_email_mime_tree(&self, raw_content: &[u8]) {
        let parsed = parse_mail(raw_content)
            .map_err(|e| Error::Other(format!("Failed to parse email: {}", e))).unwrap();

        fn print_tree(mail: &ParsedMail, depth: usize) {
            let indent = "    ".repeat(depth);
            
            // Extract the MIME type (e.g., "text/plain", "multipart/mixed")
            let mime_type = &mail.ctype.mimetype;
            
            // Check if it is an attachment by looking for filename params
            let filename: Option<String> = mail.ctype.params.get("name").cloned()
                .or_else(|| get_filename_from_disposition(mail));
        
            match filename {
                Some(name) => println!("{}|-- [Attachment] {} ({})", indent, name, mime_type),
                None => println!("{}|-- [Part] {}", indent, mime_type),
            }
        
            // Recurse into subparts (Context Frames)
            for subpart in &mail.subparts {
                print_tree(subpart, depth + 1);
            }
        }

        // Helper to check Content-Disposition for filenames
        fn get_filename_from_disposition<'a>(mail: &'a ParsedMail) -> Option<String> {
            let disposition = mail.get_headers().get_first_value("Content-Disposition")?;
            let parsed_disp = parse_content_disposition(&disposition);
            parsed_disp.params.get("filename").map(|s| s.clone())
        }      

        print_tree(&parsed, 0);
    }
}
