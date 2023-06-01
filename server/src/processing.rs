use common::{
    config::ServerConfig,
    proto::{End, Env, RcvOp, Rcvr, Sndr},
    room::Room,
    user::User,
    util::collapse,
};
use log::{debug, trace, warn};
use smallvec::SmallVec;
use std::{collections::HashMap, time::Instant};

use crate::message::Envs;

const LOGOUTS_SIZE: usize = 8;
const TEXT_SIZE: usize = 2;
const ROOM_SIZE: usize = 64;

struct Context<'a> {
    current_room_id: u64,
    current_user_id: u64,
    users_by_id: &'a mut HashMap<u64, User>,
    user_ids_by_str: &'a mut HashMap<String, u64>,
    rooms_by_id: &'a mut HashMap<u64, Room>,
    room_ids_by_str: &'a mut HashMap<String, u64>,
}

impl<'a> Context<'a> {
    fn get_user_by_id(&self, user_id: u64) -> Result<&User, String> {
        self.users_by_id.get(&user_id).ok_or_else(|| {
            format!(
                "Context {:?}.get_user_by_id({}) returns None",
                &self, &user_id
            )
        })
    }

    fn get_room_by_id(&self, room_id: u64) -> Result<&Room, String> {
        self.rooms_by_id.get(&room_id).ok_or_else(|| {
            format!(
                "Context {:?}.get_room_by_id({}) returns None",
                &self, &room_id
            )
        })
    }

    fn get_user_by_id_mut(&mut self, user_id: u64) -> Result<&mut User, String> {
        self.users_by_id.get_mut(&user_id).ok_or_else(|| {
            format!(
                "Context {{ room_id
                    : {}, user_id: {} }}.get_user_by_id_mut({}) returns None",
                self.current_room_id, self.current_user_id, &user_id
            )
        })
    }

    fn get_room_by_id_mut(&mut self, room_id: u64) -> Result<&mut Room, String> {
        self.rooms_by_id.get_mut(&room_id).ok_or_else(|| {
            format!(
                "Context {{ room_id: {}, user_id: {} }}.get_room_by_id_mut({}) returns None",
                self.current_room_id, self.current_user_id, &room_id
            )
        })
    }

    fn get_user_id_by_str(&self, u_idstr: &str) -> Option<u64> {
        self.user_ids_by_str.get(u_idstr).copied()
    }

    fn get_room_id_by_str(&self, r_idstr: &str) -> Option<u64> {
        self.room_ids_by_str.get(r_idstr).copied()
    }
}

impl std::fmt::Debug for Context<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("room_id", &self.current_room_id)
            .field("user_id", &self.current_user_id)
            .finish()
    }
}

fn match_string<T>(s: &str, hash: &HashMap<String, T>) -> Vec<String> {
    hash.keys().filter(|k| k.starts_with(s)).cloned().collect()
}

fn append_comma_delimited_list<T: AsRef<str>>(base: &mut String, v: &[T]) {
    if !v.is_empty() {
        base.push_str(&v.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(", "));
    }
}

fn first_free_id<T: Sized>(map: &HashMap<u64, T>) -> u64 {
    (0..).find(|n| !map.contains_key(n)).unwrap()
}

fn do_text(context: &mut Context, lines: Vec<String>) -> Result<Envs, String> {
    let user = context.get_user_by_id(context.current_user_id)?;

    let lines_ref: SmallVec<[&str; TEXT_SIZE]> = lines.iter().map(AsRef::as_ref).collect();

    let msg = Sndr::Text {
        who: user.get_name(),
        lines: &lines_ref,
    };

    let env = Env::new(
        End::User(context.current_user_id),
        End::Room(context.current_room_id),
        &msg,
    );

    Ok(Envs::new1(env))
}

