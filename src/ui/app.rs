// This file contains the application logic for the termail UI.

use ratatui::{
    DefaultTerminal,
    crossterm::event::{KeyCode, KeyEvent}
};

use crate::{types::EmailMessage, ui::event::AppEvent};
use crate::config::Config;
use crate::error::Error;
use super::event::{Event, EventHandler};

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

    /// Handles the ticke event of the terminal.
    /// 
    /// Anything that requires a fixed framerate will be put here.
    pub fn tick(&self) {}

}