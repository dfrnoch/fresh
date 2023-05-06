#[allow(unused_imports)]
use log::{debug, trace, warn};
use simplelog::WriteLogger;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::config::ServerConfig;
use common::proto::*;
use common::room::Room;
use common::socket::Sock;
use common::user::*;

const ENVS_SIZE: usize = 8;
const LOGOUTS_SIZE: usize = 8;
const TEXT_SIZE: usize = 2;
const ROOM_SIZE: usize = 64;

static BLOCK_TIMEOUT: Duration = Duration::from_millis(5000);

struct Context<'a> {
  rid: u64,
  uid: u64,
  umap: &'a mut HashMap<u64, User>,
  ustr: &'a mut HashMap<String, u64>,
  rmap: &'a mut HashMap<u64, Room>,
  rstr: &'a mut HashMap<String, u64>,
}

impl Context<'_> {
  fn gumap(&self, uid: u64) -> Result<&User, String> {
    match self.umap.get(&uid) {
      None => Err(format!("{:?}.gumap(&{}) returns None", &self, &uid)),
      Some(u) => Ok(u),
    }
  }

  fn grmap(&self, rid: u64) -> Result<&Room, String> {
    match self.rmap.get(&rid) {
      None => Err(format!("{:?}.grmap(&{}) return None", &self, &rid)),
      Some(r) => Ok(r),
    }
  }

  fn gumap_mut(&mut self, uid: u64) -> Result<&mut User, String> {
    if let Some(u) = self.umap.get_mut(&uid) {
      return Ok(u);
    }
    return Err(format!(
      "Context {{ rid: {}, uid: {} }}.gumap_mut(&{}) returns None",
      self.rid, self.uid, &uid
    ));
  }

  fn grmap_mut(&mut self, rid: u64) -> Result<&mut Room, String> {
    if let Some(r) = self.rmap.get_mut(&rid) {
      return Ok(r);
    }
    return Err(format!(
      "Context {{ rid: {}, uid: {} }}.grmap_mut(&{}) returns None",
      self.rid, self.uid, &rid
    ));
  }

  fn gustr(&self, u_idstr: &str) -> Option<u64> {
    if let Some(n) = self.ustr.get(u_idstr) {
      Some(*n)
    } else {
      None
    }
  }
  fn grstr(&self, r_idstr: &str) -> Option<u64> {
    if let Some(n) = self.rstr.get(r_idstr) {
      Some(*n)
    } else {
      None
    }
  }
}

impl std::fmt::Debug for Context<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Context")
      .field("rid", &self.rid)
      .field("uid", &self.uid)
      .finish()
  }
}

struct Envs(SmallVec<[Env; ENVS_SIZE]>);

impl Envs {
  pub fn new0() -> Envs {
    let sv: SmallVec<[Env; ENVS_SIZE]> = SmallVec::new();
    return Envs(sv);
  }

  pub fn new1(e: Env) -> Envs {
    let mut sv: SmallVec<[Env; ENVS_SIZE]> = SmallVec::new();
    sv.push(e);
    return Envs(sv);
  }

  pub fn new2(e0: Env, e1: Env) -> Envs {
    let mut sv: SmallVec<[Env; ENVS_SIZE]> = SmallVec::new();
    sv.push(e0);
    sv.push(e1);
    return Envs(sv);
  }
}

impl AsRef<SmallVec<[Env; ENVS_SIZE]>> for Envs {
  fn as_ref(&self) -> &SmallVec<[Env; ENVS_SIZE]> {
    &self.0
  }
}
impl AsMut<SmallVec<[Env; ENVS_SIZE]>> for Envs {
  fn as_mut(&mut self) -> &mut SmallVec<[Env; ENVS_SIZE]> {
    &mut self.0
  }
}

fn match_string<T>(s: &str, hash: &HashMap<String, T>) -> Vec<String> {
  let mut v: Vec<String> = Vec::new();
  for k in hash.keys() {
    if k.starts_with(s) {
      v.push(String::from(k));
    }
  }
  return v;
}

fn append_comma_delimited_list<T: AsRef<str>>(base: &mut String, v: &[T]) {
  let mut v_iter = v.iter();
  if let Some(x) = v_iter.next() {
    base.push_str(x.as_ref());
  }
  while let Some(x) = v_iter.next() {
    base.push_str(", ");
    base.push_str(x.as_ref());
  }
}

fn initial_negotiation(u: &mut User) -> Result<(), String> {
  match u.blocking_get(BLOCK_TIMEOUT) {
    Err(e) => {
      let err_str = format!("Error reading initial \"Name\" message: {}", e);
      u.logout(&err_str);
      return Err(err_str);
    }
    Ok(m) => match m {
      Rcvr::Name(new_name) => {
        u.set_name(&new_name);
        return Ok(());
      }
      x => {
        u.logout("Protocol error: Initial message should be of type \"Name\".");
        return Err(format!("Bad initial message: {:?}", &x));
      }
    },
  }
}

