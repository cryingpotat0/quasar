use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "new_channel")]
    NewChannel,
    #[serde(rename = "connect")]
    Connect { code: String },
    #[serde(rename = "connect_ack")]
    ConnectAck,
    #[serde(rename = "data")]
    Data { content: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "channel_created")]
    ChannelCreated { code: String },
    #[serde(rename = "connected")]
    Connected,
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "data")]
    Data { content: String },
}
