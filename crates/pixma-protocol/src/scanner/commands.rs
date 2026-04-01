#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PixmaCmd {
    StartSession = 0xdb20,
    SelectSource = 0xdd20,
    Gamma = 0xee20,
    ScanParam3 = 0xd820,
    ScanStart3 = 0xd920,
    Status3 = 0xda20,
    ReadImage = 0xd420,
    CapabilityQuery = 0xf320,
    GetScanDimensions = 0xdc20,
    AbortSession = 0xef20,
    ErrorInfo = 0xff20,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixmaStatus {
    Ok,
    Busy,
    Failed,
    Unknown(u16),
}

impl From<u16> for PixmaStatus {
    fn from(val: u16) -> Self {
        match val {
            0x0606 => Self::Ok,
            0x1414 => Self::Busy,
            0x1515 => Self::Failed,
            other => Self::Unknown(other),
        }
    }
}

fn encode_dpi(dpi: u16) -> u16 {
    dpi | 0x8000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Color,
    Grayscale,
}

#[derive(Debug, Clone)]
pub struct ScanParams {
    pub dpi: u16,
    pub color: ColorMode,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ScanParams {
    /// Full A4 flatbed scan at the given DPI.
    pub fn a4(dpi: u16, color: ColorMode) -> Self {
        let w = (210.0 / 25.4 * dpi as f64) as u32;
        let h = (297.0 / 25.4 * dpi as f64) as u32;
        let w = (w + 31) & !31;
        Self {
            dpi,
            color,
            x: 0,
            y: 0,
            width: w,
            height: h,
        }
    }
}

const PIXMA_CMD_HEADER_LEN: usize = 16;

/// Build a pixma command. The param block includes a checksum as the
/// last byte — SANE includes the checksum WITHIN param_len, not after it.
fn build_command(cmd: PixmaCmd, params: &[u8]) -> Vec<u8> {
    let param_len = params.len() as u16;
    let total = PIXMA_CMD_HEADER_LEN + param_len as usize;

    let mut buf = vec![0u8; total];
    let cmd_bytes = (cmd as u16).to_be_bytes();
    buf[0] = cmd_bytes[0];
    buf[1] = cmd_bytes[1];
    buf[14..16].copy_from_slice(&param_len.to_be_bytes());

    if !params.is_empty() {
        buf[16..total].copy_from_slice(params);
        // Checksum: sum of first (n-1) param bytes; last byte = -sum
        let sum: u8 = buf[16..total - 1].iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        buf[total - 1] = 0u8.wrapping_sub(sum);
    }

    buf
}

pub fn parse_response_status(data: &[u8]) -> Result<PixmaStatus, crate::error::PixmaError> {
    if data.len() < 2 {
        return Err(crate::error::PixmaError::InvalidPacket(
            "pixma response too short".into(),
        ));
    }
    let status = u16::from_be_bytes([data[0], data[1]]);
    Ok(PixmaStatus::from(status))
}

pub fn cmd_start_session() -> Vec<u8> {
    let mut buf = vec![0u8; 16];
    buf[0] = 0xdb;
    buf[1] = 0x20;
    buf[2] = 0x00;
    buf[3] = 0x01;
    buf
}

pub fn cmd_abort_session() -> Vec<u8> {
    let mut buf = vec![0u8; 16];
    buf[0] = 0xef;
    buf[1] = 0x20;
    buf
}

/// 0xf320 capability query — param_len=0x0010 requests a 16-byte response.
pub fn cmd_capability_query() -> Vec<u8> {
    let mut buf = vec![0u8; 16];
    buf[0] = 0xf3;
    buf[1] = 0x20;
    buf[14] = 0x00;
    buf[15] = 0x10;
    buf
}

/// 0xdc20 get scan dimensions — param_len=0x0008 requests 8 bytes of dimension info.
pub fn cmd_get_scan_dimensions() -> Vec<u8> {
    let mut buf = vec![0u8; 16];
    buf[0] = 0xdc;
    buf[1] = 0x20;
    buf[14] = 0x00;
    buf[15] = 0x08;
    buf
}

pub fn cmd_select_source_flatbed() -> Vec<u8> {
    let mut data = [0u8; 12];
    data[0] = 1;
    data[1] = 1;
    build_command(PixmaCmd::SelectSource, &data)
}

pub fn cmd_scan_param_3(params: &ScanParams) -> Vec<u8> {
    let mut data = [0u8; 0x38];

    data[0x00] = 0x01; // flatbed
    data[0x01] = 0x01;
    data[0x02] = 0x01;

    let xdpi = encode_dpi(params.dpi);
    let ydpi = encode_dpi(params.dpi);
    data[0x08..0x0A].copy_from_slice(&xdpi.to_be_bytes());
    data[0x0A..0x0C].copy_from_slice(&ydpi.to_be_bytes());
    data[0x0C..0x10].copy_from_slice(&params.x.to_be_bytes());
    data[0x10..0x14].copy_from_slice(&params.y.to_be_bytes());
    data[0x14..0x18].copy_from_slice(&params.width.to_be_bytes());
    data[0x18..0x1C].copy_from_slice(&params.height.to_be_bytes());

    match params.color {
        ColorMode::Color => {
            data[0x1C] = 0x08;
            data[0x1D] = 24;
        }
        ColorMode::Grayscale => {
            data[0x1C] = 0x04;
            data[0x1D] = 8;
        }
    }

    data[0x1F] = 0x01;
    data[0x20] = 0xff;
    data[0x21] = 0x82;
    data[0x23] = 0x02;
    data[0x24] = 0x01;
    data[0x30] = 0x01;

    build_command(PixmaCmd::ScanParam3, &data)
}

pub fn cmd_scan_start_3() -> Vec<u8> {
    let mut buf = vec![0u8; 16];
    buf[0] = 0xd9;
    buf[1] = 0x20;
    buf[2] = 0x00;
    buf[3] = 0x01;
    buf
}

pub fn cmd_status_3() -> Vec<u8> {
    let mut buf = vec![0u8; 16];
    buf[0] = 0xda;
    buf[1] = 0x20;
    buf[14] = 0x00;
    buf[15] = 0x08;
    buf
}

/// Build a ReadImage command. `block_size_64k` is the block size in 64KB units
/// (e.g., 0x0020 = 32 * 64KB = 2MB max).
pub fn cmd_read_image(block_size_64k: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 16];
    buf[0] = 0xd4;
    buf[1] = 0x20;
    buf[12] = (block_size_64k >> 8) as u8;
    buf[13] = block_size_64k as u8;
    // bytes[14-15] = 0x0000 (param_len)
    buf
}

