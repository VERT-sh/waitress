use bytestring::ByteString;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "camelCase")]
pub enum WebsocketMessage {
    Ping,
    LogMessage(String),
}

impl Into<ByteString> for WebsocketMessage {
    fn into(self) -> ByteString {
        serde_json::to_string(&self)
            .expect("Failed to serialize WebsocketMessage")
            .into()
    }
}