fn listen(addr: String, tx: mpsc::Sender<User>) {
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
        let new_sock: Sock;
        match Sock::new(stream) {
          Err(e) => {
            debug!("listen(): Error setting up new Sock: {}", &e);
            continue;
          }
          Ok(x) => {
            new_sock = x;
          }
        }
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
              new_user_id = new_user_id + 1;
            }
          }
        }
      }
    }
  }
}

fn first_free_id<T: Sized>(map: &HashMap<u64, T>) -> u64 {
  let mut n: u64 = 0;
  while let Some(_) = map.get(&n) {
    n = n + 1;
  }
  return n;
}

/*

The next several functions are called during `process_room(...)` in response
to the various types of `proto2::Msg` pulled out of a given user's `sock`.

*/

fn do_text(ctxt: &mut Context, lines: Vec<String>) -> Result<Envs, String> {
  let u = ctxt.gumap(ctxt.uid)?;
  let mut linesref: SmallVec<[&str; TEXT_SIZE]> = SmallVec::new();
  for s in lines.iter() {
    linesref.push(s.as_str());
  }

  let msg = Sndr::Text {
    who: u.get_name(),
    lines: &linesref,
  };
  let env = Env::new(End::User(ctxt.uid), End::Room(ctxt.rid), &msg);

  return Ok(Envs::new1(env));
}

/// In response to Msg::Priv { who, text }

fn do_priv(ctxt: &mut Context, who: String, text: String) -> Result<Envs, String> {
  let u = ctxt.gumap(ctxt.uid)?;

  let to_tok = collapse(&who);
  if to_tok.len() == 0 {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err("The recipient name must have at least one non-whitespace character."),
    );
    return Ok(Envs::new1(env));
  }

  let tgt_uid = match ctxt.gustr(&to_tok) {
    None => {
      let env = Env::new(
        End::Server,
        End::User(ctxt.uid),
        &Sndr::Err(&format!(
          "There is no user whose name matches \"{}\".",
          &to_tok
        )),
      );
      return Ok(Envs::new1(env));
    }
    Some(n) => n,
  };
  let tgt_u = ctxt.gumap(tgt_uid)?;

  let dat: [&str; 2] = [tgt_u.get_name(), &text];
  let echo_env = Env::new(
    End::Server,
    End::User(ctxt.uid),
    &Sndr::Misc {
      what: "priv_echo",
      data: &dat,
      alt: &format!("$ You @ {}: {}", tgt_u.get_name(), &text),
    },
  );
  let to_env = Env::new(
    End::User(ctxt.uid),
    End::User(tgt_uid),
    &Sndr::Priv {
      who: u.get_name(),
      text: &text,
    },
  );

  return Ok(Envs::new2(echo_env, to_env));
}

/// In response to Msg::Name(new_candidate)

fn do_name(ctxt: &mut Context, cfg: &ServerConfig, new_candidate: String) -> Result<Envs, String> {
  let new_str = collapse(&new_candidate);
  if new_str.len() == 0 {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err("Your name must have more whitespace characters."),
    );
    return Ok(Envs::new1(env));
  } else if new_candidate.len() > cfg.max_user_name_length {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err(&format!(
        "Your name cannot be longer than {} characters.",
        &cfg.max_user_name_length
      )),
    );
    return Ok(Envs::new1(env));
  }

  if let Some(ouid) = ctxt.ustr.get(&new_str) {
    let ou = ctxt.gumap(*ouid)?;
    if *ouid != ctxt.uid {
      let env = Env::new(
        End::Server,
        End::User(ctxt.uid),
        &Sndr::Err(&format!(
          "There is already a user named \"{}\".",
          ou.get_name()
        )),
      );
      return Ok(Envs::new1(env));
    }
  }

  /* The last part of this function is a little wonky. An extra scope
  with some uninitialized upvals are introduced to work around the
  mutable borrow of `mu` from `ctxt.gumap_mut()`.
  */

  let old_idstr: String;
  let new_idstr: String;
  let env: Env;
  {
    let mu = ctxt.gumap_mut(ctxt.uid)?;
    let old_name = mu.get_name().to_string();
    old_idstr = mu.get_idstr().to_string();

    mu.set_name(&new_candidate);
    new_idstr = mu.get_idstr().to_string();
    let dat: [&str; 2] = [old_name.as_str(), new_candidate.as_str()];

    env = Env::new(
      End::Server,
      End::Room(ctxt.rid),
      &Sndr::Misc {
        what: "name",
        data: &dat,
        alt: &format!("{} is now known as {}.", &old_name, &new_candidate),
      },
    );
  }
  let _ = ctxt.ustr.remove(&old_idstr);

  ctxt.ustr.insert(new_idstr, ctxt.uid);
  return Ok(Envs::new1(env));
}

/// In response to Msg::Join(room_name)

