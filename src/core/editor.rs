use crate::core::email::EmailMessage;
use std::io::{self, Write};
use tempfile::NamedTempFile;

pub struct Editor;

impl Editor {    
    pub fn open(editor: &str, mut draft: EmailMessage) -> io::Result<EmailMessage> {
        // Create a new temp file to be used by editor
        // File gets deleted once out of scope
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "To: {}", draft.to)?;
        writeln!(temp_file, "Subject: {}", draft.subject)?;
        writeln!(temp_file, "Body:\n{}", draft.body)?;

        let temp_file_path = temp_file.path().to_owned();

        // Create command to run editor with path as arg
        let mut command = std::process::Command::new(editor);
        if editor.contains("code") {
            // Add wait arg for vscode to ensure file is saved before returning
            command.arg("--wait").arg(&temp_file_path);
        }
        else {
            command.arg(&temp_file_path);
        }

        // Run the editor and check if it was successful
        let status = command.status()?;
        if !status.success() {
            tracing::error!("Editor failed with status: {:?}", status);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Editor failed",
            ));
        }

        // After the user exits the editor, read contents of temp file
        let contents = std::fs::read_to_string(&temp_file_path)?;
        let mut in_body = false;
        let mut body_lines = Vec::new();

        // Iterate through the lines of the file and parse the email fields
        // Evertyhing after Body: goes into body_lines
        for line in contents.lines() {
            if in_body {
                body_lines.push(line);
            } else if line.starts_with("To:") {
                draft.to = line["To:".len()..].trim().to_string();
            } else if line.starts_with("Subject:") {
                draft.subject = line["Subject:".len()..].trim().to_string();
            } else if line.starts_with("Body:") {
                in_body = true;
                body_lines.push(line["Body:".len()..].trim());
            }
        }
        draft.body = body_lines.join("\n");
        Ok(draft)
    }
}