use thiserror::Error;

#[derive(Error, Debug)]
pub enum QuasarError {
    #[error("Channel not found")]
    ChannelNotFound,

    #[error("Invalid connection code")]
    InvalidConnectionCode,

    #[error("WebSocket error: {0}")]
    WebSocketError(#[from] warp::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