fn do_join(ctxt: &mut Context, cfg: &ServerConfig, room_name: String) -> Result<Envs, String> {
  let collapsed = collapse(&room_name);
  if collapsed.len() == 0 {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err("A room name must have more non-whitespace characters."),
    );
    return Ok(Envs::new1(env));
  } else if room_name.len() > cfg.max_room_name_length {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err(&format!(
        "Room names cannot be longer than {} characters.",
        &cfg.max_room_name_length
      )),
    );
    return Ok(Envs::new1(env));
  }

  let tgt_rid = match ctxt.grstr(&collapsed) {
    Some(n) => n,
    None => {
      let new_id = first_free_id(&ctxt.rmap);
      let new_room = Room::new(new_id, room_name.clone(), ctxt.uid);
      ctxt.rstr.insert(collapsed, new_id);
      ctxt.rmap.insert(new_id, new_room);
      let mu = ctxt.gumap_mut(ctxt.uid)?;

      mu.deliver_msg(&Sndr::Info(&format!("You create room \"{}\".", &room_name)));
      new_id
    }
  };

  let uname: String;
  let uid = ctxt.uid;
  let rid = ctxt.rid;
  {
    let u = ctxt.gumap(ctxt.uid)?;
    uname = u.get_name().to_string();
  }

  {
    let targ_r = ctxt.grmap_mut(tgt_rid)?;
    if tgt_rid == rid {
      let env = Env::new(
        End::Server,
        End::User(uid),
        &Sndr::Info(&format!("You are already in \"{}\".", targ_r.get_name())),
      );
      return Ok(Envs::new1(env));
    } else if targ_r.is_banned(&uid) {
      let env = Env::new(
        End::Server,
        End::User(uid),
        &Sndr::Info(&format!("You are banned from \"{}\".", targ_r.get_name())),
      );
      return Ok(Envs::new1(env));
    } else if targ_r.closed && !targ_r.is_invited(&uid) {
      let env = Env::new(
        End::Server,
        End::User(uid),
        &Sndr::Info(&format!("\"{}\" is closed.", targ_r.get_name())),
      );
      return Ok(Envs::new1(env));
    }
    targ_r.join(uid);
    let dat: [&str; 2] = [&uname, targ_r.get_name()];
    let join_env = Env::new(
      End::Server,
      End::Room(tgt_rid),
      &Sndr::Misc {
        what: "join",
        data: &dat,
        alt: &format!("{} joins {}.", &uname, targ_r.get_name()),
      },
    );
    targ_r.enqueue(join_env);
  }

  let cur_r = ctxt.grmap_mut(ctxt.rid)?;
  let dat: [&str; 2] = [&uname, "[ moved to another room ]"];

  let leave_env = Env::new(
    End::Server,
    End::Room(tgt_rid),
    &Sndr::Misc {
      what: "leave",
      alt: &format!("{} moved to another room.", &uname),
      data: &dat,
    },
  );
  cur_r.leave(uid);
  return Ok(Envs::new1(leave_env));
}

/// In response to Msg::Block(user_name)

fn do_block(ctxt: &mut Context, user_name: String) -> Result<Envs, String> {
  let collapsed = collapse(&user_name);
  if collapsed.len() == 0 {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err("That cannot be anyone's user name."),
    );
    return Ok(Envs::new1(env));
  }
  let ouid = match ctxt.ustr.get(&collapsed) {
    None => {
      let env = Env::new(
        End::Server,
        End::User(ctxt.uid),
        &Sndr::Info(&format!(
          "No users matching the pattern \"{}\".",
          &collapsed
        )),
      );
      return Ok(Envs::new1(env));
    }
    Some(n) => *n,
  };
  if ouid == ctxt.uid {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err("You shouldn't block yourself."),
    );
    return Ok(Envs::new1(env));
  }

  let blocked_name = match ctxt.umap.get(&ouid) {
    None => {
      return Err(format!(
        "do_block(r {}, u {}): no target User {}",
        ctxt.rid, ctxt.uid, ouid
      ));
    }
    Some(u) => u.get_name().to_string(),
  };

  let mu = ctxt.gumap_mut(ctxt.uid)?;
  let could_block: bool = mu.block_id(ouid);
  if could_block {
    mu.deliver_msg(&Sndr::Info(&format!(
      "You are now blocking {}.",
      &blocked_name
    )));
  } else {
    mu.deliver_msg(&Sndr::Err(&format!(
      "You are already blocking {}.",
      &blocked_name
    )));
  };

  return Ok(Envs::new0());
}

/// In response to Msg::Unblock(user_name)

