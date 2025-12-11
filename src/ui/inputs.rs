use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::ui::{
    event::AppEvent,
    app::{App, ActiveViewState, BaseViewState},
    components::composer_view::{Composer, ComposeViewField},
    components::message_view::Messager,
};
use crate::core::email::EmailMessage;
use crate::error::Error;

/// Input handling for the App
impl App {
    /// Handles key events for the application.
    /// 
    /// First, `handle_key_events()` checks the current view state, and delegates to 
    /// the appropriate handler for the current view state. 
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<(), Error> {
        match &self.state {
            ActiveViewState::BaseView(b) => self.handle_base_view(key_event, *b)?,
            ActiveViewState::MessageView(_) => self.handle_message_view(key_event)?,
            // TODO: if an editor is defined, it should drop us into that editor, 
            // such that we can write the email there. If the email is done being
            // written, exiting the program should return back to termail. 
            ActiveViewState::ComposeView(_) => self.handle_compose_view(key_event)?,
        }
        Ok(())
    }

    /// Cycles through BaseViewStates: Labels -> Inbox -> Labels
    /// State is preserved when cycling (e.g., selected email index is maintained)
    fn handle_base_view(&mut self, key_event: KeyEvent, b: BaseViewState) -> Result<(), Error> {
        match (b, key_event.code) {
            (_, KeyCode::Esc) => self.events.send(AppEvent::Quit),
            // Sync from cloud (refresh local maildir from backend)
            (_, KeyCode::Char('r')) => self.events.send(AppEvent::SyncFromCloud),
            
            // Handle Compose View
            (_, KeyCode::Char('c')) => self.state = ActiveViewState::ComposeView(Composer::new(self.config.termail.editor.clone())),

            // Handle View Cycling
            (BaseViewState::Labels, KeyCode::Tab) => self.state = ActiveViewState::BaseView(BaseViewState::Inbox),
            (BaseViewState::Inbox, KeyCode::Tab) => self.state = ActiveViewState::BaseView(BaseViewState::Labels),
            // Navigate folders when the folder pane is focused
            (BaseViewState::Labels, KeyCode::Down) => self.select_next_folder(),
            (BaseViewState::Labels, KeyCode::Up) => self.select_previous_folder(),
                
            // TODO: Handle scrolling through the labels.
            (BaseViewState::Inbox, KeyCode::Down) => self.hover_next_email(),
            (BaseViewState::Inbox, KeyCode::Up) => self.hover_previous_email(),
            (BaseViewState::Inbox, KeyCode::Enter) => {
                // Enter the message view with initial scroll position at the top
                let selected_email = self.selected_email_index
                    .and_then(|index| self.emails.as_ref()?.get(index))
                    .cloned()
                    .unwrap_or_else(EmailMessage::new);
                // Initialize image protocol if email has images
                self.init_image_protocol_for_email(&selected_email);

                self.state = ActiveViewState::MessageView(Messager::new(selected_email));
            }
            _ => {}
        }
        Ok(())
    }

    /// Hovers the next email in the list
    fn hover_next_email(&mut self) {
        if let Some(emails) = &self.emails {
            if emails.is_empty() {
                return;
            }
            
            if let Some(index) = self.selected_email_index {
                if index + 1 < emails.len() {
                    self.selected_email_index = Some(index + 1);
                }
            }
        }
    }

    /// Hovers the previous email in the list
    fn hover_previous_email(&mut self) {
        if let Some(index) = self.selected_email_index {
            if index > 0 {
                self.selected_email_index = Some(index - 1);
            }
        }
    }

    /// Move the folder selection down by one position.
    fn select_next_folder(&mut self) {
        self.shift_selected_folder(1);
    }

    /// Move the folder selection up by one position.
    fn select_previous_folder(&mut self) {
        self.shift_selected_folder(-1);
    }

    /// Shared logic for updating the selected folder based on direction.
    fn shift_selected_folder(&mut self, direction: isize) {
        let labels = match &self.labels {
            Some(labels) if !labels.is_empty() => labels,
            _ => return,
        };

        // Build a list of indices that have displayable names.
        let selectable_indices: Vec<usize> = labels
            .iter()
            .enumerate()
            .filter(|(_, label)| label.name.is_some())
            .map(|(idx, _)| idx)
            .collect();

        if selectable_indices.is_empty() {
            return;
        }

        let current_position = selectable_indices
            .iter()
            .position(|&idx| {
                labels[idx]
                    .name
                    .as_deref()
                    .map(|name| name == self.selected_folder)
                    .unwrap_or(false)
            })
            .unwrap_or(0);

        let max_pos = (selectable_indices.len() - 1) as isize;
        let mut new_position = current_position as isize + direction;
        new_position = new_position.clamp(0, max_pos);

        let new_label_idx = selectable_indices[new_position as usize];
        if let Some(name) = labels[new_label_idx].name.clone() {
            self.selected_folder = name;
        }
    }

