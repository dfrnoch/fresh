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
    rid: u64,
    uid: u64,
    user_map: &'a mut HashMap<u64, User>,
    ustr_map: &'a mut HashMap<String, u64>,
    room_map: &'a mut HashMap<u64, Room>,
    rstr_map: &'a mut HashMap<String, u64>,
}

impl Context<'_> {
    fn gumap(&self, uid: u64) -> Result<&User, String> {
        match self.user_map.get(&uid) {
            None => Err(format!("{:?}.gumap(&{}) returns None", &self, &uid)),
            Some(u) => Ok(u),
        }
    }

    fn grmap(&self, rid: u64) -> Result<&Room, String> {
        match self.room_map.get(&rid) {
            None => Err(format!("{:?}.grmap(&{}) return None", &self, &rid)),
            Some(r) => Ok(r),
        }
    }

    fn gumap_mut(&mut self, uid: u64) -> Result<&mut User, String> {
        if let Some(u) = self.user_map.get_mut(&uid) {
            return Ok(u);
        }
        Err(format!(
            "Context {{ rid: {}, uid: {} }}.gumap_mut(&{}) returns None",
            self.rid, self.uid, &uid
        ))
    }

    fn grmap_mut(&mut self, rid: u64) -> Result<&mut Room, String> {
        if let Some(r) = self.room_map.get_mut(&rid) {
            return Ok(r);
        }
        Err(format!(
            "Context {{ rid: {}, uid: {} }}.grmap_mut(&{}) returns None",
            self.rid, self.uid, &rid
        ))
    }

    fn gustr(&self, u_idstr: &str) -> Option<u64> {
        self.ustr_map.get(u_idstr).copied()
    }
    fn grstr(&self, r_idstr: &str) -> Option<u64> {
        self.rstr_map.get(r_idstr).copied()
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

fn match_string<T>(s: &str, hash: &HashMap<String, T>) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for k in hash.keys() {
        if k.starts_with(s) {
            v.push(String::from(k));
        }
    }
    v
}

fn append_comma_delimited_list<T: AsRef<str>>(base: &mut String, v: &[T]) {
    let mut v_iter = v.iter();
    if let Some(x) = v_iter.next() {
        base.push_str(x.as_ref());
    }
    for x in v_iter {
        base.push_str(", ");
        base.push_str(x.as_ref());
    }
}

fn first_free_id<T: Sized>(map: &HashMap<u64, T>) -> u64 {
    let mut n: u64 = 0;
    while map.get(&n).is_some() {
        n += 1;
    }
    n
}

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

    Ok(Envs::new1(env))
}

/// In response to Msg::Priv { who, text }