fn do_unblock(ctxt: &mut Context, user_name: String) -> Result<Envs, String> {
  let collapsed = collapse(&user_name);
  if collapsed.len() == 0 {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err("That cannot be anyone's user name."),
    );
    return Ok(Envs::new1(env));
  }
  let ouid = match ctxt.ustr.get(&collapsed) {
    None => {
      let env = Env::new(
        End::Server,
        End::User(ctxt.uid),
        &Sndr::Info(&format!(
          "No users matching the pattern \"{}\".",
          &collapsed
        )),
      );
      return Ok(Envs::new1(env));
    }
    Some(n) => *n,
  };
  if ouid == ctxt.uid {
    let env = Env::new(
      End::Server,
      End::User(ctxt.uid),
      &Sndr::Err("You couldn't block yourself; you can't unblock yourself."),
    );
    return Ok(Envs::new1(env));
  }

  let blocked_name = match ctxt.umap.get(&ouid) {
    None => {
      return Err(format!(
        "do_unblock(r {}, u {}): no target User {}",
        ctxt.rid, ctxt.uid, ouid
      ));
    }
    Some(u) => u.get_name().to_string(),
  };

  let mu = ctxt.gumap_mut(ctxt.uid)?;
  let could_unblock: bool = mu.unblock_id(ouid);
  if could_unblock {
    mu.deliver_msg(&Sndr::Info(&format!("You unblock {}.", &blocked_name)));
  } else {
    mu.deliver_msg(&Sndr::Err(&format!(
      "You were not blocking {}.",
      &blocked_name
    )));
  }

  return Ok(Envs::new0());
}

/// In response to Msg::Logout(salutation)

fn do_logout(ctxt: &mut Context, salutation: String) -> Result<Envs, String> {
  let mr = match ctxt.rmap.get_mut(&ctxt.rid) {
    None => {
      return Err(format!(
        "do_logout(r {}, u {}): no Room {}",
        ctxt.rid, ctxt.uid, ctxt.rid
      ));
    }
    Some(r) => r,
  };
  mr.leave(ctxt.uid);

  let mut mu = match ctxt.umap.remove(&ctxt.uid) {
    None => {
      return Err(format!(
        "do_logout(r {}, u {}): no User {}",
        ctxt.rid, ctxt.uid, ctxt.uid
      ));
    }
    Some(u) => u,
  };
  let _ = ctxt.ustr.remove(mu.get_idstr());
  mu.logout("You have logged out.");

  let dat: [&str; 2] = [mu.get_name(), &salutation];
  let env = Env::new(
    End::Server,
    End::Room(ctxt.rid),
    &Sndr::Misc {
      what: "leave",
      alt: &format!("{} leaves: {}", mu.get_name(), &salutation),
      data: &dat,
    },
  );
  mr.enqueue(env);

  Ok(Envs::new0())
}

/// In response to Msg::Query { what, arg }

fn do_query(ctxt: &mut Context, what: String, arg: String) -> Result<Envs, String> {
  match what.as_str() {
    "addr" => {
      let mu = ctxt.gumap_mut(ctxt.uid)?;
      let (addr_str, alt_str): (String, String) = match mu.get_addr() {
        None => (
          "???".to_string(),
          "Your public address cannot be determined.".to_string(),
        ),
        Some(s) => {
          let astr = format!("Your public address is {}.", &s);
          (s, astr)
        }
      };
      let dat: [&str; 1] = [&addr_str];
      let msg = Sndr::Misc {
        what: "addr",
        data: &dat,
        alt: &alt_str,
      };
      mu.deliver_msg(&msg);
      return Ok(Envs::new0());
    }

    "roster" => {
      let r = ctxt.grmap(ctxt.rid)?;
      let op_id = r.get_op();
      let mut names_list: SmallVec<[&str; ROOM_SIZE]> =
        SmallVec::with_capacity(r.get_users().len());

      for uid in r.get_users().iter().rev() {
        if *uid != op_id {
          match ctxt.umap.get(uid) {
            None => {
              warn!(
                "do_query(r {}, u{} {:?}): no User {}",
                ctxt.rid, ctxt.uid, &what, uid
              );
            }
            Some(u) => {
              names_list.push(u.get_name());
            }
          }
        }
      }

      let mut altstr: String;
      /* The lobby will never have an operator. It's operator uid is
      set to 0 (the lowest possible uid is 100). */
      if op_id == 0 {
        altstr = format!("{} roster: ", r.get_name());
        append_comma_delimited_list(&mut altstr, &names_list);
      } else {
        let op_name = match ctxt.umap.get(&op_id) {
          None => "[ ??? ]",
          Some(u) => u.get_name(),
        };
        altstr = format!("{} roster: {} (operator) ", r.get_name(), op_name);
        append_comma_delimited_list(&mut altstr, &names_list);
        names_list.push(op_name);
      }

      let mut names_ref: SmallVec<[&str; ROOM_SIZE]> = SmallVec::with_capacity(names_list.len());
      for x in names_list.iter().rev() {
        names_ref.push(x);
      }

      let env = Env::new(
        End::Server,
        End::User(ctxt.uid),
        &Sndr::Misc {
          what: "roster",
          data: &names_ref,
          alt: &altstr,
        },
      );
      return Ok(Envs::new1(env));
    }

    "who" => {
      let collapsed = collapse(&arg);
      let matches = match_string(&collapsed, ctxt.ustr);
      let env: Env;
      if matches.len() == 0 {
        env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info(&format!(
            "No users matching the pattern \"{}\".",
            &collapsed
          )),
        );
      } else {
        let mut altstr = String::from("Matching names: ");
        append_comma_delimited_list(&mut altstr, &matches);
        let listref: Vec<&str> = matches.iter().map(|x| x.as_str()).collect();
        env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Misc {
            what: "who",
            data: &listref,
            alt: &altstr,
          },
        );
      }
      return Ok(Envs::new1(env));
    }

    "rooms" => {
      let collapsed = collapse(&arg);
      let matches = match_string(&collapsed, ctxt.rstr);
      let env: Env;
      if matches.len() == 0 {
        env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info(&format!(
            "No Rooms matching the pattern \"{}\".",
            &collapsed
          )),
        );
      } else {
        let mut altstr = String::from("Matching Rooms: ");
        append_comma_delimited_list(&mut altstr, &matches);
        let listref: Vec<&str> = matches.iter().map(|x| x.as_str()).collect();
        env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Misc {
            what: "rooms",
            data: &listref,
            alt: &altstr,
          },
        );
      }
      return Ok(Envs::new1(env));
    }

    ukn @ _ => {
      let env = Env::new(
        End::Server,
        End::User(ctxt.uid),
        &Sndr::Err(&format!("Unknown \"Query\" type: \"{}\".", ukn)),
      );
      return Ok(Envs::new1(env));
    }
  }
}

