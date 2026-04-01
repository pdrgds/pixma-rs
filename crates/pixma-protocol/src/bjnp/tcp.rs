use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::packet::{BjnpHeader, CommandCode, DeviceType, HEADER_SIZE};
use crate::error::PixmaError;

/// A BJNP TCP connection to a scanner.
pub struct BjnpTcp {
    stream: TcpStream,
    session_id: u16,
    seq_no: u16,
}

impl BjnpTcp {
    pub async fn connect(addr: SocketAddr, session_id: u16) -> Result<Self, PixmaError> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            session_id,
            seq_no: 0,
        })
    }

    fn next_seq(&mut self) -> u16 {
        let seq = self.seq_no;
        self.seq_no = self.seq_no.wrapping_add(1);
        seq
    }

    /// Send data to the scanner (CMD_TCP_SEND = 0x21).
    pub async fn send(&mut self, data: &[u8]) -> Result<(), PixmaError> {
        let mut header = BjnpHeader::new(DeviceType::Scan, CommandCode::TcpSend);
        header.seq_no = self.next_seq();
        header.session_id = self.session_id;
        header.payload_len = data.len() as u32;

        self.stream.write_all(&header.to_bytes()).await?;
        self.stream.write_all(data).await?;
        Ok(())
    }

    /// Read data from the scanner (CMD_TCP_READ = 0x20).
    pub async fn read(&mut self, max_len: u32) -> Result<Vec<u8>, PixmaError> {
        let mut header = BjnpHeader::new(DeviceType::Scan, CommandCode::TcpRead);
        header.seq_no = self.next_seq();
        header.session_id = self.session_id;
        header.payload_len = 4;

        self.stream.write_all(&header.to_bytes()).await?;
        self.stream.write_all(&max_len.to_be_bytes()).await?;

        let mut resp_buf = [0u8; HEADER_SIZE];
        self.stream.read_exact(&mut resp_buf).await?;
        let resp_header = BjnpHeader::from_bytes(&resp_buf)?;

        if resp_header.payload_len == 0 {
            return Ok(Vec::new());
        }

        let mut payload = vec![0u8; resp_header.payload_len as usize];
        self.stream.read_exact(&mut payload).await?;
        Ok(payload)
    }

    /// Send data and read the response in one transaction.
    pub async fn transaction(&mut self, data: &[u8], read_len: u32) -> Result<Vec<u8>, PixmaError> {
        self.send(data).await?;
        self.read(read_len).await
    }
}
