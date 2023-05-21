use crate::input::Mode;
use common::{config::ClientConfig, proto::Sndr, socket::Socket};
use std::net::TcpStream;

pub struct State {
    pub username: String,
    pub room_name: String,
    pub mode: Mode,
    pub buffered_messages: Vec<String>,
    #[cfg(target_os = "windows")]
    pub last_key: Option<(std::time::Instant, crossterm::event::KeyCode)>,
    pub local_address: String,
    pub server_address: String,
    pub socket: Socket,
    pub cmd: char,
    pub running: bool,
}

impl State {
    pub fn enqueue(&mut self, msg: &Sndr) {
        let bytes = msg.bytes();
        self.socket.enqueue(&bytes);
    }

    pub fn enqueue_bytes(&mut self, bytes: &[u8]) {
        self.socket.enqueue(bytes);
    }
}

/// Attempt to connect to the server.
pub fn connect(cfg: &ClientConfig) -> Result<Socket, String> {
    let tcp_stream = TcpStream::connect(&cfg.address)
        .map_err(|e| format!("Error connecting to {}: {}", cfg.address, e))?;

    let mut socket =
        Socket::new(tcp_stream).map_err(|e| format!("Error setting up socket: {}", e))?;

    let bytes = Sndr::Name(&cfg.name).bytes();
    let res = socket.blocking_send(&bytes, cfg.tick);

    if let Err(e) = res {
        let shutdown_err = format!("Error in initial protocol: {}", e);
        socket
            .shutdown()
            .map_err(|ee| format!("{}; error during shutdown: {}", shutdown_err, ee))?;
        return Err(shutdown_err);
    }

    Ok(socket)
}