fn do_op(ctxt: &mut Context, op: RcvOp) -> Result<Envs, String> {
  {
    let r = ctxt.grmap(ctxt.rid)?;
    if r.get_op() != ctxt.uid {
      let env = Env::new(
        End::Server,
        End::User(ctxt.uid),
        &Sndr::Err("You are not the operator of this Room."),
      );
      return Ok(Envs::new1(env));
    }
  }

  let (uid, rid) = (ctxt.uid, ctxt.rid);
  let op_name = {
    let u = ctxt.gumap(ctxt.uid)?;
    u.get_name().to_string()
  };

  match op {
    RcvOp::Open => {
      let cur_r = ctxt.grmap_mut(rid)?;
      if cur_r.closed {
        cur_r.closed = false;
        let env = Env::new(
          End::Server,
          End::Room(rid),
          &Sndr::Info(&format!("{} has opened {}.", &op_name, cur_r.get_name())),
        );
        return Ok(Envs::new1(env));
      } else {
        let env = Env::new(
          End::Server,
          End::User(uid),
          &Sndr::Info(&format!("{} is already open.", cur_r.get_name())),
        );
        return Ok(Envs::new1(env));
      }
    }

    RcvOp::Close => {
      let cur_r = ctxt.grmap_mut(rid)?;
      if cur_r.closed {
        let env = Env::new(
          End::Server,
          End::User(uid),
          &Sndr::Info(&format!("{} is already closed.", cur_r.get_name())),
        );
        return Ok(Envs::new1(env));
      } else {
        cur_r.closed = true;
        let env = Env::new(
          End::Server,
          End::Room(rid),
          &Sndr::Info(&format!("{} has closed {}.", &op_name, cur_r.get_name())),
        );
        return Ok(Envs::new1(env));
      }
    }

    RcvOp::Give(ref new_name) => {
      let collapsed = collapse(&new_name);
      if collapsed.len() == 0 {
        let env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Err("That cannot be anyone's user name."),
        );
        return Ok(Envs::new1(env));
      }

      let ouid = match ctxt.ustr.get(&collapsed) {
        None => {
          let env = Env::new(
            End::Server,
            End::User(ctxt.uid),
            &Sndr::Info(&format!(
              "No users matching the pattern \"{}\".",
              &collapsed
            )),
          );
          return Ok(Envs::new1(env));
        }
        Some(n) => *n,
      };

      if ouid == ctxt.uid {
        let env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info("You are already the operator of this room."),
        );
        return Ok(Envs::new1(env));
      }

      let ou_name = {
        let u = ctxt.gumap(ouid)?;
        u.get_name().to_string()
      };

      let cur_r = ctxt.grmap_mut(rid)?;
      if !cur_r.get_users().contains(&ouid) {
        let env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info(&format!(
            "{} must be in the room to transfer ownership.",
            &ou_name
          )),
        );
        return Ok(Envs::new1(env));
      }
      cur_r.set_op(ouid);
      let dat: [&str; 2] = [&ou_name, cur_r.get_name()];
      let env = Env::new(
        End::Server,
        End::Room(rid),
        &Sndr::Misc {
          what: "new_op",
          alt: &format!("{} is now the operator of {}.", &ou_name, cur_r.get_name()),
          data: &dat,
        },
      );
      return Ok(Envs::new1(env));
    }

    RcvOp::Invite(ref uname) => {
      let collapsed = collapse(&uname);
      if collapsed.len() == 0 {
        let env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info("That cannot be anyone's user name."),
        );
        return Ok(Envs::new1(env));
      }

      let ouid = match ctxt.ustr.get(&collapsed) {
        None => {
          let env = Env::new(
            End::Server,
            End::User(ctxt.uid),
            &Sndr::Info(&format!(
              "No users matching the pattern \"{}\".",
              &collapsed
            )),
          );
          return Ok(Envs::new1(env));
        }
        Some(n) => *n,
      };

      let cur_r = match ctxt.rmap.get_mut(&ctxt.rid) {
        None => {
          return Err(format!(
            "do_op(r {}, u {}, {:?}): no Room {}",
            ctxt.rid, ctxt.uid, &op, ctxt.rid
          ));
        }
        Some(r) => r,
      };

      if ouid == ctxt.uid {
        let env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info(&format!("You are already allowed in {}.", cur_r.get_name())),
        );
        return Ok(Envs::new1(env));
      };

      let ou = match ctxt.umap.get_mut(&ouid) {
        None => {
          return Err(format!(
            "do_op(r {}, u {}, {:?}): no target User {}",
            ctxt.rid, ctxt.uid, &op, ouid
          ));
        }
        Some(u) => u,
      };

      if cur_r.is_invited(&ouid) {
        let env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info(&format!(
            "{} has already been invited to {}.",
            ou.get_name(),
            cur_r.get_name()
          )),
        );
        return Ok(Envs::new1(env));
      };
      cur_r.invite(ouid);

      let inviter_env: Env;
      if cur_r.get_users().contains(&ouid) {
        inviter_env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info(&format!(
            "{} may now return to {} even when closed.",
            ou.get_name(),
            cur_r.get_name()
          )),
        );
        ou.deliver_msg(&Sndr::Info(&format!(
          "You have been invited to return to {} even if it closes.",
          cur_r.get_name()
        )));
      } else {
        inviter_env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info(&format!(
            "You invite {} to join {}.",
            ou.get_name(),
            cur_r.get_name()
          )),
        );
        ou.deliver_msg(&Sndr::Info(&format!(
          "You have been invited to join {}.",
          cur_r.get_name()
        )));
      }
      return Ok(Envs::new1(inviter_env));
    }

    RcvOp::Kick(ref uname) => {
      let collapsed = collapse(&uname);
      if collapsed.len() == 0 {
        let env = Env::new(
          End::Server,
          End::User(ctxt.uid),
          &Sndr::Info("That cannot be anyone's user name."),
        );
        return Ok(Envs::new1(env));
      }

      let ouid = match ctxt.ustr.get(&collapsed) {
        None => {
          let env = Env::new(
            End::Server,
            End::User(ctxt.uid),
            &Sndr::Info(&format!(
              "No users matching the pattern \"{}\".",
              &collapsed
            )),
          );
          return Ok(Envs::new1(env));
        }
        Some(n) => *n,
      };

      if ouid == ctxt.uid {
        let env = Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Info("Bestowing the operator mantle on another and then leaving would be a more orderly transfer of power."
                    ));
        return Ok(Envs::new1(env));
      }

      let ku = match ctxt.umap.get_mut(&ouid) {
        None => {
          return Err(format!(
            "do_op(r {}, u {}, {:?}): no target User {}",
            ctxt.rid, ctxt.uid, &op, ouid
          ));
        }
        Some(u) => u,
      };

      let in_room: bool;
      let mut cur_room_name = String::new();

      {
        let cur_r = match ctxt.rmap.get_mut(&ctxt.rid) {
          None => {
            return Err(format!(
              "do_op(r {}, u {}, {:?}): no Room {}",
              ctxt.rid, ctxt.uid, &op, ctxt.rid
            ));
          }
          Some(r) => r,
        };

        if cur_r.is_banned(&ouid) {
          let env = Env::new(
            End::Server,
            End::User(ctxt.uid),
            &Sndr::Info(&format!(
              "{} is already banned from {}.",
              ku.get_name(),
              cur_r.get_name()
            )),
          );
          return Ok(Envs::new1(env));
        };

        cur_r.ban(ouid);
        in_room = cur_r.get_users().contains(&ouid);

        if !in_room {
          /* This case is easy because we only have to message the
          banner about his activity.
          */
          if !cur_r.get_users().contains(&ouid) {
            let env = Env::new(
              End::Server,
              End::User(ctxt.uid),
              &Sndr::Info(&format!(
                "You have banned {} from {}.",
                ku.get_name(),
                cur_r.get_name()
              )),
            );
            return Ok(Envs::new1(env));
          }
        } else {
          /* This case is tougher because it involves
            * messaging the kicked user
            * moving the kicked user
            * messaging the room
            * messaging the Lobby that he's joined it

          It requires careful dancing around &mut lifetimes.
          */

          let altstr = format!("You have been kicked from {}.", cur_r.get_name());
          let dat: [&str; 1] = [cur_r.get_name()];
          let to_kicked = Sndr::Misc {
            what: "kick_you",
            alt: &altstr,
            data: &dat,
          };
          ku.deliver_msg(&to_kicked);
          cur_r.leave(ouid);

          cur_room_name = cur_r.get_name().to_string();
        }
      }

      let lobby = ctxt.rmap.get_mut(&0).unwrap();
      lobby.join(ouid);
      let dat: [&str; 2] = [ku.get_name(), lobby.get_name()];
      let to_lobby = Env::new(
        End::Server,
        End::Room(ctxt.rid),
        &Sndr::Misc {
          what: "join",
          data: &dat,
          alt: &format!("{} joins {}.", ku.get_name(), lobby.get_name()),
        },
      );
      lobby.enqueue(to_lobby);

      let dat: [&str; 2] = [ku.get_name(), &cur_room_name];
      let env = Env::new(
        End::Server,
        End::Room(ctxt.rid),
        &Sndr::Misc {
          what: "kick_other",
          data: &dat,
          alt: &format!("{} has been kicked from {}.", ku.get_name(), &cur_room_name),
        },
      );

      return Ok(Envs::new1(env));
    }
  }
}

