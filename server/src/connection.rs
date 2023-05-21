use common::{proto::Rcvr, socket::Socket, user::User};
use log::debug;
use std::{net::TcpListener, sync::mpsc, time::Duration};

pub fn initial_negotiation(user: &mut User) -> Result<(), String> {
    match user.blocking_get(Duration::from_secs(5)).map_err(|e| {
        let err_str = format!("Error reading initial \"Name\" message: {}", e);
        user.logout(&err_str);
        err_str
    })? {
        Rcvr::Name(new_name) => {
            user.set_name(&new_name);
            Ok(())
        }
        x => {
            let err_str = "Protocol error: Initial message should be of type \"Name\".";
            user.logout(err_str);
            Err(format!("Bad initial message: {:?}", &x))
        }
    }
}

pub fn listen(address: String, tx: mpsc::Sender<User>) -> Result<(), Box<dyn std::error::Error>> {
    let mut new_user_id: u64 = 100;
    let listener = TcpListener::bind(&address)?;

    println!("Listening on {}", &address);

    for stream_result in listener.incoming() {
        let stream = match stream_result {
            Err(e) => {
                debug!("listen(): Error accepting connection: {}", &e);
                continue;
            }
            Ok(stream) => stream,
        };

        debug!(
            "listen(): Accepted connection from {:?}",
            stream.peer_addr()?
        );

        let new_socket = match Socket::new(stream) {
            Err(e) => {
                debug!("listen(): Error setting up new Sock: {}", &e);
                continue;
            }
            Ok(socket) => socket,
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

    Ok(())
}
