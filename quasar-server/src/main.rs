use clap::Parser;
use futures::{FutureExt, StreamExt};
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;
use tracing::{error, info, warn};
use warp::ws::{Message, WebSocket};
use warp::Filter;

mod protocol;
use protocol::{ClientMessage, ServerMessage};

const WORDLIST: &str = include_str!("wordlist.txt");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to run the server on
    #[arg(short, long, default_value_t = 3030)]
    port: u16,

    /// Run in local mode (127.0.0.1)
    #[arg(short, long)]
    local: bool,
}

// Struct to hold our application state
struct AppState {
    channels: RwLock<HashMap<u32, String>>,
    connections: RwLock<HashMap<String, ConnectionState>>,
    word_list: Vec<&'static str>,
}

struct ConnectionState {
    sender: mpsc::UnboundedSender<Result<Message, warp::Error>>,
    last_message: std::time::Instant,
    ready: bool,
}

impl ConnectionState {
    fn new(sender: mpsc::UnboundedSender<Result<Message, warp::Error>>) -> Self {
        Self {
            sender,
            last_message: std::time::Instant::now(),
            ready: false,
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let word_list = WORDLIST.lines().collect::<Vec<&'static str>>();

    // Create app state
    let state = Arc::new(AppState {
        channels: RwLock::new(HashMap::new()),
        connections: RwLock::new(HashMap::new()),
        word_list,
    });

    // Define routes
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(with_state(state.clone()))
        .map(|ws: warp::ws::Ws, state| {
            ws.on_upgrade(move |socket| handle_websocket(socket, state))
        });

    // Start connection reaper task
    let reaper_state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            reap_stale_connections(&reaper_state).await;
        }
    });

    // Start the server
    // Determine the address to bind to
    let addr = if args.local {
        ([127, 0, 0, 1], args.port)
    } else {
        ([0, 0, 0, 0], args.port)
    };

    // Start the server
    warp::serve(ws_route).run(addr).await;
}

fn with_state(
    state: Arc<AppState>,
) -> impl Filter<Extract = (Arc<AppState>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || state.clone())
}

async fn handle_websocket(ws: WebSocket, state: Arc<AppState>) {
    let (ws_sender, mut ws_receiver) = ws.split();

    let (sender, receiver) = mpsc::unbounded_channel::<Result<Message, warp::Error>>();
    let receiver = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);

    // Handle incoming messages
    tokio::task::spawn(receiver.forward(ws_sender).map(|result| {
        if let Err(e) = result {
            error!("Error sending websocket message: {}", e);
        }
    }));

    // Wait for initial message with timeout
    let initial_msg = match timeout(Duration::from_secs(5), ws_receiver.next()).await {
        Ok(Some(Ok(msg))) => msg,
        Ok(Some(Err(e))) => {
            error!("WebSocket error during initial message: {}", e);
            return;
        }
        Ok(None) => {
            error!("WebSocket closed during initial message");
            return;
        }
        Err(_) => {
            send_error_and_close(&sender, "Timeout waiting for initial message").await;
            return;
        }
    };

    let initial_text = match initial_msg.to_str() {
        Ok(text) => text,
        Err(_) => {
            send_error_and_close(&sender, "Invalid message format").await;
            return;
        }
    };

    let initial_message: ClientMessage = match serde_json::from_str(initial_text) {
        Ok(msg) => msg,
        Err(_) => {
            send_error_and_close(&sender, "Invalid message format").await;
            return;
        }
    };

    let channel_code = match initial_message {
        ClientMessage::NewChannel => generate_channel_code(&state).await,
        ClientMessage::Connect { code } => {
            if !validate_channel_code(&code, &state).await {
                send_error_and_close(&sender, "Invalid channel code").await;
                return;
            }
            code
        }
        _ => {
            send_error_and_close(&sender, "Invalid initial message type").await;
            return;
        }
    };

    info!("Channel established: {}", channel_code);

    // Add connection to state
    let conn_state = ConnectionState::new(sender.clone());
    state
        .connections
        .write()
        .await
        .insert(channel_code.clone(), conn_state);

    // Send confirmation to client
    let response = match initial_message {
        ClientMessage::NewChannel => ServerMessage::ChannelCreated {
            code: channel_code.clone(),
        },
        ClientMessage::Connect { .. } => ServerMessage::Connected,
        _ => unreachable!(),
    };

    if let Err(e) = send_message(&sender, &response) {
        error!("Failed to send confirmation: {}", e);
        return;
    }

    // Main message loop
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) => {
                if let Err(e) = handle_message(msg, &channel_code, &state).await {
                    error!("Error handling message: {}", e);
                    break;
                }
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }

    // Clean up on disconnect
    cleanup_connection(&state, &channel_code).await;
}

