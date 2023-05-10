use super::proto::Rcvr;
use serde_json::error::Category;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};

const DEFAULT_BUFFER_SIZE: usize = 1024;

const NEWLINE: u8 = b'\n';

#[derive(Debug, Clone)]
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
    let kind_str = kind.to_string();
    Self::new(kind, format!("{}: {}", kind_str, e))
  }
}

impl std::fmt::Display for SocketError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "SocketError: {}", &self.message)
  }
}

impl std::fmt::Display for SocketErrorKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SocketErrorKind::SetNoDelayFailed => {
        write!(f, "Unable to set_nodelay on underlying socket")
      }
      SocketErrorKind::SetNonBlockingFailed => {
        write!(f, "Unable to set_nonblocking on underlying socket")
      }
      SocketErrorKind::ShutdownFailed => {
        write!(f, "Error shutting down underlying socket")
      }
      SocketErrorKind::ReadFailed => {
        write!(f, "Error reading from the underlying socket")
      }
      SocketErrorKind::SyntaxError => {
        write!(f, "Syntax error in data from underlying socket")
      }
      SocketErrorKind::WriteFailed => {
        write!(f, "Error writing to the underlying socket")
      }
      SocketErrorKind::FlushFailed => {
        write!(f, "Error flushing the underlying socket")
      }
      SocketErrorKind::GetRemoteAddressFailed => {
        write!(f, "Error retrieving the remote address")
      }
    }
  }
}

fn get_actual_offset(dat: &[u8], e: &serde_json::Error) -> Result<usize, &'static str> {
  let line = e.line() - 1;
  let col = e.column() - 1;
  let mut line_n: usize = 0;

  let offs = dat.iter().enumerate().find_map(|(n, b)| {
    if line_n < line {
      if *b == NEWLINE {
        line_n += 1;
      }
      None
    } else {
      Some(n + col)
    }
  });

  offs.ok_or("Overran buffer seeking error location.")
}

pub struct Socket {
  stream: TcpStream,
  read_buff: Vec<u8>,
  current: Vec<u8>,
  send_buff: Vec<u8>,
}

impl Socket {
  pub fn new(stream: TcpStream) -> Result<Socket, SocketError> {
    if let Err(e) = stream.set_nodelay(true) {
      return Err(SocketError::from_err(SocketErrorKind::SetNoDelayFailed, &e));
    }
    if let Err(e) = stream.set_nonblocking(true) {
      return Err(SocketError::from_err(
        SocketErrorKind::SetNonBlockingFailed,
        &e,
      ));
    }
    let mut new_buff: Vec<u8> = vec![0; DEFAULT_BUFFER_SIZE];
    new_buff.resize(DEFAULT_BUFFER_SIZE, 0u8);
    let s = Socket {
      stream,
      read_buff: new_buff,
      current: Vec::<u8>::new(),
      send_buff: Vec::<u8>::new(),
    };
    Ok(s)
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
      Err(e) => match e.kind() {
        std::io::ErrorKind::WouldBlock => Ok(0),
        std::io::ErrorKind::Interrupted => Ok(0),
        _ => Err(SocketError::from_err(SocketErrorKind::ReadFailed, &e)),
      },
      Ok(n) => {
        if n > 0 {
          self.current.extend_from_slice(&self.read_buff[..n]);
        }
        Ok(n)
      }
    }
  }

  pub fn try_get(&mut self) -> Result<Option<Rcvr>, SocketError> {
    let offs;
    let maybe_msg = serde_json::from_slice::<Rcvr>(&self.current);
    match maybe_msg {
      Ok(m) => {
        self.current.clear();
        return Ok(Some(m));
      }
      Err(e) => match e.classify() {
        Category::Eof => {
          return Ok(None);
        }
        Category::Syntax => {
          offs = get_actual_offset(&self.current, &e).unwrap();
        }
        _ => {
          return Err(SocketError::from_err(SocketErrorKind::SyntaxError, &e));
        }
      },
    }

    let maybe_msg = serde_json::from_slice::<Rcvr>(&self.current[..offs]).map(Some);
    self.current = self.current.split_off(offs);
    maybe_msg.map_err(|e| SocketError::from_err(SocketErrorKind::SyntaxError, &e))
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
    let res = self.stream.write(&self.send_buff);

    match res {
      Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Ok(self.send_buff.len()),
      Err(e) => Err(SocketError::from_err(SocketErrorKind::WriteFailed, &e)),
      Ok(n) => {
        if n == self.send_buff.len() {
          self
            .stream
            .flush()
            .map_err(|e| SocketError::from_err(SocketErrorKind::FlushFailed, &e))?;
          self.send_buff.clear();
          Ok(0)
        } else {
          self.send_buff = self.send_buff.split_off(n);
          Ok(self.send_buff.len())
        }
      }
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
