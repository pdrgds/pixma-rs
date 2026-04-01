use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use tokio::sync::{watch, Mutex};
use uuid::Uuid;

use pixma_protocol::scanner::session;

use crate::translate;

/// Shared daemon state.
#[derive(Clone)]
pub struct AppState {
    pub uuid: String,
    pub printer_ip: IpAddr,
    pub scanning: Arc<AtomicBool>,
    pub active_job_id: Arc<Mutex<Option<String>>>,
    /// Completed scan data keyed by job ID.
    pub jobs: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    /// Notifies waiters when a scan completes.
    pub scan_done: watch::Receiver<()>,
    pub scan_done_tx: Arc<watch::Sender<()>>,
}

const SCANNER_CAPABILITIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScannerCapabilities xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03"
                          xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
  <pwg:Version>2.0</pwg:Version>
  <pwg:MakeAndModel>Canon G3010 series</pwg:MakeAndModel>
  <scan:UUID>{uuid}</scan:UUID>
  <scan:Platen>
    <scan:PlatenInputCaps>
      <scan:MinWidth>0</scan:MinWidth>
      <scan:MinHeight>0</scan:MinHeight>
      <scan:MaxWidth>2550</scan:MaxWidth>
      <scan:MaxHeight>3508</scan:MaxHeight>
      <scan:MaxPhysicalWidth>2550</scan:MaxPhysicalWidth>
      <scan:MaxPhysicalHeight>3508</scan:MaxPhysicalHeight>
      <scan:MaxScanRegions>1</scan:MaxScanRegions>
      <scan:SettingProfiles>
        <scan:SettingProfile>
          <scan:ColorModes>
            <scan:ColorMode>RGB24</scan:ColorMode>
            <scan:ColorMode>Grayscale8</scan:ColorMode>
          </scan:ColorModes>
          <scan:DocumentFormats>
            <pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat>
          </scan:DocumentFormats>
          <scan:SupportedResolutions>
            <scan:DiscreteResolutions>
              <scan:DiscreteResolution><scan:XResolution>75</scan:XResolution><scan:YResolution>75</scan:YResolution></scan:DiscreteResolution>
              <scan:DiscreteResolution><scan:XResolution>150</scan:XResolution><scan:YResolution>150</scan:YResolution></scan:DiscreteResolution>
              <scan:DiscreteResolution><scan:XResolution>300</scan:XResolution><scan:YResolution>300</scan:YResolution></scan:DiscreteResolution>
              <scan:DiscreteResolution><scan:XResolution>600</scan:XResolution><scan:YResolution>600</scan:YResolution></scan:DiscreteResolution>
            </scan:DiscreteResolutions>
          </scan:SupportedResolutions>
        </scan:SettingProfile>
      </scan:SettingProfiles>
      <scan:SupportedIntents>
        <scan:SupportedIntent>Preview</scan:SupportedIntent>
        <scan:SupportedIntent>TextAndGraphic</scan:SupportedIntent>
        <scan:SupportedIntent>Photo</scan:SupportedIntent>
      </scan:SupportedIntents>
    </scan:PlatenInputCaps>
  </scan:Platen>
</scan:ScannerCapabilities>"#;

fn scanner_status_xml(state: &str, job_id: Option<&str>) -> String {
    let jobs_xml = match (state, job_id) {
        ("Processing", Some(id)) => format!(
            r#"
  <scan:Jobs>
    <scan:JobInfo>
      <pwg:JobUri>/eSCL/ScanJobs/{id}</pwg:JobUri>
      <pwg:JobUuid>{id}</pwg:JobUuid>
      <scan:Age>0</scan:Age>
      <pwg:JobState>Processing</pwg:JobState>
      <pwg:ImagesCompleted>0</pwg:ImagesCompleted>
    </scan:JobInfo>
  </scan:Jobs>"#
        ),
        _ => String::new(),
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScannerStatus xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03"
                    xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
  <pwg:Version>2.0</pwg:Version>
  <pwg:State>{state}</pwg:State>{jobs_xml}
</scan:ScannerStatus>"#
    )
}

