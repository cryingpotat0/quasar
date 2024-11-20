use clap::Parser;
use futures::{FutureExt, StreamExt};
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::error;
use tracing::info;
use warp::ws::{Message, WebSocket};
use warp::Filter;

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
    connections: RwLock<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>,
    word_list: Vec<&'static str>,
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
        .and(warp::query::<HashMap<String, String>>())
        .and(with_state(state.clone()))
        .map(|ws: warp::ws::Ws, params: HashMap<String, String>, state| {
            ws.on_upgrade(move |socket| handle_websocket(socket, params, state))
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

async fn handle_websocket(ws: WebSocket, params: HashMap<String, String>, state: Arc<AppState>) {
    let (ws_sender, mut ws_receiver) = ws.split();

    let (sender, receiver) = mpsc::unbounded_channel::<Result<Message, warp::Error>>();
    let receiver = tokio_stream::wrappers::UnboundedReceiverStream::new(receiver);

    // Handle incoming messages
    tokio::task::spawn(receiver.forward(ws_sender).map(|result| {
        if let Err(e) = result {
            error!("Error sending websocket message: {}", e);
        }
    }));

    // Generate or validate channel code
    let channel_code = match params.get("code") {
        Some(code) => {
            if validate_channel_code(code, &state).await {
                code.to_string()
            } else {
                send_error_and_close(&sender, "Invalid channel code").await;
                return;
            }
        }
        None => generate_channel_code(&state).await,
    };

    info!("Created channel: {}", channel_code);

    // Add connection to state
    state
        .connections
        .write()
        .await
        .insert(channel_code.clone(), sender.clone());

    // Send channel code to client
    if sender
        .send(Ok(Message::text(channel_code.clone())))
        .is_err()
    {
        error!("Failed to send channel code to client");
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
    state.connections.write().await.remove(&channel_code);
    if let Some((channel_id, _)) = state
        .channels
        .read()
        .await
        .iter()
        .find(|(_, v)| **v == channel_code)
    {
        state.channels.write().await.remove(channel_id);
    }
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
    if let Ok(text) = msg.to_str() {
        let connections = state.connections.read().await;
        if let Some(sender) = connections.get(channel_code) {
            sender.send(Ok(Message::text(text)))?;
        }
    }
    Ok(())
}

async fn send_error_and_close(
    sender: &mpsc::UnboundedSender<Result<Message, warp::Error>>,
    error_msg: &str,
) {
    if let Err(e) = sender.send(Ok(Message::text(format!("ERROR: {}", error_msg)))) {
        error!("Failed to send error message: {}", e);
    }
}
