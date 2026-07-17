use std::net::IpAddr;
use std::time::Duration;

use crate::chmp::ChmpConnection;
use crate::error::PixmaError;
use crate::scanner::commands::*;

/// Block size for ReadImage in 64KB units (0x0020 = 32 * 64KB = 2MB).
const READ_BLOCK_SIZE_64K: u16 = 0x0020;

/// Maximum time to wait for the scanner to report data-ready (0x03) during the
/// Status3 poll. A cold scanner can spend a while "warming up" (status 0x02),
/// but if it never advances we error out instead of hanging the caller — and
/// Image Capture — forever.
const STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(120);

const XML_START_JOB: &str = r#"<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"><ivec:contents><ivec:operation>StartJob</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID>00000001</ivec:jobID><ivec:bidi>1</ivec:bidi></ivec:param_set></ivec:contents></cmd>"#;

const XML_MODE_SHIFT: &str = r#"<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/" xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/"><ivec:contents><ivec:operation>VendorCmd</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID>00000001</ivec:jobID><vcn:ijoperation>ModeShift</vcn:ijoperation><vcn:ijmode>1</vcn:ijmode></ivec:param_set></ivec:contents></cmd>"#;

const XML_MODE_SHIFT_BLANK: &str = r#"<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/" xmlns:vcn="http://www.canon.com/ns/cmd/2008/07/canon/"><ivec:contents><ivec:operation>VendorCmd</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID> </ivec:jobID><vcn:ijoperation>ModeShift</vcn:ijoperation><vcn:ijmode>1</vcn:ijmode></ivec:param_set></ivec:contents></cmd>"#;

const XML_END_JOB: &str = r#"<?xml version="1.0" encoding="utf-8" ?><cmd xmlns:ivec="http://www.canon.com/ns/cmd/2008/07/common/"><ivec:contents><ivec:operation>EndJob</ivec:operation><ivec:param_set servicetype="scan"><ivec:jobID>00000001</ivec:jobID></ivec:param_set></ivec:contents></cmd>"#;

/// Raw scan result — contains JPEG data from the scanner.
pub struct ScanResult {
    /// JPEG image bytes (starts with `ff d8 ff e0`).
    pub data: Vec<u8>,
    /// True if data is JPEG-compressed (always true for CHMP scans).
    pub is_jpeg: bool,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
}

fn check_xml_ok(resp: &[u8], label: &str) -> Result<(), PixmaError> {
    let resp_str = String::from_utf8_lossy(resp);
    if !resp_str.contains("OK") {
        return Err(PixmaError::Protocol(format!("{label} rejected: {resp_str}")));
    }
    Ok(())
}

fn check_binary_ok(resp: &[u8], label: &str) -> Result<(), PixmaError> {
    let status = parse_response_status(resp)?;
    if status != PixmaStatus::Ok {
        return Err(PixmaError::Protocol(format!("{label}: {status:?}")));
    }
    Ok(())
}