    /// Handles key events for the message view.
    /// 
    /// Supports scrolling through the message body.
    fn handle_message_view(&mut self, key_event: KeyEvent) -> Result<(), Error> {
        let messager = match &mut self.state {
            ActiveViewState::MessageView(messager) => messager,
            _ => unreachable!("Not in message view"),
        };
        match key_event.code {
            KeyCode::Esc => self.state = ActiveViewState::BaseView(BaseViewState::Inbox),
            KeyCode::Down => messager.scroll_down(),
            KeyCode::Up => messager.scroll_up(),
            _ => {}
        }
        Ok(())
    }

    /// Handles the key events for the compose view.
    fn handle_compose_view(&mut self, key_event: KeyEvent) -> Result<(), Error> {
        let cvs = match &mut self.state {
            ActiveViewState::ComposeView(cvs) => cvs,
            _ => return Err(Error::Other("Not in compose view".to_string())),
        };
        
        // Depending on the terminal, some modifiers may not work as intended.
        // See: https://users.rust-lang.org/t/problem-with-key-events-in-tui/128754
        // This is dead code, but keeping it here for reference when we debug the issue.
        if key_event.modifiers.contains(KeyModifiers::SHIFT) {
            match key_event.code {
                KeyCode::Enter => {
                    // TODO: check if the email is valid
                    tracing::info!("Sending email: {:?}", cvs.draft);
                    self.events.send(AppEvent::SendEmail(cvs.draft.clone()));
                    self.state = ActiveViewState::BaseView(BaseViewState::Inbox);
                    // Return early to avoid borrowing `self.state` again. Alternatively,
                    // we could wrap the match in an else block, but that would be more verbose.
                    return Ok(())
                },
                _ => {}
            }
        }
        match (&cvs.current_field, key_event.code) {
            // TODO: A pop up to confirm that the user wants to exit the compose view.
            // Should also be in the config file if the user wants this popup to appear.
            (_, KeyCode::Esc) => self.state = ActiveViewState::BaseView(BaseViewState::Inbox),

            // Cycle through the fields
            (ComposeViewField::To, KeyCode::Down) => cvs.current_field = ComposeViewField::Subject,
            (ComposeViewField::Subject, KeyCode::Down) => cvs.current_field = ComposeViewField::Body,
            (ComposeViewField::Body, KeyCode::Down) => cvs.current_field = ComposeViewField::To,
            (ComposeViewField::To, KeyCode::Up) => cvs.current_field = ComposeViewField::Body,
            (ComposeViewField::Subject, KeyCode::Up) => cvs.current_field = ComposeViewField::To,
            (ComposeViewField::Body, KeyCode::Up) => cvs.current_field = ComposeViewField::Subject,

            // Move the cursor
            (ComposeViewField::To, KeyCode::Left) => cvs.cursor_to = cvs.cursor_to.saturating_sub(1),
            (ComposeViewField::Subject, KeyCode::Left) => cvs.cursor_subject = cvs.cursor_subject.saturating_sub(1),
            (ComposeViewField::To, KeyCode::Right) => {
                if cvs.cursor_to < cvs.draft.to.len() {
                    cvs.cursor_to += 1;
                }
            },
            (ComposeViewField::Subject, KeyCode::Right) => {
                if cvs.cursor_subject < cvs.draft.subject.len() {
                    cvs.cursor_subject += 1;
                }
            },

            // Insert a character
            (ComposeViewField::To, KeyCode::Char(c)) => {
                cvs.cursor_to = cvs.cursor_to.min(cvs.draft.to.len());
                cvs.draft.to.insert(cvs.cursor_to, c);
                cvs.cursor_to += 1;
            },
            (ComposeViewField::Subject, KeyCode::Char(c)) => {
                cvs.cursor_subject = cvs.cursor_subject.min(cvs.draft.subject.len());
                cvs.draft.subject.insert(cvs.cursor_subject, c);
                cvs.cursor_subject += 1;
            },

            // Delete a character
            (ComposeViewField::To, KeyCode::Backspace) => {
                if cvs.cursor_to > 0 {
                    cvs.cursor_to -= 1;
                    cvs.draft.to.remove(cvs.cursor_to);
                }
            },
            (ComposeViewField::Subject, KeyCode::Backspace) => {
                if cvs.cursor_subject > 0 {
                    cvs.cursor_subject -= 1;
                    cvs.draft.subject.remove(cvs.cursor_subject);
                }
            },

            // Spawn the editor to write the email body
            (ComposeViewField::Body, KeyCode::Enter) => self.events.send(AppEvent::SpawnEditor),
            (_, KeyCode::Char('p')) => {
                self.events.send(AppEvent::SendEmail(cvs.draft.clone()));
                self.state = ActiveViewState::BaseView(BaseViewState::Inbox);
            }
            _ => {}
        }
        Ok(())
    }
}