fn do_priv(ctxt: &mut Context, who: String, text: String) -> Result<Envs, String> {
    let u = ctxt.gumap(ctxt.uid)?;

    let to_tok = collapse(&who);
    if to_tok.is_empty() {
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

    Ok(Envs::new2(echo_env, to_env))
}

/// In response to Msg::Name(new_candidate)

fn do_name(ctxt: &mut Context, cfg: &ServerConfig, new_candidate: String) -> Result<Envs, String> {
    let new_str = collapse(&new_candidate);
    if new_str.is_empty() {
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

    if let Some(ouid) = ctxt.ustr_map.get(&new_str) {
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
    let _ = ctxt.ustr_map.remove(&old_idstr);

    ctxt.ustr_map.insert(new_idstr, ctxt.uid);
    Ok(Envs::new1(env))
}

/// In response to Msg::Join(room_name)

fn do_join(ctxt: &mut Context, cfg: &ServerConfig, room_name: String) -> Result<Envs, String> {
    let collapsed = collapse(&room_name);
    if collapsed.is_empty() {
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
            let new_id = first_free_id(ctxt.room_map);
            let new_room = Room::new(new_id, room_name.clone(), ctxt.uid);
            ctxt.rstr_map.insert(collapsed, new_id);
            ctxt.room_map.insert(new_id, new_room);
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
    Ok(Envs::new1(leave_env))
}

/// In response to Msg::Block(user_name)

fn do_block(ctxt: &mut Context, user_name: String) -> Result<Envs, String> {
    let collapsed = collapse(&user_name);
    if collapsed.is_empty() {
        let env = Env::new(
            End::Server,
            End::User(ctxt.uid),
            &Sndr::Err("That cannot be anyone's user name."),
        );
        return Ok(Envs::new1(env));
    }
    let ouid = match ctxt.ustr_map.get(&collapsed) {
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

    let blocked_name = match ctxt.user_map.get(&ouid) {
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

    Ok(Envs::new0())
}

/// In response to Msg::Unblock(user_name)

fn do_unblock(ctxt: &mut Context, user_name: String) -> Result<Envs, String> {
    let collapsed = collapse(&user_name);
    if collapsed.is_empty() {
        let env = Env::new(
            End::Server,
            End::User(ctxt.uid),
            &Sndr::Err("That cannot be anyone's user name."),
        );
        return Ok(Envs::new1(env));
    }
    let ouid = match ctxt.ustr_map.get(&collapsed) {
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

    let blocked_name = match ctxt.user_map.get(&ouid) {
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

    Ok(Envs::new0())
}

/// In response to Msg::Logout(salutation)

fn do_logout(ctxt: &mut Context, salutation: String) -> Result<Envs, String> {
    let mr = match ctxt.room_map.get_mut(&ctxt.rid) {
        None => {
            return Err(format!(
                "do_logout(r {}, u {}): no Room {}",
                ctxt.rid, ctxt.uid, ctxt.rid
            ));
        }
        Some(r) => r,
    };
    mr.leave(ctxt.uid);

    let mut mu = match ctxt.user_map.remove(&ctxt.uid) {
        None => {
            return Err(format!(
                "do_logout(r {}, u {}): no User {}",
                ctxt.rid, ctxt.uid, ctxt.uid
            ));
        }
        Some(u) => u,
    };
    let _ = ctxt.ustr_map.remove(mu.get_idstr());
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
            Ok(Envs::new0())
        }

        "roster" => {
            let r = ctxt.grmap(ctxt.rid)?;
            let op_id = r.get_op();
            let mut names_list: SmallVec<[&str; ROOM_SIZE]> =
                SmallVec::with_capacity(r.get_users().len());

            for uid in r.get_users().iter().rev() {
                if *uid != op_id {
                    match ctxt.user_map.get(uid) {
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
            if op_id == 0 {
                altstr = format!("{} roster: ", r.get_name());
                append_comma_delimited_list(&mut altstr, &names_list);
            } else {
                let op_name = match ctxt.user_map.get(&op_id) {
                    None => "[ ??? ]",
                    Some(u) => u.get_name(),
                };
                altstr = format!("{} roster: {} (operator) ", r.get_name(), op_name);
                append_comma_delimited_list(&mut altstr, &names_list);

                names_list.push(op_name);
            }

            let mut names_ref: SmallVec<[&str; ROOM_SIZE]> =
                SmallVec::with_capacity(names_list.len());
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
            Ok(Envs::new1(env))
        }

        "who" => {
            let collapsed = collapse(&arg);
            let matches = match_string(&collapsed, ctxt.ustr_map);

            let env: Env = if matches.is_empty() {
                Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Info(&format!(
                        "No users matching the pattern \"{}\".",
                        &collapsed
                    )),
                )
            } else {
                let mut altstr = String::from("Matching names: ");
                append_comma_delimited_list(&mut altstr, &matches);
                let listref: Vec<&str> = matches.iter().map(|x| x.as_str()).collect();
                Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Misc {
                        what: "who",
                        data: &listref,
                        alt: &altstr,
                    },
                )
            };
            Ok(Envs::new1(env))
        }

        "rooms" => {
            let collapsed = collapse(&arg);
            let matches = match_string(&collapsed, ctxt.rstr_map);
            let env: Env = if matches.is_empty() {
                Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Info(&format!(
                        "No Rooms matching the pattern \"{}\".",
                        &collapsed
                    )),
                )
            } else {
                let mut altstr = String::from("Matching Rooms: ");
                append_comma_delimited_list(&mut altstr, &matches);
                let listref: Vec<&str> = matches.iter().map(|x| x.as_str()).collect();
                Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Misc {
                        what: "rooms",
                        data: &listref,
                        alt: &altstr,
                    },
                )
            };
            Ok(Envs::new1(env))
        }

        ukn => {
            let env = Env::new(
                End::Server,
                End::User(ctxt.uid),
                &Sndr::Err(&format!("Unknown \"Query\" type: \"{}\".", ukn)),
            );
            Ok(Envs::new1(env))
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
                Ok(Envs::new1(env))
            } else {
                let env = Env::new(
                    End::Server,
                    End::User(uid),
                    &Sndr::Info(&format!("{} is already open.", cur_r.get_name())),
                );
                Ok(Envs::new1(env))
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
                Ok(Envs::new1(env))
            } else {
                cur_r.closed = true;
                let env = Env::new(
                    End::Server,
                    End::Room(rid),
                    &Sndr::Info(&format!("{} has closed {}.", &op_name, cur_r.get_name())),
                );
                Ok(Envs::new1(env))
            }
        }

        RcvOp::Give(ref new_name) => {
            let collapsed = collapse(new_name);
            if collapsed.is_empty() {
                let env = Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Err("That cannot be anyone's user name."),
                );
                return Ok(Envs::new1(env));
            }

            let ouid = match ctxt.ustr_map.get(&collapsed) {
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
            Ok(Envs::new1(env))
        }

        RcvOp::Invite(ref uname) => {
            let collapsed = collapse(uname);
            if collapsed.is_empty() {
                let env = Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Info("That cannot be anyone's user name."),
                );
                return Ok(Envs::new1(env));
            }

            let ouid = match ctxt.ustr_map.get(&collapsed) {
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

            let cur_r = match ctxt.room_map.get_mut(&ctxt.rid) {
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

            let ou = match ctxt.user_map.get_mut(&ouid) {
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
            Ok(Envs::new1(inviter_env))
        }

        RcvOp::Kick(ref uname) => {
            let collapsed = collapse(uname);
            if collapsed.is_empty() {
                let env = Env::new(
                    End::Server,
                    End::User(ctxt.uid),
                    &Sndr::Info("That cannot be anyone's user name."),
                );
                return Ok(Envs::new1(env));
            }

            let ouid = match ctxt.ustr_map.get(&collapsed) {
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

            let ku = match ctxt.user_map.get_mut(&ouid) {
                None => {
                    return Err(format!(
                        "do_op(r {}, u {}, {:?}): no target User {}",
                        ctxt.rid, ctxt.uid, &op, ouid
                    ));
                }
                Some(u) => u,
            };

            let in_room: bool;
            let mut cur_room_name = String::default();

            {
                let cur_r = match ctxt.room_map.get_mut(&ctxt.rid) {
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

            let lobby = ctxt.room_map.get_mut(&0).unwrap();
            lobby.join(ouid);
            let data: [&str; 2] = [ku.get_name(), lobby.get_name()];
            let to_lobby = Env::new(
                End::Server,
                End::Room(ctxt.rid),
                &Sndr::Misc {
                    what: "join",
                    data: &data,
                    alt: &format!("{} joins {}.", ku.get_name(), lobby.get_name()),
                },
            );
            lobby.enqueue(to_lobby);

            let data: [&str; 2] = [ku.get_name(), &cur_room_name];
            let env = Env::new(
                End::Server,
                End::Room(ctxt.rid),
                &Sndr::Misc {
                    what: "kick_other",
                    data: &data,
                    alt: &format!("{} has been kicked from {}.", ku.get_name(), &cur_room_name),
                },
            );

            Ok(Envs::new1(env))
        }
    }
}

pub fn process_room(
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
        rid,
        uid: 0,
        user_map,
        ustr_map,
        room_map,
        rstr_map,
    };

    let mut envs: Envs = Envs::new0();
    let mut logouts: SmallVec<[(u64, &str); LOGOUTS_SIZE]> = SmallVec::new();

    for uid in &uid_list {
        // give it a better name
        let rec: Rcvr;
        {
            let user = match ctxt.user_map.get_mut(uid) {
                None => {
                    debug!("process_room({}): user {} doesn't exist", &rid, uid);
                    continue;
                }
                Some(x) => x,
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
                        Some(x) if x > cfg.blackout_time_to_kick => {
                            logouts.push((
                                *uid,
                                "Too long since server received data from the client.",
                            ));
                        }
                        Some(x) if x > cfg.blackout_time_to_ping => {
                            user.deliver_msg(&Sndr::Ping);
                        }
                        _ => {}
                    }
                    continue;
                }
                Some(msg) => {
                    if !over_quota {
                        rec = msg;
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
                let e = user.get_errors();
                warn!("User {} being logged out for error(s): {}", uid, &e);
                logouts.push((*uid, "Communication error."));
            }
        }

        ctxt.uid = *uid;

        let pres = match rec {
            Rcvr::Text { lines: l, .. } => do_text(&mut ctxt, l),
            Rcvr::Priv { who, text } => do_priv(&mut ctxt, who, text),
            Rcvr::Name(new_candidate) => do_name(&mut ctxt, cfg, new_candidate),
            Rcvr::Join(room_name) => do_join(&mut ctxt, cfg, room_name),
            Rcvr::Block(user_name) => do_block(&mut ctxt, user_name),
            Rcvr::Unblock(user_name) => do_unblock(&mut ctxt, user_name),
            Rcvr::Logout(salutation) => do_logout(&mut ctxt, salutation),
            Rcvr::Query { what, arg } => do_query(&mut ctxt, what, arg),
            Rcvr::Op(op) => do_op(&mut ctxt, op),
            _ => Ok(Envs::new0()),
        };

        match pres {
            Err(e) => {
                trace!("{}", &e);
            }
            Ok(mut v) => {
                let evz = envs.as_mut();
                for env in v.as_mut().drain(..) {
                    evz.push(env);
                }
            }
        }
    }

    for (uid, errmsg) in logouts.iter() {
        if let Some(mut user) = ctxt.user_map.remove(uid) {
            ctxt.ustr_map.remove(user.get_idstr());
            let msg = Sndr::Logout(errmsg);
            user.deliver_msg(&msg);
            let dat: [&str; 2] = [user.get_name(), "[ disconnected by server ]"];
            let env = Env::new(
                End::Server,
                End::Room(ctxt.rid),
                &Sndr::Misc {
                    what: "leave",
                    data: &dat,
                    alt: &format!("{} has been disconnected from the server.", user.get_name()),
                },
            );
            envs.as_mut().push(env);
        } else {
            warn!(
                "process_room({} ...): logouts.drain(): no User {}",
                ctxt.rid, uid
            );
        }
    }

    // Change room operator if current op is no longer in room.

    if rid != 0 {
        let room = ctxt.room_map.get_mut(&rid).unwrap();
        let op_id = room.get_op();
        let op_still_here = room.get_users().contains(&op_id);
        if !op_still_here {
            if let Some(pnid) = room.get_users().first() {
                if let Some(u) = ctxt.user_map.get(pnid) {
                    let nid = *pnid;
                    room.set_op(nid);
                    let env = Env::new(
                        End::Server,
                        End::Room(rid),
                        &Sndr::Info(&format!("{} is now the Room operator.", u.get_name())),
                    );
                    envs.as_mut().push(env);
                }
            }
        }
    }

    {
        let room = ctxt.room_map.get_mut(&rid).unwrap();
        for (uid, _) in logouts.drain(..) {
            room.leave(uid);
        }
        room.deliver_inbox(ctxt.user_map);
        for env in envs.as_ref() {
            room.deliver(env, ctxt.user_map);
        }
        uid_list.clear();
        uid_list.extend_from_slice(room.get_users());
        for uid in uid_list.iter_mut() {
            if let Some(user) = user_map.get_mut(uid) {
                user.send();
            }
        }
    }

    Ok(())
}
