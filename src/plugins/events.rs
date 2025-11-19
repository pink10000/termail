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

#[derive(Clone)]
pub struct Event<T> {
    pub data: T,
    pub triggers: &'static [Hook],
}

pub struct BeforeMessageSend {
    pub content: String
}

pub struct AfterMessageSend {
    pub content: String
}

impl Event<BeforeMessageSend> {
    pub fn new(content: String) -> Self {
        Event {
            data: BeforeMessageSend { content },
            triggers: &[Hook::BeforeSend],
        }
    }
}

impl Event<AfterMessageSend> {
    pub fn new(content: String) -> Self {
        Event {
            data: AfterMessageSend { content },
            triggers: &[Hook::AfterSend],
        }
    }
}
