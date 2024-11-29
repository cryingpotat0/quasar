use crate::code_generator::{Code, CodeError, CodeGenerator};
use crate::protocol::OutgoingMessage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

type ClientSender = mpsc::Sender<warp::ws::Message>;

pub struct Channel {
    uuid: Uuid,
    // Consolidate the two below into a single struct/ object, it can be done better.
    clients: Arc<RwLock<HashMap<usize, ClientSender>>>,
    incrementing_id: Arc<Mutex<usize>>,
    pending_connects: Arc<Mutex<HashMap<u8, Code>>>,
}

impl Channel {
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            clients: Arc::new(RwLock::new(HashMap::new())),
            incrementing_id: Arc::new(Mutex::new(0)),
            pending_connects: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn client_ids(&self) -> Vec<usize> {
        self.clients.read().await.keys().cloned().collect()
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub async fn add_client(&self, client: ClientSender) -> usize {
        let next_id = {
            let mut id = self.incrementing_id.lock().unwrap();
            *id += 1;
            *id
        };
        let mut clients = self.clients.write().await;
        clients.insert(next_id, client);
        next_id
    }

    pub async fn remove_client(&self, sender_id: usize) {
        let mut clients = self.clients.write().await;
        clients.remove(&sender_id);
    }

    pub async fn broadcast(&self, message: OutgoingMessage) {
        let msg = serde_json::to_string(&message).unwrap();
        let clients = self.clients.read().await;
        for client in clients.values() {
            let _ = client.send(warp::ws::Message::text(msg.clone())).await;
        }
    }

    pub async fn send(&self, sender_id: usize, message: OutgoingMessage) {
        let msg = serde_json::to_string(&message).unwrap();
        let clients = self.clients.read().await;
        let client = clients.get(&sender_id).unwrap();
        let _ = client.send(warp::ws::Message::text(msg)).await;
    }
}

pub struct ChannelManager {
    channels: HashMap<Uuid, Arc<Channel>>,
    // The code generator is globally owned, but used locally by each channel.
    code_generator: Arc<Mutex<CodeGenerator>>,
    channel_number_to_channel: HashMap<u8, Arc<Channel>>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            code_generator: Arc::new(Mutex::new(CodeGenerator::new())),
            channel_number_to_channel: HashMap::new(),
        }
    }

    pub fn create_channel(&mut self) -> Arc<Channel> {
        let uuid = Uuid::new_v4();
        let channel = Arc::new(Channel::new(uuid));
        self.channels.insert(uuid, channel.clone());
        channel
    }

    pub fn get_channel(&self, uuid: &Uuid) -> Option<Arc<Channel>> {
        self.channels.get(uuid).cloned()
    }

    pub fn generate_code(&mut self, channel: Arc<Channel>) -> Result<Code, CodeError> {
        let code = self.code_generator.lock().unwrap().generate()?;

        self.channel_number_to_channel
            .insert(code.channel_number, channel.clone());
        channel
            .pending_connects
            .lock()
            .unwrap()
            .insert(code.channel_number, code.clone());
        Ok(code)
    }

    pub fn validate_code(&mut self, code: Code) -> Option<Arc<Channel>> {
        // Once someone requests a channel at a number, we won't track it any more.
        let channel = self
            .channel_number_to_channel
            .remove(&code.channel_number)?;

        let valid = channel
            .pending_connects
            .lock()
            .unwrap()
            .get(&code.channel_number)
            == Some(&code);

        // Don't allow another connection on this channel ID regardless of the validity.
        channel
            .pending_connects
            .lock()
            .unwrap()
            .remove(&code.channel_number);

        // Make the channel available again.
        self.code_generator
            .lock()
            .unwrap()
            .release(code.channel_number);

        if valid {
            Some(channel)
        } else {
            None
        }
    }
}
