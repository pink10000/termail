use google_gmail1::api::Message;
use crate::error::Error;
use maildir::Maildir;

pub struct MaildirManager {
    maildir: Maildir,
}

impl MaildirManager {
    pub fn new(maildir_path: String) -> Result<Self, Error> {
        
        let maildir = Maildir::from(maildir_path);

        // create maildir directories
        maildir.create_dirs()
            .map_err(|e| Error::Other(format!("Failed to create maildir directories: {}", e)))?;

        Ok(Self { maildir })
    }

    pub fn save_message(&self, message: Message, maildir_subdir: String) -> Result<(), Error> {
   

        if maildir_subdir == "cur" {
            self.maildir.store_cur_with_flags(&message.raw.unwrap_or_default(), "").map_err(|e| Error::Other(format!("Failed to store message: {}", e)))?;
        } else if maildir_subdir == "new" {
            self.maildir.store_new(&message.raw.unwrap_or_default()).map_err(|e| Error::Other(format!("Failed to store message: {}", e)))?;
        } else {
            return Err(Error::Other("Invalid maildir subdirectory".to_string()));
        }
        Ok(())
    }
    
    // functions to make:
    // - save message to maildir
    // - get message from maildir
    // - list messages in maildir
    // - delete message from maildir

    // - get message count in maildir

    // pub fn save_message(&self, message: Message, maildir_subdir: String) -> Result<(), Error> {
    //     let maildir_path = self.get_maildir_path();
    //     let message_id = message.id.unwrap();
    //     let message_path = format!("{}/{}/{}", maildir_path, maildir_subdir, message_id);
    //     println!("Saving message to {}", message_path);
    //     std::fs::write(&message_path, message.raw.unwrap_or_default()).map_err(|e| Error::Other(e.to_string()))?;
    //     println!("Message saved successfully to {}", message_path);
    //     Ok(())
    // }

    // // need to search in maildir_subdir for message_id (curr, temp, new)
    // pub fn get_message(&self, message_id: String) -> Result<Message, Error> {
    //     let maildir_path = self.get_maildir_path();
    //     let maildir_subdir = ["cur", "tmp", "new"];
    //     for subdir in maildir_subdir {
    //         let message_path = format!("{}/{}/{}", maildir_path, subdir, message_id);
    //         if std::fs::exists(&message_path).unwrap_or(false) {
    //             let message = std::fs::read_to_string(&message_path).map_err(|e| Error::Other(e.to_string()))?;
    //             return Ok(serde_json::from_str(message.as_str()).map_err(|e| Error::Other(e.to_string()))?);
    //         }
    //     }
    //     return Err(Error::Other("Message not found".to_string()));
    // }
}
