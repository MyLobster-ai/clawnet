use anyhow::Result;

use crate::output;
use crate::scanner::{self, ScanConfig};

pub async fn run(range: &str, timeout: u64, concurrency: usize, port: u16, json: bool) -> Result<()> {
    if !json {
        let net: ipnet::IpNet = range.parse()?;
        let host_count = net.hosts().count();
        eprintln!("Scanning {} ({} hosts)...", range, host_count);
    }

    let config = ScanConfig {
        timeout_ms: timeout,
        concurrency,
        port,
    };

    let (results, stats) = scanner::scan(range, config).await?;

    let entries: Vec<output::ScanResultEntry> = results
        .into_iter()
        .map(|r| output::ScanResultEntry {
            ip: r.ip.to_string(),
            node_id: r.node_id,
            name: r.name,
            version: r.version,
            capabilities: r.capabilities,
            quic_port: r.quic_port,
            rtt_ms: r.rtt_ms,
        })
        .collect();

    let out = output::ScanOutput {
        range: range.to_string(),
        results: entries,
        stats: output::ScanStatsOutput {
            total_ips: stats.total_ips,
            responses: stats.responses,
            duration_ms: stats.duration_ms,
        },
    };

    output::print(&out, json);
    Ok(())
}
