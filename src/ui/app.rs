// This file contains the application logic for the termail UI.

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent}, DefaultTerminal
};

use crate::types::{Command, EmailMessage, Label, CommandResult};
use crate::ui::event::AppEvent;
use crate::config::Config;
use crate::error::Error;
use super::event::{Event, EventHandler};
use crate::backends::Backend;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::plugins::plugins::PluginManager;

#[derive(Clone, Debug, Copy)]
pub enum BaseViewState {
    Labels,
    Inbox,
}

#[derive(Clone, Debug)]
pub struct MessageViewState {
    /// Vertical scroll offset (in lines) for the message view
    pub scroll: u16,
    /// The height of the Paragraph widget
    pub content_height: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComposeViewField {
    To,
    Subject,
    Body,
}

#[derive(Clone, Debug)]
pub struct ComposeViewState {
    /// The draft email being composed
    pub draft: EmailMessage,
    /// Current field being edited
    pub current_field: ComposeViewField,
}

#[derive(Clone, Debug)]
pub enum ActiveViewState {
    /// This state holds the base view of the application, which is the sidebar 
    /// with labels, and the inbox view. 
    BaseView(BaseViewState),
    /// This state indicates that the user is viewing a single email message.
    MessageView(MessageViewState),
    /// This state indicates that the user is writing a new email message.
    ComposeView(ComposeViewState),
}

pub struct App {
    pub state: ActiveViewState,
    pub running: bool,
    pub events: EventHandler, 
    pub config: Config,
    /// Email storage. None means not loaded yet, Some(vec![]) means loaded but empty.
    pub emails: Option<Vec<EmailMessage>>,
    pub labels: Option<Vec<Label>>,
    /// Thread-safe backend for sharing across async tasks
    /// 
    /// We use this to allow multiple async tasks to access the backend concurrently. In 
    /// particular, we use it to fetch emails from the backend in a separate async task.
    pub backend: Arc<Mutex<Box<dyn Backend>>>,
    /// Counter to track ticks for periodic refresh (and other tasks)
    pub tick_counter: u64,
    /// Index of the currently selected email in the inbox view
    pub selected_email_index: Option<usize>,
    /// Name of the currently selected folder
    pub selected_folder: String,
    /// Plugin manager for executing plugins
    pub plugin_manager: Arc<Mutex<PluginManager>>,
}

impl App {
    pub fn new(
        config: Config, 
        backend: Box<dyn Backend>,
        plugin_manager: PluginManager,
    ) -> Self {
        let backend = Arc::new(Mutex::new(backend));
        let plugin_manager = Arc::new(Mutex::new(plugin_manager));
        let events = EventHandler::new();
        
        // Spawn initial label fetch
        Self::spawn_label_fetch(
            Arc::clone(&backend),
            events.get_sender(),
        );

        // Spawn initial email fetch
        Self::spawn_email_fetch(
            Arc::clone(&backend),
            events.get_sender(),
            config.termail.email_fetch_count,
        );

        Self { 
            state: ActiveViewState::BaseView(BaseViewState::Labels), 
            running: true,
            events,
            config,
            emails: None,  // Start with None to indicate loading state
            labels: None,  // Start with None to indicate loading state
            backend,
            tick_counter: 0,
            selected_email_index: Some(0),  // Start with first email selected
            selected_folder: "INBOX".to_string(),
            plugin_manager,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<(), Error> {
        while self.running {
            terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
            match self.events.next().await? {
                Event::Tick => self.tick(),
                Event::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event) => {
                        if key_event.kind == crossterm::event::KeyEventKind::Press {
                            self.handle_key_events(key_event)?;
                        }
                    }
                    _ => {}
                }
                Event::App(app_event) => match app_event {
                    AppEvent::Quit => self.quit(),
                    AppEvent::EmailsFetched(emails) => self.emails = Some(emails),
                    AppEvent::LabelsFetched(labels) => self.labels = Some(labels),
                }
            }
        }        
        Ok(())
    }
    
    /// Handles key events for the application.
    /// 
    /// First, `handle_key_events()` checks the current view state, and delegates to 
    /// the appropriate handler for the current view state. 
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<(), Error> {
        match &self.state {
            ActiveViewState::BaseView(b) => self.handle_key_base_view(key_event, *b)?,
            ActiveViewState::MessageView(_) => self.handle_key_message_view(key_event)?,
            // TODO: if an editor is defined, it should drop us into that editor, 
            // such that we can write the email there. If the email is done being
            // written, exiting the program should return back to termail. 
            ActiveViewState::ComposeView(_) => self.handle_key_compose_view(key_event)?,
        }
        Ok(())
    }

