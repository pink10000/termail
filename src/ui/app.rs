// This file contains the application logic for the termail UI.

use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::DefaultTerminal;
use crate::cli::command::{Command, CommandResult};
use crate::core::{email::EmailMessage, label::Label, editor::Editor};
use crate::ui::{
    event::{AppEvent, Event, EventHandler},
    components::{composer_view::Composer, message_view::Messager},
};
use crate::config::Config;
use crate::error::Error;
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
                    AppEvent::SpawnEditor => {
                        if let ActiveViewState::ComposeView(composer) = &mut self.state {
                            let editor_cmd = self.config.termail.editor.clone();
                            let current_draft = composer.draft.clone();

                            // 1. Stop event polling
                            self.events.stop_events();

                            // 2. Suspend TUI
                            let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
                            let _ = disable_raw_mode();

                            // 3. Run editor
                            let result = Editor::open(&editor_cmd, current_draft);

                            // 4. Restore TUI
                            let _ = enable_raw_mode();
                            let _ = execute!(std::io::stdout(), EnterAlternateScreen);
                            terminal.clear()?;
                            self.events.start_events();

                            // 5. Update state
                            match result {
                                Ok(new_draft) => composer.draft = new_draft,
                                Err(e) => eprintln!("Editor error: {}", e),
                            }
                        }
                    },
                    AppEvent::SendEmail(email) => {
                        let backend = self.backend.lock().await;
                        let mut plugin_manager = self.plugin_manager.lock().await;

                        let result = backend.do_command(Command::SendEmail {
                            to: Some(email.to),
                            subject: Some(email.subject),
                            body: Some(email.body),
                        }, Some(&mut plugin_manager)).await?;

                        match result {
                            CommandResult::Empty => {
                                // TODO: some kind of status bar / message? maybe use the bottom bar?
                                println!("Email sent successfully!");
                            },
                            _ => return Err(Error::Other("Unexpected command result from send_email".to_string())),
                            
                        }
                    }
                    AppEvent::SyncFromCloud => {
                        // same here can add status bar to show sync progress
                        Self::spawn_sync_from_cloud(
                            Arc::clone(&self.backend),
                            self.events.get_sender(),
                            self.config.termail.email_fetch_count,
                        );
                    }
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

    /// Spawns an async task to sync emails from the cloud backend into the local maildir
    /// and then refresh the mailbox view.
    fn spawn_sync_from_cloud(
        backend: Arc<Mutex<Box<dyn Backend>>>,
        sender: tokio::sync::mpsc::UnboundedSender<Event>,
        count: usize,
    ) {
        tokio::spawn(async move {
            // start by syncing from cloud
            let sync_result = {
                let backend_guard = backend.lock().await;
                backend_guard.do_command(Command::SyncFromCloud, None)
                    .await
            };

            let result = match sync_result {
                Ok(_) => {
                    // after sync finishes, refresh the mailbox with view_mailbox
                    let backend_guard = backend.lock().await;
                    backend_guard
                        .do_command(Command::ViewMailbox { count }, None)
                        .await
                }
                Err(e) => {
                    eprintln!("Failed to sync from cloud: {}", e);
                    // bail out of this async task, return right away without refreshing the mailbox
                    return;
                }
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
                    eprintln!("Unexpected command result from view_mailbox");
                }
            }
        });
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
            // Acquire lock and fetch emails from maildir (no plugin manager needed for basic fetch)
            let result = {
                let backend_guard = backend.lock().await;
                backend_guard.do_command(Command::ViewMailbox { count }, None).await
                // backend_guard.do_command(Command::FetchInbox { count }, None).await
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
                    eprintln!("Unexpected command result from view_mailbox");
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