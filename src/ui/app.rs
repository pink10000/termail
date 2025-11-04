// This file contains the application logic for the termail UI.

use crate::types::EmailMessage;
use crate::config::Config;
use crate::error::Error;
use super::event::EventHandler;

#[derive(Clone, Debug)]
pub enum ViewState {
    FolderView,
    InboxView { folder: String },
    MessageView { message: EmailMessage },
}

pub struct App {
    pub state: ViewState,
    pub config: Config,
    pub running: bool,
    pub events: EventHandler, 
}

impl App {
    pub fn new(config: Config) -> Self {
        Self { 
            state: ViewState::FolderView, 
            config,
            running: true,
            events: EventHandler::new(),
        }
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        // TODO: Implement TUI rendering and event loop
        // For now, just a placeholder that exits immediately
        Ok(())
    }
}