use google_gmail1::api::Message;
use crate::error::Error;
use crate::core::email::{EmailMessage, EmailSender, MimeType, EmailAttachment};
use maildir::Maildir;
use mailparse::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use chrono::DateTime;


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

        // Enable foreign key constraints (required for SQLite foreign keys to work)
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| Error::Other(format!("Failed to enable foreign keys: {}", e)))?;

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
                maildir_id TEXT NOT NULL UNIQUE
            )",
            [],
        )
        .map_err(|e| Error::Other(format!("Failed to create message_map table: {}", e)))?;

        // Metadata for the emails in the maildir
        // In particular, we want to be able to sort the emails by date (newest first)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS message_metadata (
                maildir_id TEXT PRIMARY KEY,
                date_timestamp INTEGER NOT NULL,
                subject TEXT,
                sender TEXT
            )",
            [],
        )
        .map_err(|e| Error::Other(format!("Failed to create message_metadata table: {}", e)))?;

        // Index on date_timestamp for fast sorting
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_date_timestamp ON message_metadata(date_timestamp DESC)",
            [],
        )
        .map_err(|e| Error::Other(format!("Failed to create date index: {}", e)))?;

        // create label_map table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS label_map (
                maildir_id TEXT NOT NULL,
                label TEXT NOT NULL,
                PRIMARY KEY (maildir_id, label),
                FOREIGN KEY (maildir_id) REFERENCES message_map(maildir_id)
            )",
            [],
        )
        .map_err(|e| Error::Other(format!("Failed to create label_map table: {}", e)))?;

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

    /// Save or update metadata for an email
    pub fn save_metadata(&self, maildir_id: &str, date_str: &str, subject: &str, sender: &str) -> Result<(), Error> {
        let date_timestamp = DateTime::parse_from_rfc2822(date_str)
            .map(|dt| dt.timestamp())
            .map_err(|e| Error::Other(format!("Failed to parse date: {}", e)))?;

        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock connection: {}", e)))?;

        conn.execute(
            "INSERT OR REPLACE INTO message_metadata (maildir_id, date_timestamp, subject, sender) VALUES (?1, ?2, ?3, ?4)",
            params![maildir_id, date_timestamp, subject, sender],
        ).map_err(|e| Error::Other(format!("Failed to save metadata: {}", e)))?;

        tracing::debug!("Saved metadata for {}: {} (timestamp: {})", maildir_id, subject, date_timestamp);
        Ok(())
    }

    /// Get sorted maildir_ids from metadata (newest first)
    pub fn get_sorted_maildir_ids(&self, limit: usize) -> Result<Vec<String>, Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock connection: {}", e)))?;

        let mut stmt = conn.prepare(
            "SELECT maildir_id FROM message_metadata ORDER BY date_timestamp DESC LIMIT ?1"
        ).map_err(|e| Error::Other(format!("Failed to prepare metadata query: {}", e)))?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let maildir_id: String = row.get(0)?;
            Ok(maildir_id)
        }).map_err(|e| Error::Other(format!("Failed to query metadata: {}", e)))?;

        let maildir_ids = rows
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| Error::Other(format!("Failed to collect results: {}", e)))?;
        Ok(maildir_ids)
    }

    // Check if metadata exists for a maildir_id
    pub fn has_metadata(&self, maildir_id: &str) -> bool {
        let conn = match self.connection.lock() {
            Ok(c) => c,
            Err(_) => return false,
        };

        conn.query_row(
            "SELECT 1 FROM message_metadata WHERE maildir_id = ?1",
            params![maildir_id],
            |_| Ok(()),
        )
        .is_ok()
    }

    pub fn add_label_mappings(&self, maildir_id: &str, labels: &[String]) -> Result<(), Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        for label in labels {
            conn.execute(
                "INSERT OR REPLACE INTO label_map (maildir_id, label) VALUES (?1, ?2)",
                params![maildir_id, label],
            )
            .map_err(|e| Error::Other(format!("Failed to add label_map row: {}", e)))?;
        }
        Ok(())
    }

    pub fn remove_label_mappings(&self, maildir_ids: &[String]) -> Result<(), Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        for maildir_id in maildir_ids {
            conn.execute(
                "DELETE FROM label_map WHERE maildir_id = ?1",
                params![maildir_id],
            )
            .map_err(|e| Error::Other(format!("Failed to remove label_map row: {}", e))).unwrap();
        }
        
        Ok(())
    }

    pub fn get_maildir_ids_with_label(&self, label: &str) -> Result<Vec<String>, Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        // prepare statement
        let mut stmt = conn.prepare("SELECT maildir_id FROM label_map WHERE label = ?1")
            .map_err(|e| Error::Other(format!("Failed to prepare label_map query: {}", e)))?;
        
        // get all rows from table
        let rows = stmt.query_map(params![label], |row| row.get(0))
            .map_err(|e| Error::Other(format!("Failed to get emails with label: {}", e)))?;
        
        let mut maildir_ids = Vec::new();
        for row in rows {
            let maildir_id: String = row.map_err(|e| Error::Other(format!("Failed to read label_map row: {}", e)))?;
            maildir_ids.push(maildir_id);
        }
        Ok(maildir_ids)
    }

    /// Check if a maildir_id has a specific label in the database
    pub fn has_label(&self, maildir_id: &str, label: &str) -> Result<bool, Error> {
        let conn = self.connection.lock()
            .map_err(|e| Error::Other(format!("Failed to lock sync_state connection: {}", e)))?;
        
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM label_map WHERE maildir_id = ?1 AND label = ?2",
            params![maildir_id, label],
            |row| row.get(0),
        )
        .map_err(|e| Error::Other(format!("Failed to check label: {}", e)))?;
        
        Ok(count > 0)
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
    pub fn save_message(&self, message: &Message, maildir_subdir: String, labels: &Vec<String>) -> Result<String, Error> {
        let message_id = message.id.clone().unwrap();
        let raw_content = message.raw.clone().unwrap();
        
        // save message to correct maildir subdirectory
        let maildir_id = if maildir_subdir == "cur" {
            self.maildir.store_cur_with_flags(&raw_content, "")
                .map_err(|e| Error::Other(format!("Failed to store message in cur: {}", e)))?
        } else if maildir_subdir == "new" {
            self.maildir.store_new(&raw_content)
                .map_err(|e| Error::Other(format!("Failed to store message in new: {}", e)))?
        } else {
            return Err(Error::Other(format!("Invalid maildir subdirectory: {}", maildir_subdir)));
        };

        // Parse the message to extract metadata and save it to the database cache
        match parse_mail(&raw_content) {
            Ok(parsed) => {
                let date = parsed.headers.get_first_value("Date").unwrap_or_default();
                let subject = parsed.headers.get_first_value("Subject").unwrap_or_default();
                let from = parsed.headers.get_first_value("From").unwrap_or_default();

                if let Err(e) = self.save_metadata(&maildir_id, &date, &subject, &from) {
                    tracing::warn!("Failed to save metadata for {}: {}", maildir_id, e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to parse email for metadata extraction: {}", e);
            }
        }

        // add mapping to message_map table FIRST (before label_map due to foreign key constraint)
        self.add_mapping(message_id.clone(), maildir_id.clone())?;

        // save labels to label_map table (after message_map entry exists)
        self.add_label_mappings(&maildir_id, labels)?;

        Ok(maildir_id)
    }

    /// Parses an RFC822 email format into termail's EmailMessage struct using the `mailparse` crate.
    /// # Arguments
    /// * `raw_content` - The raw content of the email in RFC822 format.
    /// * `maildir_id` - The ID of the email in the maildir.
    /// * `is_unread` - Whether the email is unread (from database check).
    /// * `load_attachments` - Whether to load attachment data (set to false for list views to improve performance)
    pub fn parse_rfc822_email(&self, raw_content: &[u8], maildir_id: String, is_unread: bool, load_attachments: bool) -> Result<EmailMessage, Error> {
        let parsed = parse_mail(raw_content)
            .map_err(|e| Error::Other(format!("Failed to parse email: {}", e)))?;

        let mut email = EmailMessage::new();
        email.id = maildir_id; // TODO we want the gmail ID here not maildir id
        // fine rn since we are not doing any actions from the TUI that we want to sync up
        email.is_unread = is_unread;

        // extract headers using mailparse (automatically decodes MIME encoded-words)
        email.subject = parsed.headers.get_first_value("Subject").unwrap_or_default();
        email.from = EmailSender::from(parsed.headers.get_first_value("From").unwrap_or_default());
        email.to = parsed.headers.get_first_value("To").unwrap_or_default();
        email.date = parsed.headers.get_first_value("Date").unwrap_or_default();

        // self.print_email_mime_tree(&raw_content);

        let (body, attachments) = Self::walk_mime_parts(&parsed, load_attachments)?;

        email.body = body;
        email.email_attachments = attachments;

        Ok(email)
    }

    /// Recursively walks MIME parts to extract text content and attachments
    /// 
    /// # Arguments
    /// * `part` - The parsed MIME part to walk
    /// * `load_attachments` - If false, skips loading attachment data (for performance in list views)
    fn walk_mime_parts(part: &ParsedMail, load_attachments: bool) -> Result<(String, Vec<EmailAttachment>), Error> {
        let mimetype = &part.ctype.mimetype;
        let mut full_text = String::new();
        let mut full_attachments = Vec::new();
        
        let is_attachment = part.headers
            .get_first_value("Content-Disposition")
            .map(|disp| disp.to_lowercase().starts_with("attachment"))
            .unwrap_or(false);
        
        // Get filename from either Content-Type name parameter or Content-Disposition
        let filename = part.ctype.params.get("name")
            .cloned()
            .or_else(|| Self::get_filename_from_disposition_static(part));
        
        let is_image = mimetype.starts_with("image/");
        
        // If it has a filename, is marked as attachment, OR is an image, treat it as an attachment
        if filename.is_some() || is_attachment || is_image {
            // Generate a default filename if none exists
            let name = filename.unwrap_or_else(|| {
                if is_image {
                    let extension = mimetype.strip_prefix("image/").unwrap_or("img");
                    format!("image.{}", extension)
                } else {
                    "attachment".to_string()
                }
            });
            
            // Only load attachment data if requested
            if load_attachments {
                if let Ok(data) = part.get_body_raw() {
                    full_attachments.push(EmailAttachment {
                        filename: name,
                        content_type: mimetype.clone(),
                        data,
                        mime_type: MimeType::AttachmentPNG,
                    });
                }
            }
        } else if mimetype.starts_with("multipart/") {
            for subpart in &part.subparts {
                let (subpart_text, subpart_attachments) = Self::walk_mime_parts(subpart, load_attachments)?;
                full_text.push_str(&subpart_text);
                full_attachments.extend(subpart_attachments);
            }
        } else if mimetype == "text/plain" {
            // Extract plain text body
            if let Ok(text) = part.get_body() {
                full_text.push_str(&text);
            }
        } else if mimetype == "text/html" {
            // Extract HTML body
            if let Ok(html) = part.get_body() {
                full_text.push_str(&html);
            }
        }
        // Other MIME types (like application/*, etc.) without filenames are ignored
        Ok((full_text, full_attachments))
    }

    /// Static helper to check Content-Disposition for filenames (used in walk_mime_parts)
    fn get_filename_from_disposition_static(mail: &ParsedMail) -> Option<String> {
        let disposition = mail.get_headers().get_first_value("Content-Disposition")?;
        let parsed_disp = parse_content_disposition(&disposition);
        parsed_disp.params.get("filename").cloned()
    }

    // list all emails from maildir (both new and cur directories)
    pub fn list_emails(&self, count: usize) -> Result<Vec<EmailMessage>, Error> {
        self.list_emails_by_label(count, None)
    }

    // list emails filtered by label (if label is None, returns all emails)
    pub fn list_emails_by_label(&self, count: usize, label: Option<&str>) -> Result<Vec<EmailMessage>, Error> {
        let maildir_path = self.maildir.path();

        // If a label is specified, get the maildir IDs for that label
        let filtered_maildir_ids: Option<std::collections::HashSet<String>> = if let Some(label_name) = label {
            let maildir_ids = self.get_maildir_ids_with_label(label_name)?;
            Some(maildir_ids.into_iter().collect())
        } else {
            None
        };

        // collect entries from both new and cur directories
        let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();

        // Read from "new" directory (unread messages)
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
                    
                    // Extract maildir_id from filename (remove flags if present)
                    let maildir_id = filename.split(":2,").next().unwrap_or(&filename).to_string();
                    
                    // Filter by label if specified
                    if let Some(ref filtered_ids) = filtered_maildir_ids {
                        if !filtered_ids.contains(&maildir_id) {
                            continue;
                        }
                    }
                    
                    entries.push((maildir_id, path));
                }
            }
        }

        // Read from "cur" directory (read messages)
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
                    
                    // Extract maildir_id from filename (remove flags if present)
                    let maildir_id = filename.split(":2,").next().unwrap_or(&filename).to_string();
                    
                    // Filter by label if specified
                    if let Some(ref filtered_ids) = filtered_maildir_ids {
                        if !filtered_ids.contains(&maildir_id) {
                            continue;
                        }
                    }
                    
                    entries.push((maildir_id, path));
                }
            }
        }

        tracing::debug!("Found {} emails in maildir", entries.len());

        // Parse all emails and save metadata to database
        let mut emails: Vec<EmailMessage> = Vec::new();
        for (maildir_id, path) in entries {
            let maildir_id_clone = maildir_id.clone();
            let raw_content = std::fs::read(&path)
                .map_err(|e| Error::Other(format!("Failed to read maildir entry {}: {}", maildir_id_clone, e)))?;

            // Check database for UNREAD label to determine if email is unread
            let is_unread = self.has_label(&maildir_id, "UNREAD")
                .unwrap_or(false); // Default to false (read) if check fails

            match self.parse_rfc822_email(&raw_content, maildir_id.clone(), is_unread, false) {
                Ok(email) => {
                    // Save metadata to cache for future use
                    if let Err(e) = self.save_metadata(&maildir_id, &email.date, &email.subject, &email.from.email) {
                        tracing::warn!("Failed to save metadata for {}: {}", maildir_id, e);
                    }
                    emails.push(email);
                },
                Err(e) => tracing::warn!("Failed to parse email: {}", e),
            }
        }

        // Sort by email date (parsed from Date header) in descending order (newest first)
        emails.sort_by(|a, b| {
            let date_a = DateTime::parse_from_rfc2822(&a.date).ok();
            let date_b = DateTime::parse_from_rfc2822(&b.date).ok();
            date_b.cmp(&date_a) // Reverse for descending order (newest first)
        });
        
        tracing::info!("Built metadata cache for {} emails", emails.len());
        
        // Debug: log first few email dates to verify sort order
        if tracing::enabled!(tracing::Level::DEBUG) {
            for (i, email) in emails.iter().take(3).enumerate() {
                tracing::debug!("Email {}: {} - {}", i, email.subject, email.date);
            }
        }

        // Take only the requested count
        emails.truncate(count);
        Ok(emails)
    }

    /// Load a single email by maildir_id with full attachment data
    pub fn load_email_with_attachments(&self, maildir_id: &str) -> Result<EmailMessage, Error> {
        let maildir_path = self.maildir.path();

        let paths = [
            maildir_path.join("new").join(maildir_id),
            maildir_path.join("cur").join(maildir_id),
        ];

        for path in &paths {
            if path.exists() {
                let raw_content = std::fs::read(path)
                    .map_err(|e| Error::Other(format!("Failed to read {}: {}", maildir_id, e)))?;

                // Check database for UNREAD label
                let is_unread = self.has_label(maildir_id, "UNREAD")
                    .unwrap_or(false);
                return self.parse_rfc822_email(&raw_content, maildir_id.to_string(), is_unread, true);
            }
        }

        Err(Error::Other(format!("Email not found: {}", maildir_id)))
    }

    fn _print_email_mime_tree(&self, raw_content: &[u8]) {
        let parsed = parse_mail(raw_content)
            .map_err(|e| Error::Other(format!("Failed to parse email: {}", e))).unwrap();

        fn print_tree(mail: &ParsedMail, depth: usize) {
            let indent = "    ".repeat(depth);
            
            // Extract the MIME type (e.g., "text/plain", "multipart/mixed")
            let mime_type = &mail.ctype.mimetype;
            
            // Check if it is an attachment by looking for filename params
            let filename: Option<String> = mail.ctype.params.get("name").cloned()
                .or_else(|| MaildirManager::get_filename_from_disposition_static(mail));
        
            match filename {
                Some(name) => println!("{}|-- [Attachment] {} ({})", indent, name, mime_type),
                None => println!("{}|-- [Part] {}", indent, mime_type),
            }
        
            // Recurse into subparts (Context Frames)
            for subpart in &mail.subparts {
                print_tree(subpart, depth + 1);
            }
        }

        println!("--------------------------------");
        println!("Subject: {}", parsed.headers.get_first_value("Subject").unwrap_or_default());
        println!("From: {}", parsed.headers.get_first_value("From").unwrap_or_default());
        if let Some(to) = parsed.headers.get_first_value("To") {
            let to_emails_len = to.split(",").count();
            if to_emails_len >= 3 {
                println!("To: {} total emails (>= 3 detected)", to_emails_len);
            } else {
                println!("To: {}", to);
            }
        }
        println!("Date: {}", parsed.headers.get_first_value("Date").unwrap_or_default());
        print_tree(&parsed, 0);
        println!("--------------------------------\n");
    }
}
