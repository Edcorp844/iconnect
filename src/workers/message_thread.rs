use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Thread {
    pub key: String,
    #[serde(default)]
    pub name: String,
    pub last_ts: String,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub starred: bool,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub recipients: Vec<String>,
    #[serde(default)]
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Message {
    pub body: String,
    pub timestamp: String,
    pub outgoing: bool,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub body_truncated: bool,
}

impl Thread {
    pub fn display_name(&self) -> &str {
        if !self.name.is_empty() {
            &self.name
        } else if let Some(first) = self.recipients.first() {
            first.as_str()
        } else {
            &self.key
        }
    }

    pub fn preview(&self) -> &str {
        self.messages
            .last()
            .map(|m| m.body.as_str())
            .unwrap_or("")
    }
}