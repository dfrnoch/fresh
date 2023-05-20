mod connection;
mod message;
mod processing;

use crate::connection::listen;
use crate::processing::process_room;
use common::config::ServerConfig;
use common::proto::*;
use common::room::Room;
use common::user::*;
use common::util::collapse;
use log::{debug, error, info, warn};
use simplelog::WriteLogger;
use std::collections::HashMap;
use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

/// Unique user name generator.
fn gen_name(init_count: u64, map: &HashMap<String, u64>) -> String {
    let mut new_id = init_count;
    loop {
        let new_name = format!("user{}", new_id);
        if map.get(&new_name).is_none() {
            return new_name;
        }
        new_id += 1;
    }
}
fn main() {
    if let Err(e) = run() {
        error!("Error: {}", e);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cfg: ServerConfig = ServerConfig::configure();

    WriteLogger::init(
        cfg.log_level,
        simplelog::Config::default(),
        std::fs::File::create(&cfg.log_file)?,
    )?;

    let listen_addr = cfg.address.clone();

    info!("Starting server on {}", listen_addr);

    let mut users_by_id = HashMap::new();
    let mut user_ids_by_str = HashMap::new();
    let mut rooms_by_id = HashMap::new();
    let mut room_ids_by_str = HashMap::new();

    let mut lobby: Room = Room::new(0, cfg.lobby_name.clone(), 0);
    lobby.leave(0);
    rooms_by_id.insert(0, lobby);
    room_ids_by_str.insert(collapse(&cfg.lobby_name), 0);

    let (usender, urecvr) = mpsc::channel::<User>();

    thread::spawn(move || {
        listen(listen_addr, usender).unwrap_or_else(|e| {
            error!("listen() encountered an error: {}", e);
        })
    });

    let mut now: Instant;

    loop {
        now = Instant::now();
        let mut rooms: Vec<u64> = rooms_by_id.keys().copied().collect();
        for room_id in rooms.drain(..) {
            let room_count = rooms_by_id.len();
            match process_room(
                room_id,
                now,
                &mut users_by_id,
                &mut user_ids_by_str,
                &mut rooms_by_id,
                &mut room_ids_by_str,
                &cfg,
            ) {
                Ok(()) => {}
                Err(e) => {
                    warn!("process_room({}, ...) returned error: {}", room_id, &e);
                }
            }
            if room_count != rooms_by_id.len() {
                for (k, v) in room_ids_by_str.iter() {
                    debug!("{} => {}", k, v);
                }
                for (k, v) in rooms_by_id.iter() {
                    debug!("{} => {}", k, v.get_idstr());
                }
            }

            if room_id != 0 {
                let mut remove: bool = false;
                if let Some(r) = rooms_by_id.get(&room_id) {
                    if r.get_users().is_empty() {
                        remove = true;
                        let _ = room_ids_by_str.remove(r.get_idstr());
                    }
                }
                if remove {
                    let _ = rooms_by_id.remove(&room_id);
                }
            }
        }

        if let Ok(mut user) = urecvr.try_recv() {
            debug!("Accepting user {}: {}", user.get_id(), user.get_name());
            user.deliver_msg(&Sndr::Info(&cfg.welcome_message));

            let mut required_name_change: Option<String> = None;
            if user.get_idstr().is_empty() {
                required_name_change = Some(String::from(
                    "Your name does not have enough whitespace characters.",
                ));
            } else if user.get_name().len() > cfg.max_user_name_length {
                required_name_change = Some(format!(
                    "Your name cannot be longer than {} characters.",
                    cfg.max_user_name_length
                ));
            } else {
                let potential_name_conflict = user_ids_by_str.get(user.get_idstr());
                if let Some(user_n) = potential_name_conflict {
                    required_name_change = Some(format!(
                        "Name \"{}\" exists.",
                        users_by_id.get(user_n).unwrap().get_name()
                    ));
                }
            }

            if let Some(err_msg) = required_name_change {
                let suggested_new_name = gen_name(user.get_id(), &user_ids_by_str);
                let msg = Sndr::Err(&err_msg);
                user.deliver_msg(&msg);
                let original_name = user.get_name().to_string();
                let data: [&str; 2] = [&original_name, &suggested_new_name];
                let user_notification = format!("You are now known as \"{}\".", &suggested_new_name);
                let msg = Sndr::Misc {
                    what: "name",
                    data: &data,
                    alt: &user_notification,
                };
                user.set_name(&suggested_new_name);
                user.deliver_msg(&msg);
            }

            let data: [&str; 2] = [user.get_name(), &cfg.lobby_name];
            let env = Env::new(
                End::Server,
                End::Room(0),
                &Sndr::Misc {
                    what: "join",
                    data: &data,
                    alt: &format!("{} joined {}.", user.get_name(), &cfg.lobby_name),
                },
            );
            let lobby = rooms_by_id.get_mut(&0).unwrap();
            lobby.join(user.get_id());
            lobby.enqueue(env);
            user_ids_by_str.insert(user.get_idstr().to_string(), user.get_id());
            users_by_id.insert(user.get_id(), user);
        }

        let loop_time = Instant::now().duration_since(now);
        if loop_time < cfg.min_tick {
            thread::sleep(cfg.min_tick - loop_time);
        }
    }
}
