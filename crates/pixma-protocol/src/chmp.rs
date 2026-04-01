//! CHMP (Canon Home Management Protocol) HTTP transport.
//!
//! CHMP wraps the pixma binary command protocol in HTTP POST/GET cycles
//! on port 80. Each command is sent as a POST, and the response is
//! retrieved with a subsequent GET. Both use `application/octet-stream`.

use std::net::IpAddr;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::error::PixmaError;

/// A CHMP HTTP connection to a Canon scanner.
pub struct ChmpConnection {
    stream: BufReader<TcpStream>,
    host: String,
    path: String,
}

impl ChmpConnection {
    /// Connect to a Canon scanner's CHMP endpoint.
    pub async fn connect(ip: IpAddr, path: Option<&str>) -> Result<Self, PixmaError> {
        let addr = format!("{ip}:80");
        let stream = TcpStream::connect(&addr).await?;
        stream.set_nodelay(true)?;

        Ok(Self {
            stream: BufReader::new(stream),
            host: ip.to_string(),
            path: path.unwrap_or("/canon/ij/command2/port3").to_string(),
        })
    }

    /// Send data via HTTP POST, then receive response via HTTP GET.
    pub async fn exchange(&mut self, data: &[u8]) -> Result<Vec<u8>, PixmaError> {
        self.post(data).await?;
        self.get().await
    }

    async fn post(&mut self, body: &[u8]) -> Result<(), PixmaError> {
        let request = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nX-CHMP-Version: 1.4.0\r\nX-CHMP-Timeout: 20\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: Keep-Alive\r\n\r\n",
            self.path, self.host, body.len()
        );

        self.stream.get_mut().write_all(request.as_bytes()).await?;
        self.stream.get_mut().write_all(body).await?;
        self.stream.get_mut().flush().await?;

        let status = self.read_status_line().await?;
        let (content_length, _chunked) = self.read_headers().await?;

        if content_length > 0 {
            let mut discard = vec![0u8; content_length];
            self.stream.read_exact(&mut discard).await?;
        }

        if !status.contains("200") {
            return Err(PixmaError::Protocol(format!("POST failed: {status}")));
        }

        Ok(())
    }

    async fn get(&mut self) -> Result<Vec<u8>, PixmaError> {
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/octet-stream\r\nConnection: Keep-Alive\r\nX-CHMP-Version: 1.4.0\r\n\r\n",
            self.path, self.host
        );

        self.stream.get_mut().write_all(request.as_bytes()).await?;
        self.stream.get_mut().flush().await?;

        let status = self.read_status_line().await?;
        if !status.contains("200") {
            return Err(PixmaError::Protocol(format!("GET failed: {status}")));
        }

        let (content_length, chunked) = self.read_headers().await?;

        if chunked {
            self.read_chunked_body().await
        } else if content_length > 0 {
            let mut body = vec![0u8; content_length];
            self.stream.read_exact(&mut body).await?;
            Ok(body)
        } else {
            Ok(Vec::new())
        }
    }

    async fn read_status_line(&mut self) -> Result<String, PixmaError> {
        let mut line = String::new();
        self.stream.read_line(&mut line).await?;
        Ok(line.trim().to_string())
    }

    async fn read_headers(&mut self) -> Result<(usize, bool), PixmaError> {
        let mut content_length = 0usize;
        let mut chunked = false;

        loop {
            let mut line = String::new();
            self.stream.read_line(&mut line).await?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            let lower = trimmed.to_lowercase();
            if lower.starts_with("content-length:") {
                if let Some(val) = lower.strip_prefix("content-length:") {
                    content_length = val.trim().parse().unwrap_or(0);
                }
            } else if lower.contains("transfer-encoding") && lower.contains("chunked") {
                chunked = true;
            }
        }

        Ok((content_length, chunked))
    }

    async fn read_chunked_body(&mut self) -> Result<Vec<u8>, PixmaError> {
        let mut body = Vec::new();

        loop {
            let mut size_line = String::new();
            self.stream.read_line(&mut size_line).await?;
            let size_str = size_line.trim();
            if size_str.is_empty() {
                continue;
            }

            let chunk_size = usize::from_str_radix(size_str, 16).map_err(|e| {
                PixmaError::Protocol(format!("bad chunk size '{size_str}': {e}"))
            })?;

            if chunk_size == 0 {
                let mut trail = String::new();
                let _ = self.stream.read_line(&mut trail).await;
                break;
            }

            let mut chunk = vec![0u8; chunk_size];
            self.stream.read_exact(&mut chunk).await?;
            body.extend_from_slice(&chunk);

            let mut trail = String::new();
            self.stream.read_line(&mut trail).await?;
        }

        Ok(body)
    }

    /// Perform the 1-byte handshake that starts a CHMP session.
    pub async fn handshake(&mut self) -> Result<(), PixmaError> {
        let _resp = self.exchange(&[0x00]).await?;
        Ok(())
    }
}
