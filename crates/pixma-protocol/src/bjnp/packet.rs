//! BJNP protocol constants and packet header.
//!
//! The BJNP protocol wraps USB commands in UDP/TCP packets.
//! All multi-byte fields are big-endian.

const BJNP_MAGIC: [u8; 4] = *b"BJNP";
pub const HEADER_SIZE: usize = 16;

/// Device type occupies the high nibble; bit 7 marks a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeviceType {
    Print = 0x01,
    Scan = 0x02,
}

/// Command codes sent over UDP or TCP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandCode {
    Discover = 0x01,
    JobDetails = 0x10,
    Close = 0x11,
    TcpRead = 0x20,
    TcpSend = 0x21,
    GetId = 0x30,
}

/// A 16-byte BJNP packet header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BjnpHeader {
    pub device_type: DeviceType,
    pub is_response: bool,
    pub command: CommandCode,
    pub seq_no: u16,
    pub session_id: u16,
    pub payload_len: u32,
}

impl BjnpHeader {
    pub fn new(device_type: DeviceType, command: CommandCode) -> Self {
        Self {
            device_type,
            is_response: false,
            command,
            seq_no: 0,
            session_id: 0,
            payload_len: 0,
        }
    }

    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(&BJNP_MAGIC);
        let dev = self.device_type as u8 | if self.is_response { 0x80 } else { 0 };
        buf[4] = dev;
        buf[5] = self.command as u8;
        // buf[6..8] = 0 (unknown1)
        buf[8..10].copy_from_slice(&self.seq_no.to_be_bytes());
        buf[10..12].copy_from_slice(&self.session_id.to_be_bytes());
        buf[12..16].copy_from_slice(&self.payload_len.to_be_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::error::PixmaError> {
        if buf.len() < HEADER_SIZE {
            return Err(crate::error::PixmaError::InvalidPacket(
                format!("header too short: {} bytes", buf.len()),
            ));
        }
        if buf[0..4] != BJNP_MAGIC {
            return Err(crate::error::PixmaError::InvalidPacket(
                format!("bad magic: {:02x?}", &buf[0..4]),
            ));
        }

        let raw_dev = buf[4];
        let is_response = raw_dev & 0x80 != 0;
        let device_type = match raw_dev & 0x7F {
            0x01 => DeviceType::Print,
            0x02 => DeviceType::Scan,
            other => {
                return Err(crate::error::PixmaError::InvalidPacket(
                    format!("unknown device type: 0x{other:02x}"),
                ))
            }
        };

        let command = match buf[5] {
            0x01 => CommandCode::Discover,
            0x10 => CommandCode::JobDetails,
            0x11 => CommandCode::Close,
            0x20 => CommandCode::TcpRead,
            0x21 => CommandCode::TcpSend,
            0x30 => CommandCode::GetId,
            other => {
                return Err(crate::error::PixmaError::InvalidPacket(
                    format!("unknown command: 0x{other:02x}"),
                ))
            }
        };

        let seq_no = u16::from_be_bytes([buf[8], buf[9]]);
        let session_id = u16::from_be_bytes([buf[10], buf[11]]);
        let payload_len = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);

        Ok(Self {
            device_type,
            is_response,
            command,
            seq_no,
            session_id,
            payload_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_discover_header() {
        let header = BjnpHeader {
            device_type: DeviceType::Scan,
            is_response: false,
            command: CommandCode::Discover,
            seq_no: 0,
            session_id: 0,
            payload_len: 0,
        };

        let bytes = header.to_bytes();
        assert_eq!(&bytes[0..4], b"BJNP");
        assert_eq!(bytes[4], 0x02); // Scan, not response
        assert_eq!(bytes[5], 0x01); // Discover
        assert_eq!(bytes.len(), 16);

        let parsed = BjnpHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, header);
    }

    #[test]
    fn parse_scan_response() {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(b"BJNP");
        bytes[4] = 0x82; // Scan response (0x02 | 0x80)
        bytes[5] = 0x01; // Discover
        bytes[12..16].copy_from_slice(&42u32.to_be_bytes());

        let header = BjnpHeader::from_bytes(&bytes).unwrap();
        assert!(header.is_response);
        assert_eq!(header.device_type, DeviceType::Scan);
        assert_eq!(header.payload_len, 42);
    }

    #[test]
    fn reject_bad_magic() {
        let bytes = [0u8; 16];
        let err = BjnpHeader::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, crate::error::PixmaError::InvalidPacket(_)));
    }

    #[test]
    fn reject_short_buffer() {
        let bytes = [0u8; 8];
        let err = BjnpHeader::from_bytes(&bytes).unwrap_err();
        assert!(matches!(err, crate::error::PixmaError::InvalidPacket(_)));
    }

    #[test]
    fn header_with_session_and_seq() {
        let header = BjnpHeader {
            device_type: DeviceType::Scan,
            is_response: false,
            command: CommandCode::TcpSend,
            seq_no: 42,
            session_id: 7,
            payload_len: 1024,
        };

        let bytes = header.to_bytes();
        let parsed = BjnpHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.seq_no, 42);
        assert_eq!(parsed.session_id, 7);
        assert_eq!(parsed.payload_len, 1024);
    }
}
