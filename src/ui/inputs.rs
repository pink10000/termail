use ratatui::crossterm::event::{KeyCode, KeyEvent};
use crate::ui::{
    event::AppEvent,
    app::{App, ActiveViewState, BaseViewState, MessageViewState},
    components::composer_view::{Composer, ComposeViewField},
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
            
            // Handle Compose View
            (_, KeyCode::Char('c')) | (_, KeyCode::Char('C')) => {
                self.state = ActiveViewState::ComposeView(Composer {
                    draft: EmailMessage::new(),
                    current_field: ComposeViewField::To,
                    cursor_to: 0,
                    cursor_subject: 0,
                    editor_name: self.config.termail.editor.clone(),
                });
            },

            // Handle View Cycling
            (BaseViewState::Labels, KeyCode::Tab) => self.state = ActiveViewState::BaseView(BaseViewState::Inbox),
            (BaseViewState::Inbox, KeyCode::Tab) => self.state = ActiveViewState::BaseView(BaseViewState::Labels),
                
            // TODO: Handle scrolling through the labels.
            (BaseViewState::Inbox, KeyCode::Down) => self.hover_next_email(),
            (BaseViewState::Inbox, KeyCode::Up) => self.hover_previous_email(),
            (BaseViewState::Inbox, KeyCode::Enter) => {
                // Enter the message view with initial scroll position at the top
                let selected_email = self.selected_email_index
                    .and_then(|index| self.emails.as_ref()?.get(index))
                    .cloned()
                    .unwrap_or_else(EmailMessage::new);
                
                let (term_w, _) = ratatui::crossterm::terminal::size().unwrap();

                // Since we're using the terminal as the height, we will overcount a couple of lines
                // due to the rendering of the top bar. However, since the top bar is 3 lines, this 
                // discrepancy is acceptable.
                //
                // For each `\n`, we get at least one line. For each word, we divide the words into chunks of the max width,
                // and add the number of chunks - 1 to the line count.
                let content_height = selected_email.body
                    .lines()
                    .map(|line| line.chars().count() / term_w as usize + 1) // +1 for the \n
                    .sum::<usize>() as u16;

                self.state = ActiveViewState::MessageView(MessageViewState { scroll: 0, content_height });
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

    /// Handles key events for the message view.
    /// 
    /// Supports scrolling through the message body.
    fn handle_message_view(&mut self, key_event: KeyEvent) -> Result<(), Error> {
        match key_event.code {
            KeyCode::Esc => self.state = ActiveViewState::BaseView(BaseViewState::Inbox),
            KeyCode::Down => self.change_scroll(1)?,
            KeyCode::Up => self.change_scroll(-1)?,
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
        
        match key_event.code {
            // TODO: A pop up to confirm that the user wants to exit the compose view.
            // Should also be in the config file if the user wants this popup to appear.
            KeyCode::Esc => self.state = ActiveViewState::BaseView(BaseViewState::Inbox),
            KeyCode::Down => match cvs.current_field {
                ComposeViewField::To => cvs.current_field = ComposeViewField::Subject,
                ComposeViewField::Subject => cvs.current_field = ComposeViewField::Body,
                ComposeViewField::Body => cvs.current_field = ComposeViewField::To,
            },
            KeyCode::Up => match cvs.current_field {
                ComposeViewField::To => cvs.current_field = ComposeViewField::Body,
                ComposeViewField::Subject => cvs.current_field = ComposeViewField::To,
                ComposeViewField::Body => cvs.current_field = ComposeViewField::Subject,
            },
            KeyCode::Left => match cvs.current_field {
                ComposeViewField::To => cvs.cursor_to = cvs.cursor_to.saturating_sub(1),
                ComposeViewField::Subject => cvs.cursor_subject = cvs.cursor_subject.saturating_sub(1),
                _ => {}
            },
            KeyCode::Right => match cvs.current_field {
                ComposeViewField::To => cvs.cursor_to += if cvs.cursor_to < cvs.draft.to.len() { 1 } else { 0 },
                ComposeViewField::Subject => cvs.cursor_subject += if cvs.cursor_subject < cvs.draft.subject.len() { 1 } else { 0 },
                _ => {}
            },
            KeyCode::Char(c) => match cvs.current_field {
                ComposeViewField::To => {
                    cvs.cursor_to = cvs.cursor_to.min(cvs.draft.to.len());
                    cvs.draft.to.insert(cvs.cursor_to, c);
                    cvs.cursor_to += 1;
                },
                ComposeViewField::Subject => {
                    cvs.cursor_subject = cvs.cursor_subject.min(cvs.draft.subject.len());
                    cvs.draft.subject.insert(cvs.cursor_subject, c);
                    cvs.cursor_subject += 1;
                },
                _ => {}
            },
            KeyCode::Backspace => match cvs.current_field {
                ComposeViewField::To => {
                    if cvs.cursor_to > 0 {
                        cvs.cursor_to -= 1;
                        cvs.draft.to.remove(cvs.cursor_to);
                    }
                },
                ComposeViewField::Subject => {
                    if cvs.cursor_subject > 0 {
                        cvs.cursor_subject -= 1;
                        cvs.draft.subject.remove(cvs.cursor_subject);
                    }
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// This function changes the scroll offset of the MessageViewState
    /// Since ratatui's `Paragraph` widget does not limit how far we can scroll down, 
    /// scroll down, we need to use the height of the Paragraph widget.
    /// 
    /// Note that the value 15 is arbitrary, and can be changed to any value.
    /// TODO: Make this configurable by the config.toml file OR a way to determine
    /// the height without knowing the UI layout.
    /// 
    /// Note that the `content_height` is estimated, and may not be exact. See the
    /// comment about using `term_w` in `handle_base_view()` for more details.
    /// Ideally, this value is determined by the height of the AppLayouts.middle
    /// rectangle, but its implementation would remove the separations of concerns
    /// as the App State would require the knowledge of the UI layout, which already
    /// requires knowledge of the App State. So for now, we'll just do a rough estimate.
    pub fn change_scroll(&mut self, amount: i16) -> Result<(), Error> {
        let view_state = match &mut self.state {
            ActiveViewState::MessageView(view_state) => view_state,
            _ => return Err(Error::Other("Not in message view".to_string())),
        };
        let overflow = 15;
        let max_scroll = view_state.content_height.saturating_sub(overflow);
        if amount > 0 {
            view_state.scroll = view_state.scroll.saturating_add(1).clamp(0, max_scroll);
        } else {
            view_state.scroll = view_state.scroll.saturating_sub(1).clamp(0, max_scroll);
        }
        Ok(())
    }    
}