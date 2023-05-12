use common::{proto::Rcvr, socket::Socket, user::User};
use log::debug;
use std::{net::TcpListener, sync::mpsc, time::Duration};

pub fn initial_negotiation(user: &mut User) -> Result<(), String> {
    match user.blocking_get(Duration::from_secs(5)) {
        Err(e) => {
            let err_str = format!("Error reading initial \"Name\" message: {}", e);
            user.logout(&err_str);
            Err(err_str)
        }
        Ok(m) => match m {
            Rcvr::Name(new_name) => {
                user.set_name(&new_name);
                Ok(())
            }
            x => {
                user.logout("Protocol error: Initial message should be of type \"Name\".");
                Err(format!("Bad initial message: {:?}", &x))
            }
        },
    }
}

pub fn listen(address: String, tx: mpsc::Sender<User>) {
    let mut new_user_id: u64 = 100;
    let listener = TcpListener::bind(&address).unwrap();
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
                let new_socket = match Socket::new(stream) {
                    Err(e) => {
                        debug!("listen(): Error setting up new Sock: {}", &e);
                        continue;
                    }
                    Ok(x) => x,
                };

                let mut user = User::new(new_socket, new_user_id);
                match initial_negotiation(&mut user) {
                    Err(e) => {
                        debug!("listen(): Error negotiating initial protocol: {}", &e);
                    }
                    Ok(()) => {
                        debug!(
                            "listen(): Sending new client \"{}\" through channel.",
                            user.get_name()
                        );
                        if let Err(e) = tx.send(user) {
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