fn xml_response(body: String) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=UTF-8")],
        body,
    )
        .into_response()
}

pub async fn get_capabilities(State(state): State<AppState>) -> Response {
    eprintln!("[escl] GET /eSCL/ScannerCapabilities");
    let xml = SCANNER_CAPABILITIES.replace("{uuid}", &state.uuid);
    xml_response(xml)
}

pub async fn get_status(State(state): State<AppState>) -> Response {
    let scanning = state.scanning.load(std::sync::atomic::Ordering::Relaxed);
    let status = if scanning { "Processing" } else { "Idle" };
    let job_id = state.active_job_id.lock().await.clone();
    eprintln!("[escl] GET /eSCL/ScannerStatus -> {status}");
    xml_response(scanner_status_xml(status, job_id.as_deref()))
}

pub async fn create_scan_job(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Response {
    let settings = translate::parse_scan_settings(&body);
    let params = settings.to_scan_params();
    let job_id = Uuid::new_v4().to_string();
    eprintln!(
        "[escl] POST /eSCL/ScanJobs -> {}dpi {:?} {}x{} -> job {job_id}",
        settings.x_resolution, settings.color_mode, settings.width, settings.height
    );

    state
        .scanning
        .store(true, std::sync::atomic::Ordering::Relaxed);
    *state.active_job_id.lock().await = Some(job_id.clone());

    let job_id_clone = job_id.clone();
    let state_clone = state.clone();

    tokio::spawn(async move {
        match session::scan(state_clone.printer_ip, &params).await {
            Ok(result) => {
                eprintln!("[escl] scan complete, {} bytes", result.data.len());
                state_clone
                    .jobs
                    .lock()
                    .await
                    .insert(job_id_clone, result.data);
            }
            Err(e) => {
                eprintln!("[escl] scan failed: {e}");
            }
        }
        state_clone
            .scanning
            .store(false, std::sync::atomic::Ordering::Relaxed);
        *state_clone.active_job_id.lock().await = None;
        // Wake up anyone waiting for scan data
        let _ = state_clone.scan_done_tx.send(());
    });

    (
        StatusCode::CREATED,
        [(header::LOCATION, format!("/eSCL/ScanJobs/{job_id}"))],
        "",
    )
        .into_response()
}

pub async fn get_next_document(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Response {
    eprintln!("[escl] GET /eSCL/ScanJobs/{job_id}/NextDocument");

    // If data is already ready, return it
    if let Some(data) = state.jobs.lock().await.remove(&job_id) {
        eprintln!("[escl] returning {} bytes of JPEG data", data.len());
        return jpeg_response(data);
    }

    // If scan is in progress, wait for the completion signal
    if state.scanning.load(std::sync::atomic::Ordering::Relaxed) {
        let mut rx = state.scan_done.clone();
        // Mark current value as seen so changed() only fires on NEW sends
        rx.borrow_and_update();
        eprintln!("[escl] waiting for scan to complete...");
        // Wait up to 5 minutes for the scan to finish
        if tokio::time::timeout(std::time::Duration::from_secs(300), rx.changed())
            .await
            .is_ok()
            && let Some(data) = state.jobs.lock().await.remove(&job_id)
        {
            eprintln!("[escl] returning {} bytes of JPEG data", data.len());
            return jpeg_response(data);
        }
    }

    // No data — no more pages
    eprintln!("[escl] no more pages for job {job_id}");
    StatusCode::NOT_FOUND.into_response()
}

fn jpeg_response(data: Vec<u8>) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/jpeg")],
        Body::from(data),
    )
        .into_response()
}

pub async fn delete_scan_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> StatusCode {
    eprintln!("[escl] DELETE /eSCL/ScanJobs/{job_id}");
    state.jobs.lock().await.remove(&job_id);
    StatusCode::OK
}
