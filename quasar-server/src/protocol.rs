use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u64 = 1;

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
    GeneratedCode {
        code: String,
    },
    Data {
        content: String,
    },
    ConnectionInfo {
        id: usize,
        channel_uuid: Uuid,
        client_ids: Vec<usize>,
        protocol_version: u64,
    },
    ClientConnected {
        id: usize,
    },
    ClientDisconnected {
        id: usize,
    },
}
