// crates/pixma-protocol/src/bjnp/udp.rs
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::net::UdpSocket;

use super::packet::{BjnpHeader, CommandCode, DeviceType};
use crate::error::PixmaError;

pub const BJNP_SCAN_PORT: u16 = 8612;

/// Info returned from a BJNP discovery probe.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub addr: SocketAddr,
    pub mac: [u8; 6],
}

/// Parse a BJNP discovery response payload.
fn parse_discover_response(buf: &[u8], source: SocketAddr) -> Result<DiscoveredDevice, PixmaError> {
    let header = BjnpHeader::from_bytes(buf)?;
    if !header.is_response || header.command != CommandCode::Discover {
        return Err(PixmaError::InvalidPacket("not a discover response".into()));
    }

    let payload = &buf[16..];
    if payload.len() < 16 {
        return Err(PixmaError::InvalidPacket("discover payload too short".into()));
    }

    let mac_len = payload[4] as usize;
    if mac_len != 6 || payload.len() < 6 + mac_len + 4 {
        return Err(PixmaError::InvalidPacket("unexpected mac/addr layout".into()));
    }

    let mut mac = [0u8; 6];
    mac.copy_from_slice(&payload[6..12]);

    Ok(DiscoveredDevice { addr: source, mac })
}

/// Send a BJNP discovery broadcast and collect responses.
pub async fn discover(timeout: Duration) -> Result<Vec<DiscoveredDevice>, PixmaError> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;

    let probe = BjnpHeader::new(DeviceType::Scan, CommandCode::Discover).to_bytes();
    let broadcast = SocketAddrV4::new(Ipv4Addr::BROADCAST, BJNP_SCAN_PORT);
    socket.send_to(&probe, broadcast).await?;

    let mut devices = Vec::new();
    let mut buf = [0u8; 1024];

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, src))) => {
                if let Ok(device) = parse_discover_response(&buf[..len], src) {
                    devices.push(device);
                }
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }

    Ok(devices)
}

/// Query a known device for its IEEE 1284 identity string.
pub async fn get_identity(addr: SocketAddr) -> Result<String, PixmaError> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;

    let probe = BjnpHeader::new(DeviceType::Scan, CommandCode::GetId).to_bytes();
    socket.send_to(&probe, addr).await?;

    let mut buf = [0u8; 2048];
    let (len, _) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buf))
        .await
        .map_err(|_| PixmaError::Timeout)?
        .map_err(PixmaError::Io)?;

    let header = BjnpHeader::from_bytes(&buf[..len])?;
    if !header.is_response || header.command != CommandCode::GetId {
        return Err(PixmaError::InvalidPacket("not a GetId response".into()));
    }

    let payload = &buf[16..len];
    if payload.len() < 2 {
        return Err(PixmaError::InvalidPacket("identity payload too short".into()));
    }
    let str_len = u16::from_be_bytes([payload[0], payload[1]]) as usize;
    let id_bytes = payload.get(2..2 + str_len).ok_or_else(|| {
        PixmaError::InvalidPacket("identity string length exceeds payload".into())
    })?;

    String::from_utf8(id_bytes.to_vec())
        .map_err(|e| PixmaError::InvalidPacket(format!("identity not UTF-8: {e}")))
}

/// Parse the MDL field from an IEEE 1284 identity string.
pub fn parse_model(identity: &str) -> Option<&str> {
    identity
        .split(';')
        .find_map(|field| field.strip_prefix("MDL:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_discover_response(mac: [u8; 6], ipv4: [u8; 4]) -> Vec<u8> {
        let mut header = BjnpHeader::new(DeviceType::Scan, CommandCode::Discover);
        header.is_response = true;
        header.payload_len = 16;

        let mut buf = Vec::new();
        buf.extend_from_slice(&header.to_bytes());
        buf.extend_from_slice(&[0x00, 0x01, 0x08, 0x00]);
        buf.push(6);
        buf.push(4);
        buf.extend_from_slice(&mac);
        buf.extend_from_slice(&ipv4);
        buf
    }

    #[test]
    fn parse_valid_discover_response() {
        let mac = [0x00, 0x18, 0x3b, 0x8d, 0x18, 0x12];
        let buf = make_discover_response(mac, [192, 168, 0, 9]);
        let src: SocketAddr = "192.168.0.9:8612".parse().unwrap();

        let device = parse_discover_response(&buf, src).unwrap();
        assert_eq!(device.mac, mac);
        assert_eq!(device.addr, src);
    }

    #[test]
    fn reject_non_response() {
        let header = BjnpHeader::new(DeviceType::Scan, CommandCode::Discover);
        let mut buf = vec![0u8; 32];
        buf[..16].copy_from_slice(&header.to_bytes());
        let src: SocketAddr = "192.168.0.9:8612".parse().unwrap();

        let err = parse_discover_response(&buf, src).unwrap_err();
        assert!(matches!(err, PixmaError::InvalidPacket(_)));
    }

    #[test]
    fn parse_model_from_identity() {
        let id = "MFG:Canon;CMD:BJRaster3,NCCe,IVEC;SOJ:CHMP;MDL:G3010 series;";
        assert_eq!(parse_model(id), Some("G3010 series"));
    }

    #[test]
    fn parse_model_missing() {
        let id = "MFG:Canon;CMD:BJRaster3;";
        assert_eq!(parse_model(id), None);
    }
}
