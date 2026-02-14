use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Wire format for gossip bot announcements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotAnnouncement {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub openclaw_version: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    pub timestamp: u64,
    pub ttl: u64,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Gossip message envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GossipMessage {
    Announce(BotAnnouncement),
    Leave {
        node_id: String,
        timestamp: u64,
    },
}

impl GossipMessage {
    /// Serialize to bytes for gossip wire format.
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("serialization failed")
    }

    /// Deserialize from gossip wire bytes.
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        postcard::from_bytes(bytes).map_err(Into::into)
    }
}

/// Cached peer record for the local peer store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub name: String,
    pub capabilities: Vec<String>,
    pub last_seen: u64,
    pub ttl: u64,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl PeerInfo {
    /// Check if this peer record has expired.
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now > self.last_seen + self.ttl
    }
}

/// Message sent over direct QUIC connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessage {
    pub from: String,
    pub content: String,
    pub timestamp: u64,
}

impl DirectMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("serialization failed")
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        postcard::from_bytes(bytes).map_err(Into::into)
    }
}

/// ALPN protocol identifier for direct messaging.
pub const MSG_ALPN: &[u8] = b"clawnet/msg/1";

/// Get the current unix timestamp.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
