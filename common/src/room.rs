use std::collections::HashMap;

use super::proto::{End, Env};
use super::user::{collapse, User};

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

  /** Deliver an `Env` to the appropriate `End`: either the entire
  `Room`, or if its `dest` field is `End::User(n)`, just that user.
  */
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

  /** Push an `Env` on the queue to be delivered next time
  `.deliver_inbox(...)` (below) is called.
  */
  pub fn enqueue(&mut self, env: Env) {
    self.inbox.push(env);
  }

  /** Deliver all of the `Env`s that have been `.enqueue(...)`'d (above). */
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

  /** Add the given user ID to the list of `User`s "in" the `Room`. */
  pub fn join(&mut self, uid: u64) {
    self.users.push(uid);
  }
  /** Remove the given user ID (if present) from the list of `User`s that
  are "in" the `Room` */
  pub fn leave(&mut self, uid: u64) {
    self.users.retain(|n| *n != uid);
  }

  pub fn ban(&mut self, uid: u64) {
    self.invites.retain(|n| *n != uid);
    self.bans.push(uid);
  }

  /** Add the given user ID to the list of `User`s "invited" to the
  `Room`, meaning that they may enter even if the operator has
  `.closed` it.

  This also removes the given user ID from the "banned" (see `.ban(...)`,
  above) list, if present.
  */
  pub fn invite(&mut self, uid: u64) {
    self.bans.retain(|n| *n != uid);
    self.invites.push(uid);
  }

  /** Set the `User` with the given user ID to be the `Room`'s operator. */
  pub fn set_op(&mut self, uid: u64) {
    self.op = uid;
  }
  /** Return the user ID of the `Room`'s current operator. */
  pub fn get_op(&self) -> u64 {
    self.op
  }

  /** Return the list of user IDs of `User`s in the `Room`. */
  pub fn get_users(&self) -> &[u64] {
    &(self.users)
  }

  /** Return whether the `User` with the given ID is "banned" (see
  `.ban(...)`, above). */
  pub fn is_banned(&self, uid: &u64) -> bool {
    self.bans.contains(uid)
  }

  /** Return whether the `User` with the given ID is "invited" (see
  `.invite(...)`, above). */
  pub fn is_invited(&self, uid: &u64) -> bool {
    self.invites.contains(uid)
  }
}
