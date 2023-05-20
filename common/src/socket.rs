use super::proto::Rcvr;
use serde_json::error::Category;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};

const DEFAULT_BUFFER_SIZE: usize = 1024;

const NEWLINE: u8 = b'\n';

#[derive(Debug)]
pub enum SocketErrorKind {
    SetNoDelayFailed,
    SetNonBlockingFailed,
    ShutdownFailed,
    ReadFailed,
    SyntaxError,
    WriteFailed,
    FlushFailed,
    GetRemoteAddressFailed,
}

#[derive(Debug)]
pub struct SocketError {
    kind: SocketErrorKind,
    message: String,
}

impl SocketError {
    pub fn new(kind: SocketErrorKind, message: String) -> Self {
        Self { kind, message }
    }

    fn from_err(kind: SocketErrorKind, e: impl std::error::Error) -> Self {
        Self::new(kind, e.to_string())
    }
}

impl std::fmt::Display for SocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", &self.kind, &self.message)
    }
}

fn get_offset(data: &[u8], e: &serde_json::Error) -> Result<usize, &'static str> {
    let line_number = e.line() - 1;
    let column_number = e.column() - 1;
    let mut parsed_line_count: usize = 0;

    let character_offset = data.iter().enumerate().find_map(|(n, b)| {
        if parsed_line_count < line_number {
            if *b == NEWLINE {
                parsed_line_count += 1;
            }
            None
        } else {
            Some(n + column_number)
        }
    });

    character_offset.ok_or("Failed to find offset")
}

pub struct Socket {
    stream: TcpStream,
    read_buff: Vec<u8>,
    current: Vec<u8>,
    send_buff: Vec<u8>,
}

impl Socket {
    pub fn new(stream: TcpStream) -> Result<Socket, SocketError> {
        stream
            .set_nodelay(true)
            .map_err(|e| SocketError::from_err(SocketErrorKind::SetNoDelayFailed, &e))?;
        stream
            .set_nonblocking(true)
            .map_err(|e| SocketError::from_err(SocketErrorKind::SetNonBlockingFailed, &e))?;

        let read_buff = vec![0; DEFAULT_BUFFER_SIZE];

        Ok(Socket {
            stream,
            read_buff,
            current: Vec::<u8>::new(),
            send_buff: Vec::<u8>::new(),
        })
    }

    pub fn shutdown(&mut self) -> Result<(), SocketError> {
        match self.stream.shutdown(Shutdown::Both) {
            Err(e) => Err(SocketError::from_err(SocketErrorKind::ShutdownFailed, &e)),
            Ok(()) => Ok(()),
        }
    }

    pub fn set_read_buffer_size(&mut self, new_size: usize) {
        self.read_buff.resize(new_size, 0u8);
    }

    pub fn get_read_buffer_size(&self) -> usize {
        self.read_buff.len()
    }

    /// Attempts to read data from the underlying socket into the read buffer.
    pub fn read_data(&mut self) -> Result<usize, SocketError> {
        match self.stream.read(&mut self.read_buff) {
            Ok(read_bytes_count) => {
                if read_bytes_count > 0 {
                    self.current
                        .extend_from_slice(&self.read_buff[..read_bytes_count]);
                }
                Ok(read_bytes_count)
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted => Ok(0),
                _ => Err(SocketError::from_err(SocketErrorKind::ReadFailed, &e)),
            },
        }
    }

    pub fn try_get(&mut self) -> Result<Option<Rcvr>, SocketError> {
        match serde_json::from_slice::<Rcvr>(&self.current) {
            Ok(msg) => {
                self.current.clear();
                Ok(Some(msg))
            }
            Err(e) => match e.classify() {
                Category::Eof => Ok(None),
                Category::Syntax => match get_offset(&self.current, &e) {
                    Ok(offset) => {
                        let msg_result =
                            serde_json::from_slice::<Rcvr>(&self.current[..offset]).map(Some);
                        self.current.drain(0..offset);
                        msg_result.map_err(|err| {
                            SocketError::from_err(SocketErrorKind::SyntaxError, &err)
                        })
                    }
                    Err(_) => Err(SocketError::from_err(SocketErrorKind::SyntaxError, &e)),
                },
                _ => Err(SocketError::from_err(SocketErrorKind::SyntaxError, &e)),
            },
        }
    }
    pub fn enqueue(&mut self, data: &[u8]) {
        self.send_buff.extend_from_slice(data);
    }

    /// Attempts to send the contents of the send buffer to the remote endpoint.
    /// Returns the number of bytes remaining in the send buffer.
    /// If this returns the `Err(SocketError)` variant, it should probably be
    /// `.shutdown()`.
    /// A return value of `Ok(0)` means the send buffer is empty.
    pub fn send_data(&mut self) -> Result<usize, SocketError> {
        match self.stream.write(&self.send_buff) {
            Ok(written_bytes_count) => {
                if written_bytes_count == self.send_buff.len() {
                    self.stream
                        .flush()
                        .map_err(|e| SocketError::from_err(SocketErrorKind::FlushFailed, &e))?;
                    self.send_buff.clear();
                    Ok(0)
                } else {
                    self.send_buff.drain(0..written_bytes_count);
                    Ok(self.send_buff.len())
                }
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::Interrupted => Ok(self.send_buff.len()),
                _ => Err(SocketError::from_err(SocketErrorKind::WriteFailed, &e)),
            },
        }
    }

    /// Attempts to send `data` to the remote endpoint. If the send buffer
    /// is full, this will block until it's not.
    pub fn blocking_send(
        &mut self,
        data: &[u8],
        tick: std::time::Duration,
    ) -> Result<(), SocketError> {
        self.enqueue(data);
        loop {
            if 0 == self.send_data()? {
                return Ok(());
            }
            std::thread::sleep(tick);
        }
    }

    /// Returns how many bytes are currently in the send buffer.
    pub fn send_buff_size(&self) -> usize {
        self.send_buff.len()
    }

    /// Returns how many bytes are currently in the receive buffer.
    pub fn recv_buff_size(&self) -> usize {
        self.current.len()
    }

    /// Returns the address of the remote endpoint of the underlying stream.
    pub fn get_addr(&self) -> Result<String, SocketError> {
        match self.stream.peer_addr() {
            Ok(a) => Ok(a.to_string()),
            Err(e) => Err(SocketError::from_err(
                SocketErrorKind::GetRemoteAddressFailed,
                &e,
            )),
        }
    }
}