pub fn cmd_gamma_linear() -> Vec<u8> {
    let mut table = vec![0u8; 4096];
    for (i, val) in table.iter_mut().enumerate() {
        *val = (i >> 4) as u8;
    }
    build_command(PixmaCmd::Gamma, &table)
}

pub struct ImageBlock {
    pub status: PixmaStatus,
    pub end_of_scan: bool,
    pub data: Vec<u8>,
}

pub fn parse_image_block(response: &[u8]) -> Result<ImageBlock, crate::error::PixmaError> {
    if response.len() < 16 {
        return Err(crate::error::PixmaError::InvalidPacket(
            "image block response too short".into(),
        ));
    }

    let status = parse_response_status(response)?;
    let block_info = response[8];
    let end_of_scan = block_info & 0x20 != 0;
    let data_len = u32::from_be_bytes([response[12], response[13], response[14], response[15]]) as usize;

    let data = if data_len > 0 && response.len() >= 16 + data_len {
        response[16..16 + data_len].to_vec()
    } else {
        Vec::new()
    };

    Ok(ImageBlock {
        status,
        end_of_scan,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_checksum() {
        let cmd = cmd_select_source_flatbed();
        let param_len = u16::from_be_bytes([cmd[14], cmd[15]]) as usize;
        if param_len > 0 {
            let params = &cmd[16..16 + param_len];
            let sum: u8 = params.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
            assert_eq!(sum, 0, "checksum should make param bytes sum to 0");
        }
    }

    #[test]
    fn scan_param_3_encodes_dpi_with_flag() {
        let params = ScanParams::a4(300, ColorMode::Color);
        let cmd = cmd_scan_param_3(&params);
        let xdpi = u16::from_be_bytes([cmd[24], cmd[25]]);
        assert_eq!(xdpi, 300 | 0x8000);
    }

    #[test]
    fn scan_param_3_color_mode() {
        let color = cmd_scan_param_3(&ScanParams::a4(300, ColorMode::Color));
        assert_eq!(color[16 + 0x1C], 0x08);
        assert_eq!(color[16 + 0x1D], 24);

        let gray = cmd_scan_param_3(&ScanParams::a4(300, ColorMode::Grayscale));
        assert_eq!(gray[16 + 0x1C], 0x04);
        assert_eq!(gray[16 + 0x1D], 8);
    }

    #[test]
    fn a4_width_aligned_to_32() {
        let params = ScanParams::a4(300, ColorMode::Color);
        assert_eq!(params.width % 32, 0);
    }

    #[test]
    fn parse_ok_status() {
        let data = [0x06, 0x06, 0, 0, 0, 0, 0, 0];
        assert_eq!(parse_response_status(&data).unwrap(), PixmaStatus::Ok);
    }

    #[test]
    fn parse_busy_status() {
        let data = [0x14, 0x14, 0, 0, 0, 0, 0, 0];
        assert_eq!(parse_response_status(&data).unwrap(), PixmaStatus::Busy);
    }

    #[test]
    fn parse_image_block_end() {
        let mut response = vec![0u8; 20];
        response[0] = 0x06;
        response[1] = 0x06;
        response[8] = 0x28;
        response[12..16].copy_from_slice(&4u32.to_be_bytes());
        response[16..20].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

        let block = parse_image_block(&response).unwrap();
        assert!(block.end_of_scan);
        assert_eq!(block.data, vec![0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn parse_image_block_end_bit_0x20() {
        let mut response = vec![0u8; 20];
        response[0] = 0x06;
        response[1] = 0x06;
        response[8] = 0x20; // only bit 0x20 set
        response[12..16].copy_from_slice(&4u32.to_be_bytes());
        response[16..20].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);

        let block = parse_image_block(&response).unwrap();
        assert!(block.end_of_scan);
    }

    #[test]
    fn parse_image_block_not_end() {
        let mut response = vec![0u8; 20];
        response[0] = 0x06;
        response[1] = 0x06;
        response[8] = 0x08; // bit 0x20 not set
        response[12..16].copy_from_slice(&4u32.to_be_bytes());
        response[16..20].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);

        let block = parse_image_block(&response).unwrap();
        assert!(!block.end_of_scan);
    }

    #[test]
    fn start_session_format() {
        let cmd = cmd_start_session();
        assert_eq!(cmd.len(), 16);
        assert_eq!(cmd[0], 0xdb);
        assert_eq!(cmd[1], 0x20);
        assert_eq!(cmd[2], 0x00);
        assert_eq!(cmd[3], 0x01);
    }

    #[test]
    fn scan_start_3_format() {
        let cmd = cmd_scan_start_3();
        assert_eq!(cmd.len(), 16);
        assert_eq!(cmd[0], 0xd9);
        assert_eq!(cmd[1], 0x20);
        assert_eq!(cmd[2], 0x00);
        assert_eq!(cmd[3], 0x01);
    }

    #[test]
    fn status_3_format() {
        let cmd = cmd_status_3();
        assert_eq!(cmd.len(), 16);
        assert_eq!(cmd[0], 0xda);
        assert_eq!(cmd[1], 0x20);
        assert_eq!(cmd[14], 0x00);
        assert_eq!(cmd[15], 0x08);
    }

    #[test]
    fn read_image_format() {
        let cmd = cmd_read_image(0x0020);
        assert_eq!(cmd.len(), 16);
        assert_eq!(cmd[0], 0xd4);
        assert_eq!(cmd[1], 0x20);
        assert_eq!(cmd[12], 0x00);
        assert_eq!(cmd[13], 0x20);
        assert_eq!(cmd[14], 0x00);
        assert_eq!(cmd[15], 0x00);
    }

    #[test]
    fn capability_query_format() {
        let cmd = cmd_capability_query();
        assert_eq!(cmd.len(), 16);
        assert_eq!(cmd[0], 0xf3);
        assert_eq!(cmd[1], 0x20);
        assert_eq!(cmd[14], 0x00);
        assert_eq!(cmd[15], 0x10);
    }

    #[test]
    fn get_scan_dimensions_format() {
        let cmd = cmd_get_scan_dimensions();
        assert_eq!(cmd.len(), 16);
        assert_eq!(cmd[0], 0xdc);
        assert_eq!(cmd[1], 0x20);
        assert_eq!(cmd[14], 0x00);
        assert_eq!(cmd[15], 0x08);
    }

    #[test]
    fn gamma_table_linear() {
        let cmd = cmd_gamma_linear();
        let param_len = u16::from_be_bytes([cmd[14], cmd[15]]) as usize;
        assert_eq!(param_len, 4096);
    }
}
