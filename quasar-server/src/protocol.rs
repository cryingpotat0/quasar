use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum IncomingMessage {
    GenerateCode,
    Data { content: String },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum OutgoingMessage {
    GeneratedCode { code: String },
    Data { content: String },
    ConnectionInfo { id: usize, channel_uuid: Uuid },
    ClientConnected { id: usize },
    ClientDisconnected { id: usize },
}
