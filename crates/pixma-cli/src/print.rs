use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use ipp::prelude::*;
use pixma_protocol::discover;

pub async fn run(file: String, device: Option<String>) -> Result<()> {
    let printer_uri = match device {
        Some(ip_str) => {
            let ip: IpAddr = ip_str.parse()?;
            format!("ipp://{}:631/ipp/print", ip)
        }
        None => {
            eprintln!("Searching for Canon printers...");
            let printers = discover::find_printers(Duration::from_secs(5)).await?;
            let printer = printers.first().ok_or_else(|| {
                anyhow::anyhow!("No Canon printer found. Use --device to specify an IP.")
            })?;
            eprintln!("Found: {} at {}", printer.model, printer.ip);
            format!("ipp://{}:631/ipp/print", printer.ip)
        }
    };

    let uri: Uri = printer_uri.parse()?;
    let payload = IppPayload::new(std::fs::File::open(&file)?);

    let username = std::env::var("USER").unwrap_or_else(|_| "pixma".into());
    let filename = Path::new(&file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file);

    let mime = match Path::new(&file).extension().and_then(|e| e.to_str()) {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    };

    let operation = IppOperationBuilder::print_job(uri.clone(), payload)
        .user_name(&username)
        .job_title(filename)
        .document_format(mime)
        .build();

    eprintln!("Printing {filename} to {printer_uri}...");
    let client = IppClient::new(uri);
    let response = client.send(operation)?;

    let status = response.header().status_code();
    if status.is_success() {
        eprintln!("Print job submitted successfully.");
    } else {
        anyhow::bail!("Print failed: {status}");
    }

    Ok(())
}
