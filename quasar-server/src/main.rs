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
use tracing::{debug, error, info, warn};
use warp::ws::{Message, WebSocket};
use warp::Filter;

mod protocol;
use protocol::{ClientMessage, ServerMessage};

const WORDLIST: &str = include_str!("wordlist.txt");

#[derive(Parser, Clone)]
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
use uuid::Uuid;

struct AppState {
    pending_channels: RwLock<HashMap<u32, PendingChannel>>,
    active_channels: RwLock<HashMap<Uuid, ChannelState>>,
    word_list: Vec<&'static str>,
}

struct PendingChannel {
    display_code: String,
    initiator: ConnectionState,
    created_at: std::time::Instant,
}

struct ChannelState {
    initiator: Option<ConnectionState>,
    responder: Option<ConnectionState>,
    created_at: std::time::Instant,
}

#[derive(Clone)]
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

impl ChannelState {
    fn new(initial_connection: ConnectionState) -> Self {
        Self {
            initiator: Some(initial_connection),
            responder: None,
            created_at: std::time::Instant::now(),
        }
    }

    fn is_full(&self) -> bool {
        self.initiator.is_some() && self.responder.is_some()
    }

    fn add_responder(&mut self, connection: ConnectionState) -> Result<(), &'static str> {
        if self.responder.is_some() {
            return Err("Channel already has a responder");
        }
        self.responder = Some(connection);
        Ok(())
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
        pending_channels: RwLock::new(HashMap::new()),
        active_channels: RwLock::new(HashMap::new()),
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

    tokio::task::spawn(receiver.forward(ws_sender).map(|result| {
        if let Err(e) = result {
            error!("Error sending websocket message: {}", e);
        }
    }));

