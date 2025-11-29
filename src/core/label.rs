use serde::{Deserialize, Serialize};
use google_gmail1::api::LabelColor;

/// The `google_gmail1::api::Label` has its own Label type, but we're wrapping 
/// it in our own type for consistency.
/// 
/// We reuse some of the fields from the `google_gmail1::api::Label` type, but not all of them.
/// We reuse the `LabelColor` enum from the `google_gmail1::api::LabelColor` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub color: Option<LabelColor>,
    /// The immutable ID of the label.
    pub id: Option<String>,
    /// The total number of messages with the label.
    #[serde(rename = "messagesTotal")]
    pub messages_total: Option<usize>,
    /// The number of unread messages with the label.
    #[serde(rename = "messagesUnread")]
    pub messages_unread: Option<usize>,
    /// The display name of the label.
    pub name: Option<String>,
}

impl Label {
    pub fn new() -> Self {
        Self {
            color: None,
            id: None,
            messages_total: None,
            messages_unread: None,
            name: None,
        }
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Name: {:?}\n\tColor: {:?}\n\tID: {:?}\n\tMessages Total: {:?}\n\tMessages Unread: {:?}",
            self.name.as_ref(),
            self.color.as_ref(),
            self.id.as_ref(),
            self.messages_total.as_ref(),
            self.messages_unread.as_ref()
        )
    }
}