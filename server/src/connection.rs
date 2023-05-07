use common::{proto::Rcvr, socket::Socket, user::User};
use log::debug;
use std::{net::TcpListener, sync::mpsc, time::Duration};

pub fn initial_negotiation(u: &mut User) -> Result<(), String> {
  match u.blocking_get(Duration::from_secs(5)) {
    Err(e) => {
      let err_str = format!("Error reading initial \"Name\" message: {}", e);
      u.logout(&err_str);
      Err(err_str)
    }
    Ok(m) => match m {
      Rcvr::Name(new_name) => {
        u.set_name(&new_name);
        Ok(())
      }
      x => {
        u.logout("Protocol error: Initial message should be of type \"Name\".");
        Err(format!("Bad initial message: {:?}", &x))
      }
    },
  }
}

pub fn listen(addr: String, tx: mpsc::Sender<User>) {
  let mut new_user_id: u64 = 100;
  let listener = TcpListener::bind(&addr).unwrap();
  for res in listener.incoming() {
    match res {
      Err(e) => {
        debug!("listen(): Error accepting connection: {}", &e);
      }
      Ok(stream) => {
        debug!(
          "listen(): Accepted connection from {:?}",
          stream.peer_addr().unwrap()
        );
        let new_sock = match Socket::new(stream) {
          Err(e) => {
            debug!("listen(): Error setting up new Sock: {}", &e);
            continue;
          }
          Ok(x) => x,
        };

        let mut u = User::new(new_sock, new_user_id);
        match initial_negotiation(&mut u) {
          Err(e) => {
            debug!("listen(): Error negotiating initial protocol: {}", &e);
          }
          Ok(()) => {
            debug!(
              "listen(): Sending new client \"{}\" through channel.",
              u.get_name()
            );
            if let Err(e) = tx.send(u) {
              debug!("listen(): Error sending client through channel: {}", &e);
            } else {
              new_user_id += 1;
            }
          }
        }
      }
    }
  }
}
