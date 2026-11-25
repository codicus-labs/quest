//! `QuestCore` for the AI clients connected to the account: list and revoke, over the network.

use uuid::Uuid;

use crate::connections::{self, AiConnectionRow};
use crate::core::QuestCore;
use crate::error::MobileError;
use crate::guard::guard;
use crate::time::Now;
use crate::types::Moment;

#[uniffi::export]
impl QuestCore {
    /// The AI clients connected to the account (MCP), most recently connected first. Blocks on the
    /// network for up to about 20 s: never on the UI thread. The store lock is not held meanwhile.
    pub fn ai_connections(&self, now: Moment) -> Result<Vec<AiConnectionRow>, MobileError> {
        guard(|| {
            let now = Now::parse(&now)?;
            let client = self.store().connections();
            let list = client.list().map_err(connections::connections_error)?;
            Ok(connections::rows(&list, now))
        })
    }

    /// Revokes one connection: the client loses access at once and has to be connected again.
    /// Blocks on the network like `ai_connections`.
    pub fn revoke_ai_connection(&self, id: String) -> Result<(), MobileError> {
        guard(|| {
            let id = Uuid::parse_str(&id).map_err(|_| MobileError::InvalidId { value: id.clone() })?;
            let client = self.store().connections();
            client.revoke(id).map_err(connections::connections_error)
        })
    }
}