fn process_room(
  rid: u64,
  current_time: Instant,
  user_map: &mut HashMap<u64, User>,
  ustr_map: &mut HashMap<String, u64>,
  room_map: &mut HashMap<u64, Room>,
  rstr_map: &mut HashMap<String, u64>,
  cfg: &ServerConfig,
) -> Result<(), String> {
  let mut uid_list: SmallVec<[u64; ROOM_SIZE]>;
  {
    match room_map.get(&rid) {
      None => {
        return Err(format!("Room {} doesn't exist.", &rid));
      }
      Some(r) => {
        uid_list = SmallVec::from_slice(r.get_users());
      }
    }
  }

  let mut ctxt = Context {
    rid: rid,
    uid: 0,
    umap: user_map,
    ustr: ustr_map,
    rmap: room_map,
    rstr: rstr_map,
  };

  let mut envz: Envs = Envs::new0();
  let mut logouts: SmallVec<[(u64, &str); LOGOUTS_SIZE]> = SmallVec::new();

  for uid in &uid_list {
    let m: Rcvr;
    {
      let mu = match ctxt.umap.get_mut(uid) {
        None => {
          debug!("process_room({}): user {} doesn't exist", &rid, uid);
          continue;
        }
        Some(x) => x,
      };

      let over_quota = mu.get_byte_quota() > cfg.byte_limit;
      mu.drain_byte_quota(cfg.byte_tick);
      if over_quota && mu.get_byte_quota() <= cfg.byte_limit {
        let msg = Sndr::Err("You may send messages again.");
        mu.deliver_msg(&msg);
      }

      match mu.try_get() {
        None => {
          let last = mu.get_last_data_time();
          match current_time.checked_duration_since(last) {
            Some(x) if x > cfg.blackout_time_to_kick => {
              logouts.push((*uid, "Too long since server received data from the client."));
            }
            Some(x) if x > cfg.blackout_time_to_ping => {
              mu.deliver_msg(&Sndr::Ping);
            }
            _ => {}
          }
          continue;
        }
        Some(msg) => {
          if !over_quota {
            m = msg;
            if mu.get_byte_quota() > cfg.byte_limit {
              let msg = Sndr::Err("You have exceeded your data quota and your messages will be ignored for a short time.");
              mu.deliver_msg(&msg);
            }
          } else {
            continue;
          }
        }
      }

      if mu.has_errors() {
        let e = mu.get_errors();
        warn!("User {} being logged out for error(s): {}", uid, &e);
        logouts.push((*uid, "Communication error."));
      }
    }

    ctxt.uid = *uid;

    let pres = match m {
      Rcvr::Text { who: _, lines: l } => do_text(&mut ctxt, l),
      Rcvr::Priv { who, text } => do_priv(&mut ctxt, who, text),
      Rcvr::Name(new_candidate) => do_name(&mut ctxt, cfg, new_candidate),
      Rcvr::Join(room_name) => do_join(&mut ctxt, cfg, room_name),
      Rcvr::Block(user_name) => do_block(&mut ctxt, user_name),
      Rcvr::Unblock(user_name) => do_unblock(&mut ctxt, user_name),
      Rcvr::Logout(salutation) => do_logout(&mut ctxt, salutation),
      Rcvr::Query { what, arg } => do_query(&mut ctxt, what, arg),
      Rcvr::Op(op) => do_op(&mut ctxt, op),
      _ => {
        /* Other patterns require no response. */
        Ok(Envs::new0())
      }
    };

    match pres {
      Err(e) => {
        #[cfg(debug_assertions)]
        trace!("{}", &e);
      }
      Ok(mut v) => {
        let evz = envz.as_mut();
        for env in v.as_mut().drain(..) {
          evz.push(env);
        }
      }
    }
  }

  for (uid, errmsg) in logouts.iter() {
    if let Some(mut mu) = ctxt.umap.remove(&uid) {
      let _ = ctxt.ustr.remove(mu.get_idstr());
      let msg = Sndr::Logout(errmsg);
      mu.deliver_msg(&msg);
      let dat: [&str; 2] = [mu.get_name(), "[ disconnected by server ]"];
      let env = Env::new(
        End::Server,
        End::Room(ctxt.rid),
        &Sndr::Misc {
          what: "leave",
          data: &dat,
          alt: &format!("{} has been disconnected from the server.", mu.get_name()),
        },
      );
      envz.as_mut().push(env);
    } else {
      warn!(
        "process_room({} ...): logouts.drain(): no User {}",
        ctxt.rid, uid
      );
    }
  }

  // Change room operator if current op is no longer in room.
  // (But obviously not for the lobby.)

  if rid != 0 {
    let mr = ctxt.rmap.get_mut(&rid).unwrap();
    let op_id = mr.get_op();
    let op_still_here = mr.get_users().contains(&op_id);
    if !op_still_here {
      if let Some(pnid) = mr.get_users().get(0) {
        if let Some(u) = ctxt.umap.get(pnid) {
          let nid = *pnid;
          mr.set_op(nid);
          let env = Env::new(
            End::Server,
            End::Room(rid),
            &Sndr::Info(&format!("{} is now the Room operator.", u.get_name())),
          );
          envz.as_mut().push(env);
        }
      }
    }
  }

  {
    let r = ctxt.rmap.get_mut(&rid).unwrap();
    for (uid, _) in logouts.drain(..) {
      r.leave(uid);
    }
    r.deliver_inbox(ctxt.umap);
    for env in envz.as_ref() {
      r.deliver(env, ctxt.umap);
    }
    uid_list.clear();
    uid_list.extend_from_slice(r.get_users());
    for uid in uid_list.iter_mut() {
      if let Some(mu) = user_map.get_mut(uid) {
        mu.nudge();
      }
    }
  }

  return Ok(());
}

