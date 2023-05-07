use crate::input::Mode;
use common::{config::ClientConfig, proto::Sndr, socket::Socket};
use std::net::TcpStream;

pub struct Globals {
  pub uname: String,
  pub rname: String,
  pub mode: Mode,
  pub messages: Vec<String>,
  pub local_addr: String,
  pub server_addr: String,
  pub socket: Socket,
  pub cmd: char,
  pub run: bool,
}

impl Globals {
  pub fn enqueue(&mut self, m: &Sndr) {
    let b = m.bytes();
    self.socket.enqueue(&b);
  }

  pub fn enqueue_bytes(&mut self, b: &[u8]) {
    self.socket.enqueue(b);
  }
}

/// Attempt to connect to the server.
pub fn connect(cfg: &ClientConfig) -> Result<Socket, String> {
  let mut socket: Socket = match TcpStream::connect(&cfg.address) {
    Err(e) => {
      return Err(format!("Error connecting to {}: {}", cfg.address, e));
    }
    Ok(s) => match Socket::new(s) {
      Err(e) => {
        return Err(format!("Error setting up socket: {}", e));
      }
      Ok(sck) => sck,
    },
  };
  let b = Sndr::Name(&cfg.name).bytes();
  let res = socket.blocking_send(&b, cfg.tick);

  if let Err(e) = res {
    match socket.shutdown() {
      Err(ee) => {
        return Err(format!(
          "Error in initial protocol: {}; error during shutdown: {}",
          e, ee
        ));
      }
      Ok(()) => {
        return Err(format!("Error in initial protocol: {}", e));
      }
    }
  }

  Ok(socket)
}