/// In response to Msg::Priv { who, text }
fn do_priv(context: &mut Context, who: String, text: String) -> Result<Envs, String> {
    let user = context.get_user_by_id(context.current_user_id)?;

    let recipient = collapse(&who);
    if recipient.is_empty() {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err("The recipient name must have at least one non-whitespace character."),
        );
        return Ok(Envs::new1(env));
    }

    let target_user_id = match context.get_user_id_by_str(&recipient) {
        None => {
            let env = Env::new(
                End::Server,
                End::User(context.current_user_id),
                &Sndr::Err(&format!(
                    "The recipient name \"{}\" is not recognized.",
                    recipient
                )),
            );
            return Ok(Envs::new1(env));
        }
        Some(n) => n,
    };

    let target_user = context.get_user_by_id(target_user_id)?;

    let data: [&str; 2] = [target_user.get_name(), &text];
    let echo_env = Env::new(
        End::Server,
        End::User(context.current_user_id),
        &Sndr::Misc {
            what: "priv_echo",
            data: &data,
            alt: &format!("$ You @ {}: {}", target_user.get_name(), &text),
        },
    );

    let to_env = Env::new(
        End::User(context.current_user_id),
        End::User(target_user_id),
        &Sndr::Priv {
            who: user.get_name(),
            text: &text,
        },
    );

    Ok(Envs::new2(echo_env, to_env))
}

/// In response to Msg::Name(new_candidate)

fn do_name(
    context: &mut Context,
    cfg: &ServerConfig,
    new_candidate: String,
) -> Result<Envs, String> {
    let new_str = collapse(&new_candidate);

    if new_str.is_empty() {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err("Your name must have more whitespace characters."),
        );
        return Ok(Envs::new1(env));
    } else if new_candidate.len() > cfg.max_user_name_length {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err(&format!(
                "Your name cannot be longer than {} characters.",
                cfg.max_user_name_length
            )),
        );
        return Ok(Envs::new1(env));
    }

    if let Some(other_user_id) = context.user_ids_by_str.get(&new_str) {
        let other_user = context.get_user_by_id(*other_user_id)?;
        if *other_user_id != context.current_user_id {
            let env = Env::new(
                End::Server,
                End::User(context.current_user_id),
                &Sndr::Err(&format!(
                    "There is already a user named \"{}\".",
                    other_user.get_name()
                )),
            );
            return Ok(Envs::new1(env));
        }
    }

    let (old_idstr, new_idstr, env) = {
        let mu = context.get_user_by_id_mut(context.current_user_id)?;
        let old_name = mu.get_name().to_string();
        let old_idstr = mu.get_idstr().to_string();

        mu.set_name(&new_candidate);
        let new_idstr = mu.get_idstr().to_string();
        let data: [&str; 2] = [old_name.as_str(), new_candidate.as_str()];

        let env = Env::new(
            End::Server,
            End::Room(context.current_room_id),
            &Sndr::Misc {
                what: "name",
                data: &data,
                alt: &format!("{} is now known as {}.", &old_name, &new_candidate),
            },
        );
        (old_idstr, new_idstr, env)
    };

    context.user_ids_by_str.remove(&old_idstr);
    context
        .user_ids_by_str
        .insert(new_idstr, context.current_user_id);

    Ok(Envs::new1(env))
}

/// In response to Msg::Join(room_name)

