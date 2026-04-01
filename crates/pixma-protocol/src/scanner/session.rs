use std::net::IpAddr;
use std::time::Duration;

use crate::chmp::ChmpConnection;
use crate::error::PixmaError;
use crate::scanner::commands::*;

/// Block size for ReadImage in 64KB units (0x0020 = 32 * 64KB = 2MB).
const READ_BLOCK_SIZE_64K: u16 = 0x0020;

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
/// Follows the exact Canon PRINT app scan sequence:
/// 1. Handshake x2
/// 2. ModeShift (blank jobID)
/// 3. 0xf320 capability query
/// 4. StartJob XML
/// 5. ModeShift (real jobID)
/// 6. 0xdb20 StartSession
/// 7. 0xf320 capability query
/// 8. 0xd820 ScanParam3
/// 9. 0xd920 ScanStart3
/// 10. 0xda20 Status3 poll (until byte[8]=0x03)
/// 11. 0xdc20 get dimensions
/// 12. 0xd420 ReadImage loop
/// 13. 0xef20 AbortSession
/// 14. EndJob XML
/// 15. Handshake cleanup
pub async fn scan(
    ip: IpAddr,
    params: &ScanParams,
) -> Result<ScanResult, PixmaError> {
    let mut conn = ChmpConnection::connect(ip, None).await?;

    // Step 1: CHMP handshake (2 rounds)
    conn.handshake().await?;
    conn.handshake().await?;

    // Step 2: ModeShift with blank jobID (pre-session)
    let resp = conn.exchange(XML_MODE_SHIFT_BLANK.as_bytes()).await?;
    check_xml_ok(&resp, "ModeShift (blank)")?;

    // Step 3: Capability query (0xf320)
    let cmd = cmd_capability_query();
    let _resp = conn.exchange(&cmd).await?;

    // Step 4: StartJob XML
    let resp = conn.exchange(XML_START_JOB.as_bytes()).await?;
    check_xml_ok(&resp, "StartJob")?;

    // Step 5: ModeShift with real jobID
    let resp = conn.exchange(XML_MODE_SHIFT.as_bytes()).await?;
    check_xml_ok(&resp, "ModeShift")?;

    // Step 6: StartSession (0xdb20)
    let cmd = cmd_start_session();
    let resp = conn.exchange(&cmd).await?;
    check_binary_ok(&resp, "StartSession")?;

    // Step 7: Capability query again (0xf320)
    let cmd = cmd_capability_query();
    let _resp = conn.exchange(&cmd).await?;

    // Step 8: ScanParam3 (0xd820)
    let cmd = cmd_scan_param_3(params);
    let resp = conn.exchange(&cmd).await?;
    check_binary_ok(&resp, "ScanParam3")?;

    // Step 9: ScanStart3 (0xd920)
    let cmd = cmd_scan_start_3();
    let resp = conn.exchange(&cmd).await?;
    check_binary_ok(&resp, "ScanStart3")?;

    // Step 10: Status3 poll (0xda20) — wait until byte[8] == 0x03 (data ready)
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let cmd = cmd_status_3();
        let resp = conn.exchange(&cmd).await?;
        if resp.len() >= 9 {
            let scan_status = resp[8];
            match scan_status {
                0x03 => break,               // data ready
                0x00 | 0x02 => continue,     // idle / scanning+calibrating
                other => {
                    return Err(PixmaError::Protocol(format!(
                        "unexpected status byte during poll: 0x{other:02x}"
                    )));
                }
            }
        }
    }

    // Step 11: Get scan dimensions (0xdc20)
    let cmd = cmd_get_scan_dimensions();
    let _dim_resp = conn.exchange(&cmd).await?;

    // Step 12: ReadImage loop (0xd420)
    let mut image_data = Vec::new();
    loop {
        let cmd = cmd_read_image(READ_BLOCK_SIZE_64K);
        let resp = conn.exchange(&cmd).await?;
        let block = parse_image_block(&resp)?;

        if block.status != PixmaStatus::Ok && block.status != PixmaStatus::Busy {
            return Err(PixmaError::ScanFailed(format!(
                "ReadImage: {:?}",
                block.status
            )));
        }

        image_data.extend_from_slice(&block.data);

        if block.end_of_scan {
            break;
        }
    }

    // Step 13: AbortSession (0xef20)
    let cmd = cmd_abort_session();
    let _ = conn.exchange(&cmd).await;

    // Step 14: EndJob XML
    let _ = conn.exchange(XML_END_JOB.as_bytes()).await;

    // Step 15: Handshake cleanup
    let _ = conn.handshake().await;

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