async fn validate_channel_code(code: &str, state: &Arc<AppState>) -> bool {
    state.channels.read().await.values().any(|v| v == code)
}
async fn generate_channel_code(state: &Arc<AppState>) -> String {
    let mut rng = ChaCha20Rng::from_entropy();
    let channel_id: u32 = loop {
        let id = rng.gen_range(0..=9999);
        if !state.channels.read().await.contains_key(&id) {
            break id;
        }
    };

    let words: Vec<&&'static str> = state.word_list.choose_multiple(&mut rng, 3).collect();
    let code = format!("{}-{}-{}-{}", channel_id, words[0], words[1], words[2]);

    state
        .channels
        .write()
        .await
        .insert(channel_id, code.clone());
    code
}

async fn handle_message(
    msg: Message,
    channel_code: &str,
    state: &Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let text = msg.to_str()?;
    let client_msg: ClientMessage = serde_json::from_str(text)?;

    // Update last message timestamp
    if let Some(conn) = state.connections.write().await.get_mut(channel_code) {
        conn.last_message = std::time::Instant::now();
    }

    match client_msg {
        ClientMessage::Data { content } => {
            let mut connections = state.connections.read().await;
            let current_conn = connections
                .get(channel_code)
                .ok_or("Connection not found")?;

            // Check if both sides are ready
            if !current_conn.ready {
                send_error_and_close_all(
                    &connections,
                    channel_code,
                    "Data sent before connection ready",
                )
                .await;
                return Ok(());
            }

            // Forward data to the other connection
            for (other_code, other_conn) in connections.iter() {
                if other_code != channel_code && other_conn.ready {
                    let msg = ServerMessage::Data { content };
                    send_message(&other_conn.sender, &msg)?;
                }
            }
        }
        ClientMessage::ConnectAck => {
            if let Some(conn) = state.connections.write().await.get_mut(channel_code) {
                conn.ready = true;
            }
        }
        _ => {
            send_error_and_close(
                state.connections.read().await.get(channel_code).unwrap(),
                "Invalid message type for established connection",
            )
            .await;
        }
    }
    Ok(())
}

fn send_message(
    sender: &mpsc::UnboundedSender<Result<Message, warp::Error>>,
    msg: &ServerMessage,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string(msg)?;
    sender.send(Ok(Message::text(json)))?;
    Ok(())
}

async fn send_error_and_close(conn: &ConnectionState, error_msg: &str) {
    let msg = ServerMessage::Error {
        message: error_msg.to_string(),
    };
    if let Err(e) = send_message(&conn.sender, &msg) {
        error!("Failed to send error message: {}", e);
    }
}

async fn send_error_and_close_all(
    connections: &HashMap<String, ConnectionState>,
    channel_code: &str,
    error_msg: &str,
) {
    for (code, conn) in connections.iter() {
        if code == channel_code || connections.values().any(|c| c.ready) {
            send_error_and_close(conn, error_msg).await;
        }
    }
}

async fn cleanup_connection(state: &Arc<AppState>, channel_code: &str) {
    let mut connections = state.connections.write().await;
    connections.remove(channel_code);

    // If this was a channel owner, clean up the channel
    if let Some((channel_id, _)) = state
        .channels
        .read()
        .await
        .iter()
        .find(|(_, v)| **v == channel_code)
    {
        state.channels.write().await.remove(channel_id);
    }

    // Disconnect the other side if it exists
    for (other_code, other_conn) in connections.iter() {
        if other_code != channel_code {
            let msg = ServerMessage::Error {
                message: "Other party disconnected".to_string(),
            };
            if let Err(e) = send_message(&other_conn.sender, &msg) {
                error!("Failed to send disconnect message: {}", e);
            }
        }
    }
}

async fn reap_stale_connections(state: &Arc<AppState>) {
    let mut to_remove = Vec::new();
    let connections = state.connections.read().await;

    for (code, conn) in connections.iter() {
        if conn.last_message.elapsed() > Duration::from_secs(60) {
            to_remove.push(code.clone());
        }
    }
    drop(connections);

    for code in to_remove {
        warn!("Reaping stale connection: {}", code);
        cleanup_connection(state, &code).await;
    }
}
