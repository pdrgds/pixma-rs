mod escl;
mod translate;

use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::Router;
use tokio::sync::Mutex;

use escl::AppState;
use pixma_protocol::discover;

const ESCL_PORT: u16 = 8470;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    eprintln!("pixma-bridge: discovering Canon printer...");
    let printers = discover::find_printers(Duration::from_secs(10)).await?;
    let printer = printers
        .into_iter()
        .find(|p| p.scan_capable)
        .ok_or_else(|| anyhow::anyhow!("no scan-capable Canon printer found"))?;

    eprintln!("pixma-bridge: found {} at {}", printer.model, printer.ip);

    let (scan_done_tx, scan_done_rx) = tokio::sync::watch::channel(());

    let state = AppState {
        uuid: uuid::Uuid::new_v4().to_string(),
        printer_ip: printer.ip,
        scanning: Arc::new(AtomicBool::new(false)),
        active_job_id: Arc::new(Mutex::new(None)),
        jobs: Arc::new(Mutex::new(HashMap::new())),
        scan_done: scan_done_rx,
        scan_done_tx: Arc::new(scan_done_tx),
    };

    // Advertise eSCL service via macOS dns-sd (more reliable than mdns-sd crate)
    let _mdns_process = Command::new("dns-sd")
        .args([
            "-R",
            &printer.model,
            "_uscan._tcp",
            "local.",
            &ESCL_PORT.to_string(),
            "txtvers=1",
            "vers=2.0",
            &format!("ty={}", printer.model),
            "rs=eSCL",
            "pdl=image/jpeg",
            "cs=color,grayscale",
            "is=platen",
            &format!("uuid={}", state.uuid),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    eprintln!("pixma-bridge: advertising eSCL scanner on port {ESCL_PORT}");

    // Start HTTP server
    let app = Router::new()
        .route("/eSCL/ScannerCapabilities", get(escl::get_capabilities))
        .route("/eSCL/ScannerStatus", get(escl::get_status))
        .route("/eSCL/ScanJobs", post(escl::create_scan_job))
        .route(
            "/eSCL/ScanJobs/{job_id}/NextDocument",
            get(escl::get_next_document),
        )
        .route("/eSCL/ScanJobs/{job_id}", delete(escl::delete_scan_job))
        .with_state(state);

    // Catch-all fallback to log unhandled requests
    let app = app.fallback(|req: axum::extract::Request| async move {
        eprintln!("[escl] UNHANDLED: {} {}", req.method(), req.uri());
        StatusCode::NOT_FOUND
    });

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{ESCL_PORT}")).await?;
    eprintln!("pixma-bridge: eSCL server listening on port {ESCL_PORT}");

    axum::serve(listener, app).await?;
    Ok(())
}