/// Run a full flatbed scan over CHMP (HTTP) and return image bytes.
///
/// Follows the exact Canon PRINT app scan sequence (see docs/protocol.md):
/// 1. ModeShift (blank jobID)
/// 2. 0xf320 capability query
/// 3. StartJob XML
/// 4. ModeShift (real jobID)
/// 5. 0xdb20 StartSession
/// 6. 0xf320 capability query
/// 7. 0xd820 ScanParam3
/// 8. 0xd920 ScanStart3
/// 9. 0xda20 Status3 poll (until byte[8]=0x03)
/// 10. 0xdc20 get dimensions
/// 11. 0xd420 ReadImage loop
/// 12. 0xef20 AbortSession
/// 13. EndJob XML
pub async fn scan(
    ip: IpAddr,
    params: &ScanParams,
) -> Result<ScanResult, PixmaError> {
    eprintln!(
        "[scan] connecting to {ip} — {}dpi {:?} {}x{} at ({},{})",
        params.dpi, params.color, params.width, params.height, params.x, params.y
    );
    let mut conn = ChmpConnection::connect(ip, None).await?;

    // Step 1: ModeShift with blank jobID (pre-session)
    eprintln!("[scan] -> ModeShift (blank jobID)");
    let resp = conn.exchange(XML_MODE_SHIFT_BLANK.as_bytes()).await?;
    check_xml_ok(&resp, "ModeShift (blank)")?;

    // Step 2: Capability query (0xf320)
    eprintln!("[scan] -> capability query (0xf320)");
    let _resp = conn.exchange(&cmd_capability_query()).await?;

    // Step 3: StartJob XML
    eprintln!("[scan] -> StartJob");
    let resp = conn.exchange(XML_START_JOB.as_bytes()).await?;
    check_xml_ok(&resp, "StartJob")?;

    // Step 4: ModeShift with real jobID
    eprintln!("[scan] -> ModeShift (jobID=00000001)");
    let resp = conn.exchange(XML_MODE_SHIFT.as_bytes()).await?;
    check_xml_ok(&resp, "ModeShift")?;

    // Step 5: StartSession (0xdb20)
    eprintln!("[scan] -> StartSession (0xdb20)");
    let resp = conn.exchange(&cmd_start_session()).await?;
    check_binary_ok(&resp, "StartSession")?;

    // Step 6: Capability query again (0xf320)
    eprintln!("[scan] -> capability query (0xf320, 2nd)");
    let _resp = conn.exchange(&cmd_capability_query()).await?;

    // Step 7: ScanParam3 (0xd820)
    eprintln!("[scan] -> ScanParam3 (0xd820)");
    let resp = conn.exchange(&cmd_scan_param_3(params)).await?;
    check_binary_ok(&resp, "ScanParam3")?;

    // Step 8: ScanStart3 (0xd920)
    eprintln!("[scan] -> ScanStart3 (0xd920)");
    let resp = conn.exchange(&cmd_scan_start_3()).await?;
    check_binary_ok(&resp, "ScanStart3")?;

    // Step 9: Status3 poll (0xda20) — wait until byte[8] == 0x03 (data ready).
    // Bounded by STATUS_POLL_TIMEOUT so a scanner stuck at 0x00/0x02
    // ("warming up") surfaces an error instead of hanging forever.
    eprintln!("[scan] -> Status3 poll (0xda20), waiting for data-ready (0x03)");
    let poll_deadline = tokio::time::Instant::now() + STATUS_POLL_TIMEOUT;
    loop {
        if tokio::time::Instant::now() >= poll_deadline {
            return Err(PixmaError::ScanFailed(format!(
                "scanner never reported data-ready; stuck warming up after {}s",
                STATUS_POLL_TIMEOUT.as_secs()
            )));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let resp = conn.exchange(&cmd_status_3()).await?;
        if resp.len() < 9 {
            eprintln!(
                "[scan]    status: short response ({} bytes), retrying",
                resp.len()
            );
            continue;
        }
        let scan_status = resp[8];
        eprintln!("[scan]    status byte[8] = 0x{scan_status:02x}");
        match scan_status {
            0x03 => break,                      // data ready
            0x00 | 0x01 | 0x02 => continue,     // idle / preparing / scanning+calibrating
            // The reference capture only ever showed 0x00/0x02/0x03, but the real
            // G3010 also passes through 0x01 (and possibly other transient states)
            // mid-scan. Treating an unknown poll byte as fatal was itself a bug —
            // it failed the scan whenever the 500ms poll happened to sample 0x01.
            // Keep polling instead; a genuine stall is caught by STATUS_POLL_TIMEOUT.
            other => {
                eprintln!("[scan]    (unexpected status 0x{other:02x} — treating as in-progress)");
                continue;
            }
        }
    }

    // Step 10: Get scan dimensions (0xdc20)
    eprintln!("[scan] -> get dimensions (0xdc20)");
    let _dim_resp = conn.exchange(&cmd_get_scan_dimensions()).await?;

    // Step 11: ReadImage loop (0xd420)
    eprintln!("[scan] -> ReadImage loop (0xd420)");
    let mut image_data = Vec::new();
    loop {
        let resp = conn.exchange(&cmd_read_image(READ_BLOCK_SIZE_64K)).await?;
        let block = parse_image_block(&resp)?;

        if block.status != PixmaStatus::Ok && block.status != PixmaStatus::Busy {
            return Err(PixmaError::ScanFailed(format!(
                "ReadImage: {:?}",
                block.status
            )));
        }

        image_data.extend_from_slice(&block.data);
        eprintln!(
            "[scan]    read {} bytes (total {}), end_of_scan={}",
            block.data.len(),
            image_data.len(),
            block.end_of_scan
        );

        if block.end_of_scan {
            break;
        }
    }

    // Step 12: AbortSession (0xef20)
    eprintln!("[scan] -> AbortSession (0xef20)");
    let _ = conn.exchange(&cmd_abort_session()).await;

    // Step 13: EndJob XML
    eprintln!("[scan] -> EndJob");
    let _ = conn.exchange(XML_END_JOB.as_bytes()).await;

    eprintln!("[scan] complete: {} bytes of image data", image_data.len());

    let channels: u32 = match params.color {
        ColorMode::Color => 3,
        ColorMode::Grayscale => 1,
    };

    Ok(ScanResult {
        data: image_data,
        is_jpeg: true,
        width: params.width,
        height: params.height,
        channels,
    })
}
