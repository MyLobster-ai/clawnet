use anyhow::{Context, Result};
use iroh::EndpointId;

use crate::node::ClawNode;
use crate::output::{self, SendOutput};
use crate::protocol::{self, DirectMessage};

pub async fn run(node_id_str: &str, message: &str, json: bool) -> Result<()> {
    let node = ClawNode::spawn().await?;

    let target: EndpointId = node_id_str
        .parse()
        .context("invalid node ID")?;

    if !json {
        eprintln!("Sending message to {node_id_str}...");
    }

    let connection = node
        .endpoint
        .connect(target, protocol::MSG_ALPN)
        .await
        .context("failed to connect to peer")?;

    let (mut send_stream, mut recv_stream) = connection
        .open_bi()
        .await
        .context("failed to open bidirectional stream")?;

    let msg = DirectMessage {
        from: node.endpoint.id().to_string(),
        content: message.to_string(),
        timestamp: protocol::now_secs(),
    };

    let bytes = msg.to_bytes();
    let bytes_len = bytes.len();

    // Send length-prefixed message
    send_stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .await
        .context("failed to send message length")?;
    send_stream
        .write_all(&bytes)
        .await
        .context("failed to send message")?;
    send_stream.finish().context("failed to finish stream")?;

    // Try to read response
    let response = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        read_response(&mut recv_stream),
    )
    .await
    {
        Ok(Ok(resp)) => Some(resp),
        _ => None,
    };

    output::print(
        &SendOutput {
            status: "sent".to_string(),
            node_id: node_id_str.to_string(),
            bytes_sent: bytes_len,
            response,
        },
        json,
    );

    connection.close(0u32.into(), b"done");
    node.shutdown().await?;
    Ok(())
}

async fn read_response(recv: &mut iroh::endpoint::RecvStream) -> Result<String> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 1024 * 1024 {
        anyhow::bail!("response too large");
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?;
    let msg = DirectMessage::from_bytes(&buf)?;
    Ok(msg.content)
}