/** When a user joins with a name that `collapse()`s to a user who is
already joined, this generates them a generic (but unique) name.
*/
fn gen_name(init_count: u64, map: &HashMap<String, u64>) -> String {
  let mut n = init_count;
  loop {
    let new_name = format!("user{}", n);
    if map.get(&new_name) == None {
      return new_name;
    }
    n = n + 1;
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

  let (usender, urecvr) = mpsc::channel::<User>();
  thread::spawn(move || {
    listen(listen_addr, usender);
  });

  let mut now: Instant;

  loop {
    now = Instant::now();
    let mut roomz: Vec<u64> = room_map.keys().map(|k| *k).collect();
    for rid in roomz.drain(..) {
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
          if r.get_users().len() == 0 {
            remove = true;
            let _ = rstr_map.remove(r.get_idstr());
          }
        }
        if remove {
          let _ = room_map.remove(&rid);
        }
      }
    }

    match urecvr.try_recv() {
      Ok(mut u) => {
        debug!("Accepting user {}: {}", u.get_id(), u.get_name());
        u.deliver_msg(&Sndr::Info(&cfg.welcome));

        let mut rename: Option<String> = None;
        if u.get_idstr().len() == 0 {
          rename = Some(String::from(
            "Your name does not have enough whitespace characters.",
          ));
        } else if u.get_name().len() > cfg.max_user_name_length {
          rename = Some(format!(
            "Your name cannot be longer than {} chars.",
            cfg.max_user_name_length
          ));
        } else {
          let maybe_same_name = ustr_map.get(u.get_idstr());
          if let Some(user_n) = maybe_same_name {
            rename = Some(format!(
              "Name \"{}\" exists.",
              user_map.get(user_n).unwrap().get_name()
            ));
          }
        }

        if let Some(err_msg) = rename {
          let new_name = gen_name(u.get_id(), &ustr_map);
          let msg = Sndr::Err(&err_msg);
          u.deliver_msg(&msg);
          let old_name = u.get_name().to_string();
          let dat: [&str; 2] = [&old_name, &new_name];
          let altstr = format!("You are now known as \"{}\".", &new_name);
          let msg = Sndr::Misc {
            what: "name",
            data: &dat,
            alt: &altstr,
          };
          u.set_name(&new_name);
          u.deliver_msg(&msg);
        }

        let dat: [&str; 2] = [u.get_name(), &cfg.lobby_name];
        let env = Env::new(
          End::Server,
          End::Room(0),
          &Sndr::Misc {
            what: "join",
            data: &dat,
            alt: &format!("{} joins {}.", u.get_name(), &cfg.lobby_name),
          },
        );
        let lobby = room_map.get_mut(&0).unwrap();
        lobby.join(u.get_id());
        lobby.enqueue(env);
        ustr_map.insert(u.get_idstr().to_string(), u.get_id());
        user_map.insert(u.get_id(), u);
      }
      Err(_) => {}
    }

    let loop_time = Instant::now().duration_since(now);
    if loop_time < cfg.min_tick {
      thread::sleep(cfg.min_tick - loop_time);
    }
  }
}
