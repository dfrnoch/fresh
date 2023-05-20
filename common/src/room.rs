use std::collections::HashMap;

use crate::util::collapse;

use super::proto::{End, Env};
use super::user::User;

#[derive(Debug)]
pub struct Room {
    id: u64,
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
            id,
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
        self.id
    }
    pub fn get_name(&self) -> &str {
        &(self.name)
    }
    pub fn get_idstr(&self) -> &str {
        &(self.idstr)
    }

    pub fn deliver(&self, env: &Env, user_id_hash: &mut HashMap<u64, User>) {
        match env.dest {
            End::User(user_id) => {
                if let Some(user) = user_id_hash.get_mut(&user_id) {
                    user.deliver(env);
                }
            }
            _ => {
                for user_id in &self.users {
                    if let Some(user) = user_id_hash.get_mut(user_id) {
                        user.deliver(env);
                    }
                }
            }
        }
    }

    pub fn enqueue(&mut self, env: Env) {
        self.inbox.push(env);
    }

    /// Deliver all the `Env`s in the queue.
    pub fn deliver_inbox(&mut self, user_id_hash: &mut HashMap<u64, User>) {
        while let Some(env) = self.inbox.pop() {
            match env.dest {
                End::User(user_id) => {
                    if let Some(user) = user_id_hash.get_mut(&user_id) {
                        user.deliver(&env);
                    }
                }
                _ => {
                    for user_id in &self.users {
                        if let Some(user) = user_id_hash.get_mut(user_id) {
                            user.deliver(&env);
                        }
                    }
                }
            }
        }
    }

    pub fn join(&mut self, user_id: u64) {
        self.users.push(user_id);
    }

    pub fn leave(&mut self, user_id: u64) {
        self.users.retain(|n| *n != user_id);
    }

    pub fn ban(&mut self, user_id: u64) {
        self.invites.retain(|n| *n != user_id);
        self.bans.push(user_id);
    }

    pub fn invite(&mut self, user_id: u64) {
        self.bans.retain(|n| *n != user_id);
        self.invites.push(user_id);
    }

    pub fn set_op(&mut self, user_id: u64) {
        self.op = user_id;
    }

    pub fn get_op(&self) -> u64 {
        self.op
    }

    pub fn get_users(&self) -> &[u64] {
        &(self.users)
    }

    pub fn is_banned(&self, user_id: &u64) -> bool {
        self.bans.contains(user_id)
    }

    pub fn is_invited(&self, user_id: &u64) -> bool {
        self.invites.contains(user_id)
    }
}
