use crate::plugins::plugins::bindings;
use bindings::tm::plugin_system::event_api;

// Re-export the WIT event type for convenience
pub use event_api::Event as WitEvent;

/// The `Hook` enum represents the different events that can be triggered by the plugin.
/// It is what `serde` deserializes from the `hooks` field in the plugin manifest.
/// 
/// This is different from the `main.wit` file's `event` variant, which is what the plugin 
/// will receive when it is called by termail. 
#[derive(Debug, serde::Deserialize, Clone, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Hook {
	#[serde(rename = "before_send")]
    BeforeSend,
    #[serde(rename = "after_send")]
    AfterSend,
    #[serde(rename = "before_receive")]
    BeforeReceive,
    #[serde(rename = "after_receive")]
    AfterReceive,
}

/// Convert from `event_api::Event` (WIT type) to `Hook` (manifest/config type)
impl From<event_api::Event> for Hook {
    fn from(event: event_api::Event) -> Self {
        match event {
            event_api::Event::BeforeSend(_) => Hook::BeforeSend,
            event_api::Event::AfterSend(_) => Hook::AfterSend,
            event_api::Event::BeforeReceive(_) => Hook::BeforeReceive,
            event_api::Event::AfterReceive(_) => Hook::AfterReceive,
        }
    }   
}

/// Convert from `Hook` (manifest/config type) to `event_api::Event` (WIT type)
/// Note: This requires content, so we provide helper functions instead
impl Hook {
    /// Get the corresponding WIT event variant for a given hook and content
    pub fn to_wit_event(&self, content: String) -> event_api::Event {
        match self {
            Hook::BeforeSend => event_api::Event::before_send(content),
            Hook::AfterSend => event_api::Event::after_send(content),
            Hook::BeforeReceive => event_api::Event::before_receive(content),
            Hook::AfterReceive => event_api::Event::after_receive(content),
        }
    }
}

/// Helper functions to create WIT events from content
impl event_api::Event {
    /// Create a BeforeSend event with the given content
    pub fn before_send(content: String) -> Self {
        event_api::Event::BeforeSend ( event_api::BeforeSend {
            to: Some("".to_string()),
            fr_om: Some("".to_string()),
            content: Some(content),
            subject: Some("".to_string()),
        })
    }

    /// Create an AfterSend event with the given content
    pub fn after_send(content: String) -> Self {
        event_api::Event::AfterSend(event_api::AfterSend { 
            content: Some(content) 
        })
    }

    /// Create a BeforeReceive event with the given content
    pub fn before_receive(content: String) -> Self {
        event_api::Event::BeforeReceive(event_api::BeforeReceive { 
            content: Some(content) 
        })
    }

    /// Create an AfterReceive event with the given content
    pub fn after_receive(content: String) -> Self {
        event_api::Event::AfterReceive(event_api::AfterReceive { 
            content: Some(content) 
        })
    }

    /// Extract the content string from any event variant
   /* pub fn content(&self) -> &str {
        match self {
            event_api::Event::BeforeSend(content) => &content.content.unwrap(),
            event_api::Event::AfterSend(content) => &content.content.unwrap(),
            event_api::Event::BeforeReceive(content) => &content.content.unwrap(),
            event_api::Event::AfterReceive(content) => &(&content.content).unwrap(),
        }
    }*/

    /// Get the Hook variant that corresponds to this event
    pub fn hook(&self) -> Hook {
        match self {
            event_api::Event::BeforeSend(_) => Hook::BeforeSend,
            event_api::Event::AfterSend(_) => Hook::AfterSend,
            event_api::Event::BeforeReceive(_) => Hook::BeforeReceive,
            event_api::Event::AfterReceive(_) => Hook::AfterReceive,
        }
    }
}
