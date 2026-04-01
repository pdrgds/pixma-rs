// crates/pixma-cli/src/discover.rs
use std::time::Duration;

use anyhow::Result;
use pixma_protocol::discover;

pub async fn run(timeout: u64) -> Result<()> {
    let printers = discover::find_printers(Duration::from_secs(timeout)).await?;

    if printers.is_empty() {
        println!("No Canon printers found.");
        return Ok(());
    }

    for p in &printers {
        println!("{}", p.model);
        println!("  IP: {}", p.ip);
        if let Some(mac) = p.mac {
            let mac_str: Vec<String> = mac.iter().map(|b| format!("{b:02x}")).collect();
            println!("  MAC: {}", mac_str.join(":"));
        }
        println!("  Scan: {}", if p.scan_capable { "yes" } else { "no" });
        if let Some(id) = &p.identity {
            println!("  ID: {id}");
        }
        println!();
    }

    Ok(())
}