    let connection = ConnectionState::new(sender.clone());

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
            let temp_conn = ConnectionState::new(sender.clone());
            send_error_and_close(&temp_conn, "Timeout waiting for initial message").await;
            return;
        }
    };

    let initial_text = match initial_msg.to_str() {
        Ok(text) => text,
        Err(_) => {
            let temp_conn = ConnectionState::new(sender.clone());
            send_error_and_close(&temp_conn, "Invalid message format").await;
            return;
        }
    };

    let initial_message: ClientMessage = match serde_json::from_str(initial_text) {
        Ok(msg) => msg,
        Err(_) => {
            send_error_and_close(&connection, "Invalid message format").await;
            return;
        }
    };

    let channel_uuid = match initial_message {
        ClientMessage::NewChannel => {
            handle_new_channel(&state, connection.clone()).await;
            Uuid::new_v4()
        }
        ClientMessage::Connect { code } => {
            handle_connect(&state, connection.clone(), &code).await;
            Uuid::new_v4()
        }
        _ => {
            send_error_and_close(&connection, "Invalid initial message type").await;
            return;
        }
    };

    // Main message loop
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) => {
                if let Err(e) = handle_message(msg, &channel_uuid, &state).await {
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
    cleanup_connection(&state, &channel_uuid).await;
}


async fn handle_message(
    msg: Message,
    channel_uuid: &Uuid,
    state: &Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let text = msg.to_str().map_err(|_| "Invalid message format")?;
    let client_msg: ClientMessage = serde_json::from_str(text)?;

    let mut active_channels = state.active_channels.write().await;
    let channel = active_channels
        .get_mut(channel_uuid)
        .ok_or("Channel not found")?;

    // Update last message timestamp for the sender
    let (sender_conn, receiver_conn) = {
        let (init, resp) = (channel.initiator.as_mut(), channel.responder.as_mut());
        match (init, resp) {
            (Some(i), Some(r)) => (i, r),
            _ => return Err("Incomplete channel state".into()),
        }
    };
    
    sender_conn.last_message = std::time::Instant::now();

    match client_msg {
        ClientMessage::Data { content } => {
            debug!("Received data message on channel {}", channel_uuid);
            // Check if both sides are ready
            if !sender_conn.ready || !receiver_conn.ready {
                send_error_and_close(sender_conn, "Data sent before connection ready").await;
                send_error_and_close(receiver_conn, "Data sent before connection ready").await;
                return Ok(());
            }

            // Forward data to the receiver
            let msg = ServerMessage::Data { content: content };
            send_message(&receiver_conn.sender, &msg)?;
        }
        ClientMessage::ConnectAck => {
            sender_conn.ready = true;
        }
        _ => {
            send_error_and_close(sender_conn, "Invalid message type for established connection").await;
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


async fn handle_new_channel(state: &Arc<AppState>, connection: ConnectionState) {
    let mut rng = ChaCha20Rng::from_entropy();
    
    // Generate channel ID and display code
    let channel_id: u32 = loop {
        let id = rng.gen_range(0..=9999);
        if !state.pending_channels.read().await.contains_key(&id) {
            break id;
        }
    };

    let words: Vec<&&'static str> = state.word_list.choose_multiple(&mut rng, 3).collect();
    let display_code = format!("{}-{}-{}-{}", channel_id, words[0], words[1], words[2]);

    // Create pending channel
    let pending = PendingChannel {
        display_code: display_code.clone(),
        initiator: connection.clone(),
        created_at: std::time::Instant::now(),
    };

    // Store pending channel
    state.pending_channels.write().await.insert(channel_id, pending);
    
    debug!("Created new pending channel with UUID: {}", channel_id);

    // Send channel created message
    let msg = ServerMessage::ChannelCreated { code: display_code };
    if let Err(e) = send_message(&connection.sender, &msg) {
        error!("Failed to send channel created message: {}", e);
    }
}

async fn handle_connect(state: &Arc<AppState>, connection: ConnectionState, code: &str) {
    // Find pending channel by display code
    let channel_id_opt = {
        let pending_channels = state.pending_channels.read().await;
        pending_channels
            .iter()
            .find(|(_, p)| p.display_code == code)
            .map(|(id, _)| *id)
    };

    let pending = match channel_id_opt {
        Some(id) => {
            state.pending_channels.write().await.remove(&id).unwrap()
        }
        None => {
            send_error_and_close(&connection, "Invalid channel code").await;
            return;
        }
    };

    // Create UUID for the active channel
    let channel_uuid = Uuid::new_v4();

    // Create active channel
    let (init_conn, resp_conn) = (pending.initiator, connection);
    
    // Store active channel
    let channel_state = ChannelState {
        initiator: Some(init_conn.clone()),
        responder: Some(resp_conn.clone()),
        created_at: std::time::Instant::now(),
    };
    
    state.active_channels.write().await.insert(channel_uuid, channel_state);
    
    info!("Channel {} activated with both peers connected", channel_uuid);

    // Send connected messages to both parties
    let msg = ServerMessage::Connected;
    if let Err(e) = send_message(&init_conn.sender, &msg) {
        error!("Failed to send connected message to initiator: {}", e);
    }
    if let Err(e) = send_message(&resp_conn.sender, &msg) {
        error!("Failed to send connected message to responder: {}", e);
    }
}

async fn cleanup_connection(state: &Arc<AppState>, channel_uuid: &Uuid) {
    if let Some(channel) = state.active_channels.write().await.remove(channel_uuid) {
        debug!("Cleaning up channel {}", channel_uuid);
        
        // Send disconnect message to both parties
        let msg = ServerMessage::Error {
            message: "Channel closed".to_string(),
        };
        if let Some(init) = &channel.initiator {
            if let Err(e) = send_message(&init.sender, &msg) {
                error!("Failed to send disconnect message to initiator: {}", e);
            }
            // Force close the websocket
            let _ = init.sender.send(Err(warp::Error::new("Connection closed")));
        }
        if let Some(resp) = &channel.responder {
            if let Err(e) = send_message(&resp.sender, &msg) {
                error!("Failed to send disconnect message to responder: {}", e);
            }
            // Force close the websocket
            let _ = resp.sender.send(Err(warp::Error::new("Connection closed")));
        }
    }
}

async fn reap_stale_connections(state: &Arc<AppState>) {
    // Clean up pending channels
    {
        let mut pending = state.pending_channels.write().await;
        let stale_pending: Vec<_> = pending
            .iter()
            .filter(|(_, channel)| channel.created_at.elapsed() > Duration::from_secs(60))
            .map(|(id, _)| *id)
            .collect();

        for id in stale_pending {
            warn!("Reaping stale pending channel: {}", id);
            if let Some(channel) = pending.remove(&id) {
                let msg = ServerMessage::Error {
                    message: "Channel timed out".to_string(),
                };
                if let Err(e) = send_message(&channel.initiator.sender, &msg) {
                    error!("Failed to send timeout message: {}", e);
                }
            }
        }
    }

    // Clean up active channels
    let active = state.active_channels.write().await;
    let stale_active: Vec<_> = active
        .iter()
        .filter(|(_, channel)| {
            channel.created_at.elapsed() > Duration::from_secs(300 * 1) || // 5 minute total lifetime
            channel.initiator.as_ref().map_or(true, |c| c.last_message.elapsed() > Duration::from_secs(60)) ||
            channel.responder.as_ref().map_or(true, |c| c.last_message.elapsed() > Duration::from_secs(60))
        })
        .map(|(uuid, _)| *uuid)
        .collect();

    for uuid in stale_active {
        warn!("Reaping stale active channel: {}", uuid);
        cleanup_connection(state, &uuid).await;
    }
}
async fn get_peer_connection<'a>(
    channel: &'a ChannelState,
    sender: &mpsc::UnboundedSender<Result<Message, warp::Error>>,
) -> Option<&'a ConnectionState> {
    if let (Some(init), Some(resp)) = (&channel.initiator, &channel.responder) {
        if std::ptr::eq(sender, &init.sender) {
            Some(resp)
        } else if std::ptr::eq(sender, &resp.sender) {
            Some(init)
        } else {
            None
        }
    } else {
        None
    }
}