fn do_join(context: &mut Context, cfg: &ServerConfig, room_name: String) -> Result<Envs, String> {
    let normalized_room_name = collapse(&room_name);
    if normalized_room_name.is_empty() {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err("A room name must have more non-whitespace characters."),
        );
        return Ok(Envs::new1(env));
    } else if room_name.len() > cfg.max_room_name_length {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err(&format!(
                "Room names cannot be longer than {} characters.",
                cfg.max_room_name_length
            )),
        );
        return Ok(Envs::new1(env));
    }

    let target_room_id = context
        .get_room_id_by_str(&normalized_room_name)
        .unwrap_or_else(|| {
            let new_id = first_free_id(context.rooms_by_id);
            let new_room = Room::new(new_id, room_name.clone(), context.current_user_id);
            context.room_ids_by_str.insert(normalized_room_name, new_id);
            context.rooms_by_id.insert(new_id, new_room);
            let mu = context.get_user_by_id_mut(context.current_user_id).unwrap();
            mu.deliver_msg(&Sndr::Info(&format!("You create room \"{}\".", room_name)));
            new_id
        });

    let user = context.get_user_by_id(context.current_user_id)?;
    let username = user.get_name().to_string();
    let user_id = context.current_user_id;
    let room_id = context.current_room_id;

    let target_room = context.get_room_by_id_mut(target_room_id)?;

    if target_room_id == room_id {
        let env = Env::new(
            End::Server,
            End::User(user_id),
            &Sndr::Info(&format!(
                "You are already in \"{}\".",
                target_room.get_name()
            )),
        );
        return Ok(Envs::new1(env));
    } else if target_room.is_banned(&user_id) {
        let env = Env::new(
            End::Server,
            End::User(user_id),
            &Sndr::Info(&format!(
                "You are banned from \"{}\".",
                target_room.get_name()
            )),
        );
        return Ok(Envs::new1(env));
    } else if target_room.closed && !target_room.is_invited(&user_id) {
        let env = Env::new(
            End::Server,
            End::User(user_id),
            &Sndr::Info(&format!("\"{}\" is closed.", target_room.get_name())),
        );
        return Ok(Envs::new1(env));
    }

    target_room.join(user_id);
    let data: [&str; 2] = [&username, target_room.get_name()];
    let join_env = Env::new(
        End::Server,
        End::Room(target_room_id),
        &Sndr::Misc {
            what: "join",
            data: &data,
            alt: &format!("{} joins {}.", username, target_room.get_name()),
        },
    );
    target_room.enqueue(join_env);

    let current_room = context.get_room_by_id_mut(room_id)?;
    let leave_data: [&str; 2] = [&username, "[ moved to another room ]"];
    let leave_env = Env::new(
        End::Server,
        End::Room(target_room_id),
        &Sndr::Misc {
            what: "leave",
            data: &leave_data,
            alt: &format!("{} moved to another room.", username),
        },
    );
    current_room.leave(user_id);
    Ok(Envs::new1(leave_env))
}

/// In response to Msg::Block(username)

fn do_block(context: &mut Context, username: String) -> Result<Envs, String> {
    let normalized_username = collapse(&username);
    if normalized_username.is_empty() {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err("A user name must have more non-whitespace characters."),
        );
        return Ok(Envs::new1(env));
    }

    let othe_user_id = match context.user_ids_by_str.get(&normalized_username) {
        None => {
            let env = Env::new(
                End::Server,
                End::User(context.current_user_id),
                &Sndr::Info(&format!(
                    "No users matching the pattern \"{}\".",
                    normalized_username
                )),
            );
            return Ok(Envs::new1(env));
        }
        Some(n) => *n,
    };

    if othe_user_id == context.current_user_id {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err("You shouldn't block yourself."),
        );
        return Ok(Envs::new1(env));
    }

    let blocked_name = context
        .users_by_id
        .get(&othe_user_id)
        .ok_or_else(|| {
            format!(
                "do_block(r {}, u {}): no target User {}",
                context.current_room_id, context.current_user_id, othe_user_id
            )
        })?
        .get_name()
        .to_string();

    let user = context.get_user_by_id_mut(context.current_user_id)?;
    let could_block: bool = user.block_id(othe_user_id);

    if could_block {
        user.deliver_msg(&Sndr::Info(&format!(
            "You are now blocking {}.",
            &blocked_name
        )));
    } else {
        user.deliver_msg(&Sndr::Err(&format!(
            "You are already blocking {}.",
            &blocked_name
        )));
    };

    Ok(Envs::new0())
}

/// In response to Msg::Unblock(username)

fn do_unblock(context: &mut Context, username: String) -> Result<Envs, String> {
    let normalized_username = collapse(&username);
    if normalized_username.is_empty() {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err("That cannot be anyone's user name."),
        );
        return Ok(Envs::new1(env));
    }

    let other_user_id = match context.user_ids_by_str.get(&normalized_username) {
        None => {
            let env = Env::new(
                End::Server,
                End::User(context.current_user_id),
                &Sndr::Info(&format!(
                    "No users matching the pattern \"{}\".",
                    normalized_username
                )),
            );
            return Ok(Envs::new1(env));
        }
        Some(n) => *n,
    };

    if other_user_id == context.current_user_id {
        let env = Env::new(
            End::Server,
            End::User(context.current_user_id),
            &Sndr::Err("You couldn't block yourself; you can't unblock yourself."),
        );
        return Ok(Envs::new1(env));
    }

    let blocked_name = context
        .users_by_id
        .get(&other_user_id)
        .ok_or_else(|| {
            format!(
                "do_unblock(r {}, u {}): no target User {}",
                context.current_room_id, context.current_user_id, other_user_id
            )
        })?
        .get_name()
        .to_string();

    let user = context.get_user_by_id_mut(context.current_user_id)?;
    let could_unblock: bool = user.unblock_id(other_user_id);
    if could_unblock {
        user.deliver_msg(&Sndr::Info(&format!(
            "You are no longer blocking {}.",
            &blocked_name
        )));
    } else {
        user.deliver_msg(&Sndr::Err(&format!(
            "You are not blocking {}.",
            &blocked_name
        )));
    }

    Ok(Envs::new0())
}

