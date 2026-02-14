use clap::Parser;
use tracing_subscriber::EnvFilter;

use clawnet::cli::{self, Command, ConfigAction};

#[tokio::main]
async fn main() {
    let args = cli::Cli::parse();

    // Initialize logging
    let filter = if args.verbose {
        EnvFilter::new("clawnet=debug,iroh=info,iroh_gossip=info")
    } else {
        EnvFilter::new("clawnet=warn")
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let result = run(args).await;
    if let Err(e) = result {
        clawnet::output::print_error(&e, false);
        std::process::exit(1);
    }
}

async fn run(args: cli::Cli) -> anyhow::Result<()> {
    let json = args.json;

    match args.command {
        Command::Identity => {
            cli::identity::run(json)?;
        }
        Command::Config { action } => match action {
            ConfigAction::Show => cli::config_cmd::show(json)?,
            ConfigAction::Set { key, value } => cli::config_cmd::set(&key, &value, json)?,
            ConfigAction::Reset => cli::config_cmd::reset(json)?,
        },
        Command::Discover {
            timeout,
            max_peers,
        } => {
            cli::discover::run(timeout, max_peers, json).await?;
        }
        Command::Announce {
            name,
            capabilities,
            duration,
        } => {
            cli::announce::run(name, capabilities, duration, json).await?;
        }
        Command::Peers { online } => {
            cli::peers::run(online, json)?;
        }
        Command::Connect { node_id } => {
            cli::connect::run(&node_id, json).await?;
        }
        Command::Send { node_id, message } => {
            cli::send::run(&node_id, &message, json).await?;
        }
        Command::Daemon {
            interval,
            foreground,
        } => {
            cli::daemon_cmd::run(interval, foreground, json).await?;
        }
        Command::Status => {
            cli::status::run(json)?;
        }
    }
    Ok(())
}
