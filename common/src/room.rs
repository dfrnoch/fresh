use std::collections::HashMap;

use crate::util::collapse;

use super::proto::{End, Env};
use super::user::User;

#[derive(Debug)]
pub struct Room {
  idn: u64,
  name: String,
  idstr: String,
  users: Vec<u64>,
  op: u64,
  pub closed: bool,
  bans: Vec<u64>,
  invites: Vec<u64>,
  inbox: Vec<Env>,
}

impl Room {
  pub fn new(id: u64, new_name: String, creator_id: u64) -> Room {
    Room {
      idn: id,
      idstr: collapse(&new_name),
      name: new_name,
      users: Vec::new(),
      op: creator_id,
      closed: false,
      bans: Vec::new(),
      invites: Vec::new(),
      inbox: Vec::new(),
    }
  }

  pub fn get_id(&self) -> u64 {
    self.idn
  }
  pub fn get_name(&self) -> &str {
    &(self.name)
  }
  pub fn get_idstr(&self) -> &str {
    &(self.idstr)
  }

  pub fn deliver(&self, env: &Env, uid_hash: &mut HashMap<u64, User>) {
    match env.dest {
      End::User(uid) => {
        if let Some(u) = uid_hash.get_mut(&uid) {
          u.deliver(env);
        }
      }
      _ => {
        for uid in &(self.users) {
          if let Some(u) = uid_hash.get_mut(uid) {
            u.deliver(env);
          }
        }
      }
    }
  }

  pub fn enqueue(&mut self, env: Env) {
    self.inbox.push(env);
  }

  /// Deliver all the `Env`s in the queue.
  pub fn deliver_inbox(&mut self, uid_hash: &mut HashMap<u64, User>) {
    for env in self.inbox.drain(..) {
      match env.dest {
        End::User(uid) => {
          if let Some(u) = uid_hash.get_mut(&uid) {
            u.deliver(&env);
          }
        }
        _ => {
          for uid in &(self.users) {
            if let Some(u) = uid_hash.get_mut(uid) {
              u.deliver(&env);
            }
          }
        }
      }
    }
  }

  pub fn join(&mut self, uid: u64) {
    self.users.push(uid);
  }

  pub fn leave(&mut self, uid: u64) {
    self.users.retain(|n| *n != uid);
  }

  pub fn ban(&mut self, uid: u64) {
    self.invites.retain(|n| *n != uid);
    self.bans.push(uid);
  }

  pub fn invite(&mut self, uid: u64) {
    self.bans.retain(|n| *n != uid);
    self.invites.push(uid);
  }

  pub fn set_op(&mut self, uid: u64) {
    self.op = uid;
  }

  pub fn get_op(&self) -> u64 {
    self.op
  }

  pub fn get_users(&self) -> &[u64] {
    &(self.users)
  }

  pub fn is_banned(&self, uid: &u64) -> bool {
    self.bans.contains(uid)
  }

  pub fn is_invited(&self, uid: &u64) -> bool {
    self.invites.contains(uid)
  }
}