/// In response to Msg::Logout(salutation)

fn do_logout(context: &mut Context, salutation: String) -> Result<Envs, String> {
    let current_room = context
        .rooms_by_id
        .get_mut(&context.current_room_id)
        .ok_or_else(|| {
            format!(
                "do_logout(r {}, u {}): no Room {}",
                context.current_room_id, context.current_user_id, context.current_room_id
            )
        })?;

    current_room.leave(context.current_user_id);

    let mut user = context
        .users_by_id
        .remove(&context.current_user_id)
        .ok_or_else(|| {
            format!(
                "do_logout(r {}, u {}): no User {}",
                context.current_room_id, context.current_user_id, context.current_user_id
            )
        })?;

    let _ = context.user_ids_by_str.remove(user.get_idstr());
    user.logout("You have logged out.");

    let data: [&str; 2] = [user.get_name(), &salutation];
    let env = Env::new(
        End::Server,
        End::Room(context.current_room_id),
        &Sndr::Misc {
            what: "leave",
            alt: &format!("{} left: {}", user.get_name(), salutation),
            data: &data,
        },
    );
    current_room.enqueue(env);

    Ok(Envs::new0())
}

/// In response to Msg::Query { what, arg }

fn do_query(context: &mut Context, what: String, arg: String) -> Result<Envs, String> {
    match what.as_str() {
        "addr" => {
            let current_user = context.get_user_by_id_mut(context.current_user_id)?;
            let (addr_str, alt_str): (String, String) = match current_user.get_addr() {
                None => (
                    "???".to_string(),
                    "Your public address cannot be determined.".to_string(),
                ),
                Some(address) => {
                    let addr_string = format!("Your public address is {}.", &address);
                    (address, addr_string)
                }
            };
            let data: [&str; 1] = [&addr_str];
            let msg = Sndr::Misc {
                what: "addr",
                data: &data,
                alt: &alt_str,
            };
            current_user.deliver_msg(&msg);
            Ok(Envs::new0())
        }

        "roster" => {
            let current_room = context.get_room_by_id(context.current_room_id)?;
            let op_id = current_room.get_op();
            let mut names_list: SmallVec<[&str; ROOM_SIZE]> =
                SmallVec::with_capacity(current_room.get_users().len());

            for user_id in current_room.get_users().iter().rev() {
                if *user_id != op_id {
                    match context.users_by_id.get(user_id) {
                        None => {
                            warn!(
                                "do_query(r {}, u{} {:?}): no User {}",
                                context.current_room_id, context.current_user_id, &what, user_id
                            );
                        }
                        Some(u) => {
                            names_list.push(u.get_name());
                        }
                    }
                }
            }

            let mut alternative_string: String;
            if op_id == 0 {
                alternative_string = format!("{} roster: ", current_room.get_name());
                append_comma_delimited_list(&mut alternative_string, &names_list);
            } else {
                let op_name = match context.users_by_id.get(&op_id) {
                    None => "[ ??? ]",
                    Some(u) => u.get_name(),
                };
                alternative_string = format!(
                    "{} roster: {} (operator) ",
                    current_room.get_name(),
                    op_name
                );
                append_comma_delimited_list(&mut alternative_string, &names_list);

                names_list.push(op_name);
            }

            let mut names_ref: SmallVec<[&str; ROOM_SIZE]> =
                SmallVec::with_capacity(names_list.len());
            for name in names_list.iter().rev() {
                names_ref.push(name);
            }

            let env = Env::new(
                End::Server,
                End::User(context.current_user_id),
                &Sndr::Misc {
                    what: "roster",
                    data: &names_ref,
                    alt: &alternative_string,
                },
            );
            Ok(Envs::new1(env))
        }

        "who" => {
            let normalized_arg = collapse(&arg);
            let matches = match_string(&normalized_arg, context.user_ids_by_str);

            let env: Env = if matches.is_empty() {
                Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info(&format!(
                        "No users matching the pattern \"{}\".",
                        &normalized_arg
                    )),
                )
            } else {
                let mut alternative_string = String::from("Matching names: ");
                append_comma_delimited_list(&mut alternative_string, &matches);
                let listref: Vec<&str> = matches.iter().map(|x| x.as_str()).collect();
                Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Misc {
                        what: "who",
                        data: &listref,
                        alt: &alternative_string,
                    },
                )
            };
            Ok(Envs::new1(env))
        }

        "rooms" => {
            let normalized_arg = collapse(&arg);
            let matches = match_string(&normalized_arg, context.room_ids_by_str);
            let env: Env = if matches.is_empty() {
                Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info(&format!(
                        "No Rooms matching the pattern \"{}\".",
                        &normalized_arg
                    )),
                )
            } else {
                let mut alternative_string = String::from("Matching Rooms: ");
                append_comma_delimited_list(&mut alternative_string, &matches);
                let listref: Vec<&str> = matches.iter().map(|x| x.as_str()).collect();
                Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Misc {
                        what: "rooms",
                        data: &listref,
                        alt: &alternative_string,
                    },
                )
            };
            Ok(Envs::new1(env))
        }

        unknown_query_type => {
            let env = Env::new(
                End::Server,
                End::User(context.current_user_id),
                &Sndr::Err(&format!(
                    "Unknown \"Query\" type: \"{}\".",
                    unknown_query_type
                )),
            );
            Ok(Envs::new1(env))
        }
    }
}