    /// Cycles through BaseViewStates: Labels -> Inbox -> Labels
    /// State is preserved when cycling (e.g., selected email index is maintained)
    fn handle_key_base_view(&mut self, key_event: KeyEvent, b: BaseViewState) -> Result<(), Error> {
        match (b, key_event.code) {
            (_, KeyCode::Esc) => self.events.send(AppEvent::Quit),
            
            // Handle Compose View
            (_, KeyCode::Char('c')) | (_, KeyCode::Char('C')) => {
                self.state = ActiveViewState::ComposeView(ComposeViewState {
                    draft: EmailMessage::new(),
                    current_field: ComposeViewField::To,
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

    /// This function changes the scroll offset of the MessageViewState
    /// Since ratatui's `Paragraph` widget does not limit how far we can scroll down, 
    /// scroll down, we need to use the height of the Paragraph widget.
    /// 
    /// Note that the value 15 is arbitrary, and can be changed to any value.
    /// TODO: Make this configurable by the config.toml file OR a way to determine
    /// the height without knowing the UI layout.
    /// 
    /// Note that the `content_height` is estimated, and may not be exact. See the
    /// comment about using `term_w` in `handle_key_base_view()` for more details.
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

    /// Handles key events for the message view.
    /// 
    /// Supports scrolling through the message body.
    fn handle_key_message_view(&mut self, key_event: KeyEvent) -> Result<(), Error> {
        match key_event.code {
            KeyCode::Esc => self.state = ActiveViewState::BaseView(BaseViewState::Inbox),
            KeyCode::Down => self.change_scroll(1)?,
            KeyCode::Up => self.change_scroll(-1)?,
            _ => {}
        }
        Ok(())
    }

    /// Handles the key events for the compose view. 
    fn handle_key_compose_view(&mut self, key_event: KeyEvent) -> Result<(), Error> {
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
            _ => {}
        }
        Ok(())
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Hovers the next email in the list
    pub fn hover_next_email(&mut self) {
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
    pub fn hover_previous_email(&mut self) {
        if let Some(index) = self.selected_email_index {
            if index > 0 {
                self.selected_email_index = Some(index - 1);
            }
        }
    }

    /// Handles the tick event of the terminal.
    /// 
    /// Anything that requires a fixed framerate will be put here.
    /// Also handles periodic email refresh (every 120 seconds).
    pub fn tick(&mut self) {
        self.tick_counter += 1;
        
        // Refresh emails every 120 seconds (30 FPS * 120 seconds = 3600 ticks)
        const REFRESH_INTERVAL: u64 = 3600;
        
        if self.tick_counter % REFRESH_INTERVAL == 0 {
            Self::spawn_email_fetch(
                Arc::clone(&self.backend),
                self.events.get_sender(),
                self.config.termail.email_fetch_count,
            );
        }
    }

    /// Spawns an async task to fetch emails from the backend.
    /// Results are sent back via the AppEvent::EmailsFetched event.
    /// 
    /// # Arguments
    /// * `backend` - Arc-wrapped backend for thread-safe access
    /// * `sender` - Event sender to send results back
    /// * `count` - Number of emails to fetch
    fn spawn_email_fetch(
        backend: Arc<Mutex<Box<dyn Backend>>>,
        sender: tokio::sync::mpsc::UnboundedSender<Event>,
        count: usize,
    ) {
        tokio::spawn(async move {
            // Acquire lock and fetch emails (no plugin manager needed for basic fetch)
            let result = {
                let backend_guard = backend.lock().await;
                backend_guard.do_command(Command::FetchInbox { count }, None).await
            };
            
            match result {
                Ok(CommandResult::Emails(emails)) => {
                    let _ = sender.send(Event::App(AppEvent::EmailsFetched(emails)));
                }
                Ok(CommandResult::Email(email)) => {
                    let _ = sender.send(Event::App(AppEvent::EmailsFetched(vec![email])));
                }
                Ok(CommandResult::Empty) => {
                    let _ = sender.send(Event::App(AppEvent::EmailsFetched(vec![])));
                }
                Err(e) => {
                    eprintln!("Failed to fetch emails: {}", e);
                }
                _ => {
                    eprintln!("Unexpected command result from fetch_inbox");
                }
            }
        });
    }

    fn spawn_label_fetch(
        backend: Arc<Mutex<Box<dyn Backend>>>,
        sender: tokio::sync::mpsc::UnboundedSender<Event>,
    ) {
        tokio::spawn(async move {
            let result = {
                let backend_guard = backend.lock().await;
                backend_guard.do_command(Command::ListLabels, None).await
            };

            match result {
                Ok(CommandResult::Labels(labels)) => {
                    let _ = sender.send(Event::App(AppEvent::LabelsFetched(labels)));
                }
                Err(e) => {
                    eprintln!("Failed to fetch labels: {}", e);
                },
                _ => eprintln!("Unexpected command result from list_labels"),
            }
        });
    }

}