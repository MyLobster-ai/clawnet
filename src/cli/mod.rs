use clap::Parser;

#[derive(Parser)]
#[command(name = "clawnet", version, about = "P2P bot discovery for OpenClaw agents")]
pub struct Cli {
    /// Output in JSON format for machine parsing
    #[arg(long, global = true)]
    pub json: bool,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Show or generate bot identity (NodeId)
    Identity,

    /// One-shot peer discovery scan
    Discover {
        /// Discovery timeout in seconds
        #[arg(long, default_value = "10")]
        timeout: u64,

        /// Maximum number of peers to discover
        #[arg(long)]
        max_peers: Option<usize>,
    },

    /// List cached peers
    Peers {
        /// Only show peers seen recently
        #[arg(long)]
        online: bool,
    },

    /// Broadcast presence to the network
    Announce {
        /// Bot name to announce
        #[arg(long)]
        name: Option<String>,

        /// Comma-separated list of capabilities
        #[arg(long, value_delimiter = ',')]
        capabilities: Vec<String>,

        /// Duration to keep announcing (seconds)
        #[arg(long, default_value = "30")]
        duration: u64,
    },

    /// Direct QUIC connection to a peer
    Connect {
        /// Target node ID
        node_id: String,
    },

    /// Send a message to a peer
    Send {
        /// Target node ID
        node_id: String,

        /// Message to send
        message: String,
    },

    /// Run continuous discovery daemon
    Daemon {
        /// Announce interval in seconds
        #[arg(long, default_value = "60")]
        interval: u64,

        /// Run in foreground (default)
        #[arg(long, default_value = "true")]
        foreground: bool,
    },

    /// Show network and daemon status
    Status,

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(clap::Subcommand)]
pub enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Reset configuration to defaults
    Reset,
}

pub mod announce;
pub mod config_cmd;
pub mod connect;
pub mod daemon_cmd;
pub mod discover;
pub mod identity;
pub mod peers;
pub mod send;
pub mod status;