fn do_op(context: &mut Context, op: RcvOp) -> Result<Envs, String> {
    {
        let current_room = context.get_room_by_id(context.current_room_id)?;
        if current_room.get_op() != context.current_user_id {
            let env = Env::new(
                End::Server,
                End::User(context.current_user_id),
                &Sndr::Err("You are not the operator of this Room."),
            );
            return Ok(Envs::new1(env));
        }
    }

    let (user_id, room_id) = (context.current_user_id, context.current_room_id);
    let op_name = {
        let user = context.get_user_by_id(context.current_user_id)?;
        user.get_name().to_string()
    };

    match op {
        RcvOp::Open => {
            let current_room = context.get_room_by_id_mut(room_id)?;
            if current_room.closed {
                current_room.closed = false;
                let env = Env::new(
                    End::Server,
                    End::Room(room_id),
                    &Sndr::Info(&format!(
                        "{} has opened {}.",
                        &op_name,
                        current_room.get_name()
                    )),
                );
                Ok(Envs::new1(env))
            } else {
                let env = Env::new(
                    End::Server,
                    End::User(user_id),
                    &Sndr::Info(&format!("{} is already open.", current_room.get_name())),
                );
                Ok(Envs::new1(env))
            }
        }

        RcvOp::Close => {
            let current_room = context.get_room_by_id_mut(room_id)?;
            if current_room.closed {
                let env = Env::new(
                    End::Server,
                    End::User(user_id),
                    &Sndr::Info(&format!("{} is already closed.", current_room.get_name())),
                );
                Ok(Envs::new1(env))
            } else {
                current_room.closed = true;
                let env = Env::new(
                    End::Server,
                    End::Room(room_id),
                    &Sndr::Info(&format!(
                        "{} has closed {}.",
                        &op_name,
                        current_room.get_name()
                    )),
                );
                Ok(Envs::new1(env))
            }
        }

        RcvOp::Give(ref new_name) => {
            let normalized_username = collapse(new_name);
            if normalized_username.is_empty() {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Err("That cannot be anyone's user name."),
                );
                return Ok(Envs::new1(env));
            }

            let other_user_id = match context.user_ids_by_str.get(&normalized_username) {
                None => {
                    let env = Env::new(
                        End::Server,
                        End::User(context.current_user_id),
                        &Sndr::Info(&format!(
                            "No users matching the pattern \"{}\".",
                            &normalized_username
                        )),
                    );
                    return Ok(Envs::new1(env));
                }
                Some(id) => *id,
            };

            if other_user_id == context.current_user_id {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info("You are already the operator of this room."),
                );
                return Ok(Envs::new1(env));
            }

            let other_username = {
                let user = context.get_user_by_id(other_user_id)?;
                user.get_name().to_string()
            };

            let current_room = context.get_room_by_id_mut(room_id)?;
            if !current_room.get_users().contains(&other_user_id) {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info(&format!(
                        "{} must be in the room to transfer ownership.",
                        &other_username
                    )),
                );
                return Ok(Envs::new1(env));
            }
            current_room.set_op(other_user_id);
            let data: [&str; 2] = [&other_username, current_room.get_name()];
            let env = Env::new(
                End::Server,
                End::Room(room_id),
                &Sndr::Misc {
                    what: "new_op",
                    alt: &format!(
                        "{} is now the operator of {}.",
                        &other_username,
                        current_room.get_name()
                    ),
                    data: &data,
                },
            );
            Ok(Envs::new1(env))
        }

        RcvOp::Invite(ref username) => {
            let normalized_username = collapse(username);
            if normalized_username.is_empty() {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info("That cannot be anyone's user name."),
                );
                return Ok(Envs::new1(env));
            }

            let other_user_id = match context.user_ids_by_str.get(&normalized_username) {
                None => {
                    let env = Env::new(
                        End::Server,
                        End::User(context.current_user_id),
                        &Sndr::Info(&format!(
                            "No users matching the pattern \"{}\".",
                            &normalized_username
                        )),
                    );
                    return Ok(Envs::new1(env));
                }
                Some(n) => *n,
            };

            let current_room = match context.rooms_by_id.get_mut(&context.current_room_id) {
                None => {
                    return Err(format!(
                        "do_op(r {}, u {}, {:?}): no Room {}",
                        context.current_room_id,
                        context.current_user_id,
                        &op,
                        context.current_room_id
                    ));
                }
                Some(room) => room,
            };

            if other_user_id == context.current_user_id {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info(&format!(
                        "You are already allowed in {}.",
                        current_room.get_name()
                    )),
                );
                return Ok(Envs::new1(env));
            };

            let other_user = match context.users_by_id.get_mut(&other_user_id) {
                None => {
                    return Err(format!(
                        "do_op(r {}, u {}, {:?}): no target User {}",
                        context.current_room_id, context.current_user_id, &op, other_user_id
                    ));
                }
                Some(u) => u,
            };

            if current_room.is_invited(&other_user_id) {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info(&format!(
                        "{} has already been invited to {}.",
                        other_user.get_name(),
                        current_room.get_name()
                    )),
                );
                return Ok(Envs::new1(env));
            };
            current_room.invite(other_user_id);

            let inviter_env: Env;
            if current_room.get_users().contains(&other_user_id) {
                inviter_env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info(&format!(
                        "{} may now return to {} even when closed.",
                        other_user.get_name(),
                        current_room.get_name()
                    )),
                );
                other_user.deliver_msg(&Sndr::Info(&format!(
                    "You have been invited to return to {} even if it closes.",
                    current_room.get_name()
                )));
            } else {
                inviter_env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info(&format!(
                        "You invite {} to join {}.",
                        other_user.get_name(),
                        current_room.get_name()
                    )),
                );
                other_user.deliver_msg(&Sndr::Info(&format!(
                    "You have been invited to join {}.",
                    current_room.get_name()
                )));
            }
            Ok(Envs::new1(inviter_env))
        }

        RcvOp::Kick(ref username) => {
            let normalized_username = collapse(username);
            if normalized_username.is_empty() {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info("That cannot be anyone's user name."),
                );
                return Ok(Envs::new1(env));
            }

            let other_user_id = match context.user_ids_by_str.get(&normalized_username) {
                None => {
                    let env = Env::new(
                        End::Server,
                        End::User(context.current_user_id),
                        &Sndr::Info(&format!(
                            "No users matching the pattern \"{}\".",
                            &normalized_username
                        )),
                    );
                    return Ok(Envs::new1(env));
                }
                Some(id) => *id,
            };

            if other_user_id == context.current_user_id {
                let env = Env::new(
                    End::Server,
                    End::User(context.current_user_id),
                    &Sndr::Info("You cannot kick yourself."),
                );
                return Ok(Envs::new1(env));
            }

            let target_user = match context.users_by_id.get_mut(&other_user_id) {
                None => {
                    return Err(format!(
                        "do_op(r {}, u {}, {:?}): no target User {}",
                        context.current_room_id, context.current_user_id, &op, other_user_id
                    ));
                }
                Some(user) => user,
            };

            let in_room: bool;
            let mut current_room_name = String::default();

            {
                let room = match context.rooms_by_id.get_mut(&context.current_room_id) {
                    None => {
                        return Err(format!(
                            "do_op(r {}, u {}, {:?}): no Room {}",
                            context.current_room_id,
                            context.current_user_id,
                            &op,
                            context.current_room_id
                        ));
                    }
                    Some(room) => room,
                };

                if room.is_banned(&other_user_id) {
                    let env = Env::new(
                        End::Server,
                        End::User(context.current_user_id),
                        &Sndr::Info(&format!(
                            "{} is already banned from {}.",
                            target_user.get_name(),
                            room.get_name()
                        )),
                    );
                    return Ok(Envs::new1(env));
                };

                room.ban(other_user_id);
                in_room = room.get_users().contains(&other_user_id);

                if !in_room {
                    if !room.get_users().contains(&other_user_id) {
                        let env = Env::new(
                            End::Server,
                            End::User(context.current_user_id),
                            &Sndr::Info(&format!(
                                "You have banned {} from {}.",
                                target_user.get_name(),
                                room.get_name()
                            )),
                        );
                        return Ok(Envs::new1(env));
                    }
                } else {
                    let alternative_string =
                        format!("You have been kicked from {}.", room.get_name());
                    let data: [&str; 1] = [room.get_name()];
                    let to_kicked = Sndr::Misc {
                        what: "kick_you",
                        alt: &alternative_string,
                        data: &data,
                    };
                    target_user.deliver_msg(&to_kicked);
                    room.leave(other_user_id);

                    current_room_name = room.get_name().to_string();
                }
            }

            let lobby = context.rooms_by_id.get_mut(&0).unwrap();
            lobby.join(other_user_id);
            let data: [&str; 2] = [target_user.get_name(), lobby.get_name()];
            let to_lobby = Env::new(
                End::Server,
                End::Room(context.current_room_id),
                &Sndr::Misc {
                    what: "join",
                    data: &data,
                    alt: &format!("{} joined {}.", target_user.get_name(), lobby.get_name()),
                },
            );
            lobby.enqueue(to_lobby);

            let data: [&str; 2] = [target_user.get_name(), &current_room_name];
            let env = Env::new(
                End::Server,
                End::Room(context.current_room_id),
                &Sndr::Misc {
                    what: "kick_other",
                    data: &data,
                    alt: &format!(
                        "{} has been kicked from {}.",
                        target_user.get_name(),
                        &current_room_name
                    ),
                },
            );

            Ok(Envs::new1(env))
        }
    }
}

