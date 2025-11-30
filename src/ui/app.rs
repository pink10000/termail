// This file contains the application logic for the termail UI.

use ratatui::DefaultTerminal;
use crate::cli::command::{Command, CommandResult};
use crate::core::{email::EmailMessage, label::Label};
use crate::ui::event::{AppEvent, Event, EventHandler};
use crate::config::Config;
use crate::error::Error;
use crate::backends::Backend;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::plugins::plugins::PluginManager;
use crate::ui::components::{composer_view::Composer, message_view::Messager};

#[derive(Clone, Debug, Copy)]
pub enum BaseViewState {
    Labels,
    Inbox,
}

#[derive(Clone, Debug)]
pub enum ActiveViewState {
    /// This state holds the base view of the application, which is the sidebar 
    /// with labels, and the inbox view. 
    BaseView(BaseViewState),
    /// This state indicates that the user is viewing a single email message.
    MessageView(Messager),
    /// This state indicates that the user is writing a new email message.
    ComposeView(Composer),
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

    pub fn quit(&mut self) {
        self.running = false;
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