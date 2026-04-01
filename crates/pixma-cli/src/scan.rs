use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Result};
use pixma_protocol::discover;
use pixma_protocol::scanner::commands::{ColorMode, ScanParams};
use pixma_protocol::scanner::image::{OutputFormat, save_scan};
use pixma_protocol::scanner::session;

pub async fn run(
    output: PathBuf,
    resolution: u16,
    color: String,
    format: Option<String>,
    device: Option<String>,
) -> Result<()> {
    let ip: IpAddr = match device {
        Some(ip_str) => ip_str.parse()?,
        None => {
            eprintln!("Searching for Canon printers...");
            let printers = discover::find_printers(Duration::from_secs(5)).await?;
            let printer = printers.first().ok_or_else(|| {
                anyhow::anyhow!("No Canon printer found. Use --device to specify an IP.")
            })?;
            if !printer.scan_capable {
                bail!("Printer {} does not advertise scan capability", printer.model);
            }
            eprintln!("Found: {} at {}", printer.model, printer.ip);
            printer.ip
        }
    };

    let color_mode = match color.as_str() {
        "grayscale" | "gray" => ColorMode::Grayscale,
        _ => ColorMode::Color,
    };

    let params = ScanParams::a4(resolution, color_mode);

    let out_format = match format.as_deref() {
        Some("jpeg" | "jpg") => OutputFormat::Jpeg,
        Some("png") => OutputFormat::Png,
        None => OutputFormat::from_extension(&output),
        Some(other) => bail!("Unsupported format: {other}"),
    };

    eprintln!("Scanning at {} DPI ({:?}) via CHMP...", resolution, color_mode);
    let result = session::scan(ip, &params).await?;
    eprintln!(
        "Received {} bytes ({} x {} pixels)",
        result.data.len(),
        result.width,
        result.height
    );

    save_scan(&result, &output, out_format)?;
    eprintln!("Saved to {}", output.display());

    Ok(())
}