pub fn process_room(
    room_id: u64,
    current_time: Instant,
    users_by_id: &mut HashMap<u64, User>,
    user_ids_by_str: &mut HashMap<String, u64>,
    rooms_by_id: &mut HashMap<u64, Room>,
    room_ids_by_str: &mut HashMap<String, u64>,
    cfg: &ServerConfig,
) -> Result<(), String> {
    let mut user_id_list: SmallVec<[u64; ROOM_SIZE]>;
    {
        match rooms_by_id.get(&room_id) {
            None => {
                return Err(format!("Room {} doesn't exist.", &room_id));
            }
            Some(r) => {
                user_id_list = SmallVec::from_slice(r.get_users());
            }
        }
    }

    let mut context = Context {
        current_room_id: room_id,
        current_user_id: 0,
        users_by_id,
        user_ids_by_str,
        rooms_by_id,
        room_ids_by_str,
    };

    let mut envs: Envs = Envs::new0();
    let mut logout_users: SmallVec<[(u64, &str); LOGOUTS_SIZE]> = SmallVec::new();

    for user_id in &user_id_list {
        let received_message: Rcvr;
        {
            let user = match context.users_by_id.get_mut(user_id) {
                None => {
                    debug!("process_room({}): user {} doesn't exist", &room_id, user_id);
                    continue;
                }
                Some(user) => user,
            };

            let over_quota = user.get_byte_quota() > cfg.byte_limit;
            user.drain_byte_quota(cfg.byte_tick);
            if over_quota && user.get_byte_quota() <= cfg.byte_limit {
                let msg = Sndr::Err("You may send messages again.");
                user.deliver_msg(&msg);
            }

            match user.try_get() {
                None => {
                    let last = user.get_last_data_time();
                    match current_time.checked_duration_since(last) {
                        Some(x) if x > cfg.time_to_kick => {
                            logout_users.push((
                                *user_id,
                                "Too long since server received data from the client.",
                            ));
                        }
                        Some(x) if x > cfg.time_to_ping => {
                            user.deliver_msg(&Sndr::Ping);
                        }
                        _ => {}
                    }
                    continue;
                }
                Some(msg) => {
                    if !over_quota {
                        received_message = msg;
                        if user.get_byte_quota() > cfg.byte_limit {
                            let msg = Sndr::Err("You have exceeded your data quota and your messages will be ignored for a short time.");
                            user.deliver_msg(&msg);
                        }
                    } else {
                        continue;
                    }
                }
            }

            if user.has_errors() {
                let user_error = user.get_errors();
                warn!(
                    "User {} being logged out for error(s): {}",
                    user_id, &user_error
                );
                logout_users.push((*user_id, "Communication error."));
            }
        }

        context.current_user_id = *user_id;

        let processed_result = match received_message {
            Rcvr::Text { lines, .. } => do_text(&mut context, lines),
            Rcvr::Priv { who, text } => do_priv(&mut context, who, text),
            Rcvr::Name(new_candidate) => do_name(&mut context, cfg, new_candidate),
            Rcvr::Join(room_name) => do_join(&mut context, cfg, room_name),
            Rcvr::Block(username) => do_block(&mut context, username),
            Rcvr::Unblock(username) => do_unblock(&mut context, username),
            Rcvr::Logout(salutation) => do_logout(&mut context, salutation),
            Rcvr::Query { what, arg } => do_query(&mut context, what, arg),
            Rcvr::Op(op) => do_op(&mut context, op),
            _ => Ok(Envs::new0()),
        };

        match processed_result {
            Err(e) => {
                trace!("{}", &e);
            }
            Ok(mut result_envs) => {
                let envs_mut = envs.as_mut();
                for env in result_envs.as_mut().drain(..) {
                    envs_mut.push(env);
                }
            }
        }
    }

    for (user_id, errmsg) in logout_users.iter() {
        if let Some(mut user) = context.users_by_id.remove(user_id) {
            context.user_ids_by_str.remove(user.get_idstr());
            let msg = Sndr::Logout(errmsg);
            user.deliver_msg(&msg);
            let data: [&str; 2] = [user.get_name(), "[ disconnected by server ]"];
            let env = Env::new(
                End::Server,
                End::Room(context.current_room_id),
                &Sndr::Misc {
                    what: "leave",
                    data: &data,
                    alt: &format!("{} has been disconnected from the server.", user.get_name()),
                },
            );
            envs.as_mut().push(env);
        } else {
            warn!(
                "process_room({} ...): logouts.drain(): no User {}",
                context.current_room_id, user_id
            );
        }
    }

    // Change the room OP if necessary.
    if room_id != 0 {
        let room = context.rooms_by_id.get_mut(&room_id).unwrap();
        let op_id = room.get_op();
        let op_still_here = room.get_users().contains(&op_id);
        if !op_still_here {
            if let Some(potential_new_operator_id) = room.get_users().first() {
                if let Some(u) = context.users_by_id.get(potential_new_operator_id) {
                    let nid = *potential_new_operator_id;
                    room.set_op(nid);
                    let env = Env::new(
                        End::Server,
                        End::Room(room_id),
                        &Sndr::Info(&format!("{} is now the Room operator.", u.get_name())),
                    );
                    envs.as_mut().push(env);
                }
            }
        }
    }

    {
        let room = context.rooms_by_id.get_mut(&room_id).unwrap();
        for (user_id, _) in logout_users.drain(..) {
            room.leave(user_id);
        }
        room.deliver_inbox(context.users_by_id);
        for env in envs.as_ref() {
            room.deliver(env, context.users_by_id);
        }
        user_id_list.clear();
        user_id_list.extend_from_slice(room.get_users());
        for user_id in user_id_list.iter_mut() {
            if let Some(user) = users_by_id.get_mut(user_id) {
                user.send();
            }
        }
    }

    Ok(())
}
