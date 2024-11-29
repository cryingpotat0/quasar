use crate::channel::{Channel, ChannelManager};
use crate::protocol::{IncomingMessage, OutgoingMessage, PROTOCOL_VERSION};
use futures::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::Instrument;
use tracing::{error, info, span, Level};
use uuid::Uuid;
use warp::ws::WebSocket;
use warp::Filter;

pub struct QuasarServer {
    addr: SocketAddr,
    channel_manager: Arc<Mutex<ChannelManager>>,
}

#[derive(serde::Deserialize)]
struct ConnectQuery {
    id: Option<Uuid>,
    code: Option<String>,
}

impl QuasarServer {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            channel_manager: Arc::new(Mutex::new(ChannelManager::new())),
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let channel_manager = self.channel_manager.clone();
        let channel_manager = warp::any().map(move || channel_manager.clone());

        let new_channel = warp::path!("ws" / "new")
            .and(warp::ws())
            .and(channel_manager.clone())
            .map(
                move |ws: warp::ws::Ws, channel_manager: Arc<Mutex<ChannelManager>>| {
                    let channel = channel_manager.lock().unwrap().create_channel();
                    ws.on_upgrade(move |socket| {
                        Self::handle_websocket(socket, channel, channel_manager)
                    })
                },
            );

        let connect_uuid = warp::path!("ws" / "connect")
            .and(warp::query::<ConnectQuery>())
            .and(warp::ws())
            .and(channel_manager)
            .map(
                move |query: ConnectQuery,
                      ws: warp::ws::Ws,
                      channel_manager: Arc<Mutex<ChannelManager>>| {
                    let channel_manager = channel_manager.clone();
                    ws.on_upgrade(move |socket| {
                        Self::handle_connect(socket, query, channel_manager)
                    })
                },
            );

        let routes = new_channel.or(connect_uuid);

        warp::serve(routes).run(self.addr).await;

        Ok(())
    }

    async fn handle_connect(
        ws: WebSocket,
        query: ConnectQuery,
        channel_manager: Arc<Mutex<ChannelManager>>,
    ) {
        if let Some(id) = query.id {
            let channel = channel_manager.lock().unwrap().get_channel(&id).unwrap();
            Self::handle_websocket(ws, channel, channel_manager).await;
        } else if let Some(code) = query.code {
            // TODO: move code parsing up the stack, and better error handling.
            let code = code.parse().unwrap();
            info!("Got code: {:?}", code);
            let channel = channel_manager.lock().unwrap().validate_code(code);
            if let Some(channel) = channel {
                Self::handle_websocket(ws, channel, channel_manager).await;
            } else {
                error!("Invalid code");
            }
        }
    }

    async fn handle_websocket(
        ws: WebSocket,
        channel: Arc<Channel>,
        channel_manager: Arc<Mutex<ChannelManager>>,
    ) {
        let (mut tx, mut rx) = ws.split();
        let (sender, mut receiver) = mpsc::channel(32);

        tokio::spawn(async move {
            while let Some(message) = receiver.recv().await {
                tx.send(message).await.unwrap();
            }
        });

        let sender_id = channel.add_client(sender).await;
        let span = span!(
            Level::INFO,
            "client_span",
            channel_id = channel.uuid().to_string(),
            user_id = sender_id
        );

        async {
            info!("Client connected");
            // First tell the client it's own ID.
            channel
                .send(
                    sender_id,
                    OutgoingMessage::ConnectionInfo {
                        id: sender_id,
                        channel_uuid: channel.uuid(),
                        client_ids: channel.client_ids().await,
                        protocol_version: PROTOCOL_VERSION,
                    },
                )
                .await;

            // Then tell everyone that the client has connected (including the client itself).
            channel
                .broadcast(OutgoingMessage::ClientConnected { id: sender_id })
                .await;

            while let Some(result) = rx.next().await {
                match result {
                    Ok(msg) => match serde_json::from_slice::<IncomingMessage>(&msg.as_bytes()) {
                        Ok(control_msg) => match control_msg {
                            IncomingMessage::GenerateCode => {
                                info!("Generating code");

                                let code = channel_manager
                                    .lock()
                                    .unwrap()
                                    .generate_code(channel.clone())
                                    .unwrap();
                                let response = OutgoingMessage::GeneratedCode {
                                    code: code.to_string(),
                                };
                                channel.send(sender_id, response).await;
                            }
                            IncomingMessage::Data { content } => {
                                channel.broadcast(OutgoingMessage::Data { content }).await;
                            }
                        },
                        Err(e) => {
                            error!("Error parsing message: {:?}", e);
                        }
                    },
                    Err(e) => {
                        error!("Error receiving message: {:?}", e);
                        break;
                    }
                }
            }

            channel.remove_client(sender_id).await;
            channel
                .broadcast(OutgoingMessage::ClientDisconnected { id: sender_id })
                .await;
            info!("Client removed");
            // TODO: cleanup channel.
        }
        .instrument(span)
        .await;
    }
}
