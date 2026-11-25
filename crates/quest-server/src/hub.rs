//! Fan-out of accepted changes to every live connection of a user.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use quest_protocol::UserId;
use tokio::sync::broadcast;

use crate::repo::Sequenced;

/// Winning changes of one push, ascending by sequence number, never empty.
pub type LiveBatch = Arc<Vec<Sequenced>>;

const BATCHES_PER_USER: usize = 256;

#[derive(Clone)]
pub struct UserChannel {
    pub sender: broadcast::Sender<LiveBatch>,
    /// Held while a push commits and broadcasts, so batches go out in sequence order.
    pub write_lock: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Clone, Default)]
pub struct Hub {
    users: Arc<Mutex<HashMap<UserId, UserChannel>>>,
}

impl Hub {
    pub fn channel(&self, user: UserId) -> UserChannel {
        let mut users = self.users.lock().unwrap_or_else(PoisonError::into_inner);
        users.retain(|_, channel| channel.sender.receiver_count() > 0 || Arc::strong_count(&channel.write_lock) > 1);
        users
            .entry(user)
            .or_insert_with(|| UserChannel {
                sender: broadcast::Sender::new(BATCHES_PER_USER),
                write_lock: Arc::default(),
            })
            .clone()
    }

    pub fn subscribe(&self, user: UserId) -> broadcast::Receiver<LiveBatch> {
        self.channel(user).sender.subscribe()
    }
}
