use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::{Event as CrosstermEvent, EventStream};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::types::EmailMessage;
use crate::ui::app::ViewState;
use crate::error::Error;

const TICK_FPS: f64 = 30.0;

/// Terminal event.
#[derive(Clone, Debug)]
pub enum Event {
    /// A tick event emitted at a fixed rate.
    Tick,
    /// A crossterm event (like a key press)
    Crossterm(CrosstermEvent),
    /// An app event.
    App(AppEvent),
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    ChangeViewState(ViewState),
    EmailsFetched(Vec<EmailMessage>),
    Quit
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        // Spawn a new thread to handle events in the background by repeatedly ticking.
        tokio::spawn(async { actor.run().await });
        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> Result<Event, Error> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| Error::Other("Event channel closed".to_string()))
    }
    
    /// Get a sender handle that can be cloned and passed to async tasks
    pub fn get_sender(&self) -> mpsc::UnboundedSender<Event> {
        self.sender.clone()
    }

    /// Queue an app event to be sent to the event receiver.
    pub fn send(&mut self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(app_event));
    }
}

/// A thread that handles reading crossterm events and emitting tick events on a regular schedule.
struct EventTask {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    /// Constructs a new instance of [`EventThread`].
    fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender }
    }

    /// Runs the event thread.
    ///
    /// This function emits tick events at a fixed rate and polls for crossterm events in between.
    async fn run(self) {
        let tick_rate = Duration::from_secs_f64(1.0 / TICK_FPS);
        let mut reader = EventStream::new();
        let mut tick = tokio::time::interval(tick_rate);
        loop {
            let tick_delay = tick.tick();
            let crossterm_event = reader.next().fuse();
            tokio::select! {
              _ = self.sender.closed() => {
                break;
              }
              _ = tick_delay => {
                self.send(Event::Tick);
              }
              Some(Ok(evt)) = crossterm_event => {
                self.send(Event::Crossterm(evt));
              }
            };
        }
    }

    /// Sends an event to the receiver.
    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.sender.send(event);
    }
}