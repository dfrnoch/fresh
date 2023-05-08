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
use log::{debug, warn};
use simplelog::WriteLogger;
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

/// Unique user name generator.
fn gen_name(init_count: u64, map: &HashMap<String, u64>) -> String {
  let mut n = init_count;
  loop {
    let new_name = format!("user{}", n);
    if map.get(&new_name).is_none() {
      return new_name;
    }
    n += 1;
  }
}

fn main() {
  let cfg: ServerConfig = ServerConfig::configure();
  println!("Configuration: {:?}", &cfg);
  WriteLogger::init(
    cfg.log_level,
    simplelog::Config::default(),
    std::fs::File::create(&cfg.log_file).unwrap(),
  )
  .unwrap();
  let listen_addr = cfg.address.clone();

  let mut user_map: HashMap<u64, User> = HashMap::new();
  let mut ustr_map: HashMap<String, u64> = HashMap::new();
  let mut room_map: HashMap<u64, Room> = HashMap::new();
  let mut rstr_map: HashMap<String, u64> = HashMap::new();

  let mut lobby: Room = Room::new(0, cfg.lobby_name.clone(), 0);
  lobby.leave(0);
  room_map.insert(0, lobby);
  rstr_map.insert(collapse(&cfg.lobby_name), 0); //this took me a while to figure out :'(

  let (usender, urecvr) = mpsc::channel::<User>();
  thread::spawn(move || {
    listen(listen_addr, usender);
  });

  let mut now: Instant;

  loop {
    now = Instant::now();
    let mut rooms: Vec<u64> = room_map.keys().copied().collect();
    for rid in rooms.drain(..) {
      let rnum = room_map.len();
      match process_room(
        rid,
        now,
        &mut user_map,
        &mut ustr_map,
        &mut room_map,
        &mut rstr_map,
        &cfg,
      ) {
        Ok(()) => {}
        Err(e) => {
          warn!("process_room({}, ...) returned error: {}", rid, &e);
        }
      }
      if rnum != room_map.len() {
        for (k, v) in rstr_map.iter() {
          debug!("{} => {}", k, v);
        }
        for (k, v) in room_map.iter() {
          debug!("{} => {}", k, v.get_idstr());
        }
      }

      if rid != 0 {
        let mut remove: bool = false;
        if let Some(r) = room_map.get(&rid) {
          if r.get_users().is_empty() {
            remove = true;
            let _ = rstr_map.remove(r.get_idstr());
          }
        }
        if remove {
          let _ = room_map.remove(&rid);
        }
      }
    }

    if let Ok(mut user) = urecvr.try_recv() {
      debug!("Accepting user {}: {}", user.get_id(), user.get_name());
      user.deliver_msg(&Sndr::Info(&cfg.welcome));

      let mut rename: Option<String> = None;
      if user.get_idstr().is_empty() {
        rename = Some(String::from(
          "Your name does not have enough whitespace characters.",
        ));
      } else if user.get_name().len() > cfg.max_user_name_length {
        rename = Some(format!(
          "Your name cannot be longer than {} chars.",
          cfg.max_user_name_length
        ));
      } else {
        let maybe_same_name = ustr_map.get(user.get_idstr());
        if let Some(user_n) = maybe_same_name {
          rename = Some(format!(
            "Name \"{}\" exists.",
            user_map.get(user_n).unwrap().get_name()
          ));
        }
      }

      if let Some(err_msg) = rename {
        let new_name = gen_name(user.get_id(), &ustr_map);
        let msg = Sndr::Err(&err_msg);
        user.deliver_msg(&msg);
        let old_name = user.get_name().to_string();
        let data: [&str; 2] = [&old_name, &new_name];
        let altstr = format!("You are now known as \"{}\".", &new_name);
        let msg = Sndr::Misc {
          what: "name",
          data: &data,
          alt: &altstr,
        };
        user.set_name(&new_name);
        user.deliver_msg(&msg);
      }

      let data: [&str; 2] = [user.get_name(), &cfg.lobby_name];
      let env = Env::new(
        End::Server,
        End::Room(0),
        &Sndr::Misc {
          what: "join",
          data: &data,
          alt: &format!("{} joins {}.", user.get_name(), &cfg.lobby_name),
        },
      );
      let lobby = room_map.get_mut(&0).unwrap();
      lobby.join(user.get_id());
      lobby.enqueue(env);
      ustr_map.insert(user.get_idstr().to_string(), user.get_id());
      user_map.insert(user.get_id(), user);
    }

    let loop_time = Instant::now().duration_since(now);
    if loop_time < cfg.min_tick {
      thread::sleep(cfg.min_tick - loop_time);
    }
  }
}
