use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures_lite::StreamExt;
use iroh_gossip::api::Event;

use crate::config;
use crate::gossip;
use crate::node::ClawNode;
use crate::protocol::{self, BotAnnouncement, DirectMessage, GossipMessage, PeerInfo, MSG_ALPN};
use crate::store;

/// Shared daemon state for status queries.
pub struct DaemonState {
    pub running: AtomicBool,
    pub announcements_sent: AtomicU64,
    pub peers_discovered: AtomicU64,
    pub start_time: u64,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(true),
            announcements_sent: AtomicU64::new(0),
            peers_discovered: AtomicU64::new(0),
            start_time: protocol::now_secs(),
        }
    }
}

/// Run the continuous discovery daemon.
pub async fn run(interval_secs: u64) -> Result<()> {
    let cfg = config::load()?;
    let node = ClawNode::spawn().await?;
    let state = Arc::new(DaemonState::new());

    let node_id = node.endpoint.id().to_string();
    tracing::info!(node_id = %node_id, "daemon started");
    eprintln!("Daemon started. Node ID: {node_id}");
    eprintln!("Press Ctrl+C to stop.");

    let topic = gossip::subscribe(&node.gossip, vec![]).await?;
    let (sender, mut receiver) = topic.split();

    // Spawn message acceptor for direct connections
    let endpoint = node.endpoint.clone();
    let accept_state = state.clone();
    tokio::spawn(async move {
        accept_loop(endpoint, accept_state).await;
    });

    // Periodic announcement task
    let announce_sender = sender.clone();
    let announce_cfg = cfg.clone();
    let announce_node_id = node_id.clone();
    let announce_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            let ann = GossipMessage::Announce(BotAnnouncement {
                node_id: announce_node_id.clone(),
                name: announce_cfg.name.clone(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: announce_cfg.capabilities.clone(),
                openclaw_version: announce_cfg.openclaw_version.clone(),
                mode: announce_cfg.mode.clone(),
                timestamp: protocol::now_secs(),
                ttl: announce_cfg.peer_ttl,
                metadata: announce_cfg.metadata.clone(),
            });
            if let Err(e) = announce_sender.broadcast(ann.to_bytes().into()).await {
                tracing::warn!("failed to broadcast announcement: {e}");
            } else {
                announce_state
                    .announcements_sent
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    // Listen for incoming gossip messages
    let listen_state = state.clone();
    let listen_node_id = node_id.clone();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("\nShutting down...");
                // Send leave message
                let leave = GossipMessage::Leave {
                    node_id: listen_node_id.clone(),
                    timestamp: protocol::now_secs(),
                };
                let _ = sender.broadcast(leave.to_bytes().into()).await;
                break;
            }
            event = receiver.try_next() => {
                match event {
                    Ok(Some(Event::Received(msg))) => {
                        match GossipMessage::from_bytes(&msg.content) {
                            Ok(GossipMessage::Announce(ann)) => {
                                if ann.node_id == listen_node_id {
                                    continue;
                                }
                                tracing::info!(peer = %ann.node_id, name = %ann.name, "discovered peer");
                                eprintln!("Discovered: {} ({})", ann.name, &ann.node_id[..16.min(ann.node_id.len())]);
                                let peer = PeerInfo {
                                    node_id: ann.node_id.clone(),
                                    name: ann.name,
                                    capabilities: ann.capabilities,
                                    last_seen: protocol::now_secs(),
                                    ttl: ann.ttl,
                                    addresses: vec![],
                                    metadata: ann.metadata,
                                };
                                let _ = store::upsert(peer);
                                listen_state.peers_discovered.fetch_add(1, Ordering::Relaxed);
                            }
                            Ok(GossipMessage::Leave { node_id, .. }) => {
                                tracing::info!(peer = %node_id, "peer left");
                                eprintln!("Peer left: {}", &node_id[..16.min(node_id.len())]);
                            }
                            Err(e) => {
                                tracing::debug!("failed to parse gossip message: {e}");
                            }
                        }
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(e) => {
                        tracing::warn!("gossip receive error: {e}");
                        break;
                    }
                }
            }
        }
    }

    listen_state.running.store(false, Ordering::Relaxed);
    node.shutdown().await?;
    Ok(())
}

/// Accept incoming direct QUIC connections and handle messages.
async fn accept_loop(endpoint: iroh::Endpoint, _state: Arc<DaemonState>) {
    let node_id_str = endpoint.id().to_string();
    loop {
        let incoming = match endpoint.accept().await {
            Some(incoming) => incoming,
            None => break,
        };

        let my_id = node_id_str.clone();
        tokio::spawn(async move {
            let connection = match incoming.await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::debug!("failed to accept connection: {e}");
                    return;
                }
            };

            let alpn = connection.alpn();
            if &*alpn != MSG_ALPN {
                tracing::debug!(alpn = ?alpn, "unknown ALPN, ignoring");
                return;
            }

            match connection.accept_bi().await {
                Ok((mut send, mut recv)) => {
                    // Read length-prefixed message
                    let mut len_buf = [0u8; 4];
                    if recv.read_exact(&mut len_buf).await.is_err() {
                        return;
                    }
                    let len = u32::from_be_bytes(len_buf) as usize;
                    if len > 1024 * 1024 {
                        return;
                    }
                    let mut buf = vec![0u8; len];
                    if recv.read_exact(&mut buf).await.is_err() {
                        return;
                    }

                    if let Ok(msg) = DirectMessage::from_bytes(&buf) {
                        tracing::info!(from = %msg.from, "received direct message");
                        eprintln!(
                            "Message from {}: {}",
                            &msg.from[..16.min(msg.from.len())],
                            msg.content
                        );

                        // Send ack response
                        let ack = DirectMessage {
                            from: my_id.clone(),
                            content: "received".to_string(),
                            timestamp: protocol::now_secs(),
                        };
                        let ack_bytes = ack.to_bytes();
                        let _ = send.write_all(&(ack_bytes.len() as u32).to_be_bytes()).await;
                        let _ = send.write_all(&ack_bytes).await;
                        let _ = send.finish();
                    }
                }
                Err(e) => {
                    tracing::debug!("failed to accept stream: {e}");
                }
            }
        });
    }
}
