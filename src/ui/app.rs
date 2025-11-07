// This file contains the application logic for the termail UI.

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent}, DefaultTerminal
};

use crate::{types::{Command, EmailMessage}, ui::event::AppEvent};
use crate::config::Config;
use crate::error::Error;
use super::event::{Event, EventHandler};
use crate::backends::Backend;
use crate::types::CommandResult;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum ViewState {
    FolderView,
    InboxView { folder: String },
    MessageView { message: EmailMessage },
}

pub struct App {
    pub state: ViewState,
    pub running: bool,
    pub events: EventHandler, 
    pub config: Config,
    /// Email storage. None means not loaded yet, Some(vec![]) means loaded but empty.
    pub emails: Option<Vec<EmailMessage>>,
    /// Thread-safe backend for sharing across async tasks
    /// 
    /// We use this to allow multiple async tasks to access the backend concurrently. In 
    /// particular, we use it to fetch emails from the backend in a separate async task.
    pub backend: Arc<Mutex<Box<dyn Backend>>>,
    /// Counter to track ticks for periodic refresh (and other tasks)
    pub tick_counter: u64,
}

impl App {
    pub fn new(config: Config, backend: Box<dyn Backend>) -> Self {
        let backend = Arc::new(Mutex::new(backend));
        let events = EventHandler::new();
        
        // Spawn initial email fetch
        Self::spawn_email_fetch(
            Arc::clone(&backend),
            events.get_sender(),
            config.termail.email_fetch_count,
        );

        Self { 
            state: ViewState::FolderView, 
            running: true,
            events,
            config,
            emails: None,  // Start with None to indicate loading state
            backend,
            tick_counter: 0,
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
                    AppEvent::EmailsFetched(emails) => {
                        self.emails = Some(emails);
                    },
                    AppEvent::ChangeViewState(_) => {},
                }
            }
        }        
        Ok(())
    }
    
    pub fn handle_key_events(&mut self, key_event: KeyEvent) -> Result<(), Error> {
        match key_event.code {
            KeyCode::Esc => self.events.send(AppEvent::Quit),
            _ => {}
        }
        Ok(())
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    /// Handles the tick event of the terminal.
    /// 
    /// Anything that requires a fixed framerate will be put here.
    /// Also handles periodic email refresh (every 60 seconds).
    pub fn tick(&mut self) {
        self.tick_counter += 1;
        
        // Refresh emails every 60 seconds (30 FPS * 60 seconds = 1800 ticks)
        const REFRESH_INTERVAL: u64 = 1800;
        
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
            // Acquire lock and fetch emails
            let result = {
                let backend_guard = backend.lock().unwrap();
                backend_guard.do_command(Command::FetchInbox { count })
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

}