use google_gmail1::api::Message;
use crate::error::Error;
use crate::core::email::{EmailMessage, EmailSender, MimeType};
use maildir::Maildir;
use mailparse::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;


pub struct MaildirManager {
    maildir: Maildir,
    db_path: PathBuf,
    connection: Mutex<Connection>,
}

impl MaildirManager {
    // create maildir manager
    pub fn new(maildir_path: String) -> Result<Self, Error> {
        
        let maildir = Maildir::from(maildir_path);

        // create maildir directories
        maildir.create_dirs()
            .map_err(|e| Error::Other(format!("Failed to create maildir directories: {}", e)))?;

        let db_path = maildir.path().join("sync_state.db");
        
        let conn = Self::open_or_create_database(&db_path)?;
        
        Ok(Self { 
            maildir,
            db_path,
            connection: Mutex::new(conn),
        })
    }

    fn open_or_create_database(sync_state_path: &Path) -> Result<Connection, Error> {
        // opens or create the database file
        let conn = Connection::open(sync_state_path)
            .map_err(|e| Error::Other(format!("Failed to open / create sync state database: {}", e)))?;

        Self::create_tables(&conn)?;
        Ok(conn)
    }

    // create tables if don't exist
    fn create_tables(conn: &Connection) -> Result<(), Error> {
        // create sync_state table
        // keeps track of the lasy sync id from gmail
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sync_state (
                key TEXT PRIMARY KEY,
                last_sync_id INTEGER NOT NULL
            )",
            [],
        )
        .map_err(|e| Error::Other(format!("Failed to create sync_state table: {}", e)))?;

        conn.execute(
            "INSERT OR IGNORE INTO sync_state (key, last_sync_id) VALUES ('state', 0)",
            [],
        )
        .map_err(|e| Error::Other(format!("Failed to initialize last_sync_id: {}", e)))?;

        // create message_map table
        // keeps track of the mapping between gmail_id and maildir_id
        conn.execute(
            "CREATE TABLE IF NOT EXISTS message_map (
                gmail_id TEXT PRIMARY KEY,
                maildir_id TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| Error::Other(format!("Failed to create message_map table: {}", e)))?;

        Ok(())
    }


    // read last_sync_id from the database
    pub fn get_last_sync_id(&self) -> u64 {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)));
        
        if let Ok(conn) = conn {
            conn.query_row(
                "SELECT last_sync_id FROM sync_state WHERE key = 'state'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
        } else {
            return 0;
        }
    }   

    // save last_sync_id to the database
    pub fn save_last_sync_id(&self, last_sync_id: u64) -> Result<(), Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        conn.execute(
            "UPDATE sync_state SET last_sync_id = ?1 WHERE key = 'state'",
            params![last_sync_id as i64],
        )
        .map_err(|e| Error::Other(format!("Failed to update last_sync_id: {}", e)))?;

        Ok(())
    }

    // returns the filesystem path to the db
    pub fn get_sync_state_path(&self) -> PathBuf {
        self.db_path.clone()
    }

    // returns the number of mappings in the db
    pub fn get_number_of_mappings(&self) -> Result<usize, Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        let statement = "SELECT COUNT(*) FROM message_map";
        let count: u32 = conn.query_row(statement, params![], |row| row.get(0))
            .map_err(|e| Error::Other(format!("Failed to get number of mappings: {}", e)))?;
        Ok(count as usize)
    }

    // checks if there are any mappings in the db
    pub fn has_synced_emails(&self) -> Result<bool, Error> {
        if self.get_number_of_mappings()? > 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // returns the maildir_id for a given gmail_id
    pub fn get_maildir_id(&self, gmail_id: &str) -> Result<Option<String>, Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        conn.query_row(
            "SELECT maildir_id FROM message_map WHERE gmail_id = ?1",
            params![gmail_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| Error::Other(format!("Failed to fetch maildir_id: {}", e)))
    }

    // returns all gmail_id -> maildir_id mappings from the db
    pub fn get_all_mappings(&self) -> Result<HashMap<String, String>, Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        // prepare statement
        let mut stmt = conn.prepare("SELECT gmail_id, maildir_id FROM message_map")
            .map_err(|e| Error::Other(format!("Failed to prepare message_map query: {}", e)))?;
        
        // get all rows from table
        let rows = stmt.query_map(params![], |row| {
            let gmail_id: String = row.get(0)?;
            let maildir_id: String = row.get(1)?;
            Ok((gmail_id, maildir_id))
        })
        .map_err(|e| Error::Other(format!("Failed to query message_map: {}", e)))?;

        // iterate rows and put into hashmap
        let mut mappings = HashMap::new();
        for row in rows {
            let (gmail_id, maildir_id) = row
                .map_err(|e| Error::Other(format!("Failed to read message_map row: {}", e)))?;
            mappings.insert(gmail_id, maildir_id);
        }
        Ok(mappings)
    }


    // remove mappings for passed gmail_ids.
    pub fn remove_mappings(&self, gmail_ids: &[String]) -> Result<(), Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;

        for gmail_id in gmail_ids {
            conn.execute(
                "DELETE FROM message_map WHERE gmail_id = ?1",
                params![gmail_id],
            )
            .map_err(|e| Error::Other(format!("Failed to delete message_map row: {}", e)))?;
        }

        Ok(())
    }

    // add mapping for passed gmail_id and maildir_id.
    pub fn add_mapping(&self, gmail_id: String, maildir_id: String) -> Result<(), Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;

        conn.execute(
            "INSERT OR REPLACE INTO message_map (gmail_id, maildir_id) VALUES (?1, ?2)",
            params![gmail_id, maildir_id],
        )
        .map_err(|e| Error::Other(format!("Failed to add message_map row: {}", e)))?;
        
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

    /// parse an RFC822 email format into an EmailMessage struct using mailparse crate
    pub fn parse_rfc822_email(&self, raw_content: &[u8], maildir_id: String) -> Result<EmailMessage, Error> {
        
        // use mailparse to parse the email
        let parsed = parse_mail(raw_content)
            .map_err(|e| Error::Other(format!("Failed to parse email: {}", e)))?;

        let mut email = EmailMessage::new();
        email.id = maildir_id; // TODO we want the gmail ID here not maildir id
        // fine rn since we are not doing any actions from the TUI that we want to sync up

        // extract headers using mailparse (automatically decodes MIME encoded-words)
        if let Some(subject) = parsed.headers.get_first_value("Subject") {
            email.subject = subject;
        }

        if let Some(from) = parsed.headers.get_first_value("From") {
            email.from = EmailSender::from(from);
        }

        if let Some(to) = parsed.headers.get_first_value("To") {
            email.to = to;
        }

        if let Some(date) = parsed.headers.get_first_value("Date") {
            email.date = date;
        }

        // extract body and mime type from parts
        let (body, mime_type) = if !parsed.subparts.is_empty() {
            let mut body = String::new();
            let mut mime_type = Default::default();
            
            for part in &parsed.subparts {
                if let Ok(text) = part.get_body()
                {
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
    
}
