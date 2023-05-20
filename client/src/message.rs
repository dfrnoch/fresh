use crate::{
    connection::ClientState,
    input::write_mode_line,
    line::Line,
    screen::Screen,
    util::styles::{BOLD, DIM, DIM_BOLD, HIGHLIGHT},
    PING, ROSTER_REQUEST,
};
use common::proto::{Rcvr, SndOp, Sndr};
use log::debug;

const OP_ERROR: &str = "# The recognized OP subcommands are OPEN, CLOSE, KICK, INVITE, and GIVE.";
const RETURN: char = '\n';
const SPACE: char = ' ';

pub fn process_msg(
    msg: Rcvr,
    screen: &mut Screen,
    client_state: &mut ClientState,
) -> Result<(), String> {
    debug!("process_msg(...): rec'd: {:?}", &msg);
    match msg {
        Rcvr::Ping => {
            client_state.socket.enqueue(&PING);
        }

        Rcvr::Text { who, lines } => {
            for lin in &lines {
                let mut sl = Line::default();
                sl.pushf(&who, &HIGHLIGHT);
                sl.push(": ");
                sl.push(lin);
                screen.push_line(sl);
            }
        }

        Rcvr::Priv { who, text } => {
            let mut sl = Line::default();
            sl.push("$ ");
            sl.pushf(&who, &DIM);
            sl.push(": ");
            sl.push(&text);
            screen.push_line(sl);
        }

        Rcvr::Logout(s) => {
            client_state.buffered_messages.push(s);
            client_state.running = false;
        }

        Rcvr::Info(s) => {
            let mut sl = Line::default();
            sl.push("* ");
            sl.push(&s);
            screen.push_line(sl);
        }

        Rcvr::Err(s) => {
            let mut sl = Line::default();
            sl.pushf("# ", &DIM);
            sl.pushf(&s, &DIM);
            screen.push_line(sl);
        }

        Rcvr::Misc {
            ref what,
            ref alt,
            ref data,
        } => match what.as_str() {
            "join" => {
                let (name, room) = match &data[..] {
                    [x, y] => (x, y),
                    _ => {
                        return Err(format!("Incomplete data: {:?}", &msg));
                    }
                };
                let mut sl = Line::default();
                sl.push("* ");
                if name.as_str() == client_state.username.as_str() {
                    sl.pushf("You", &BOLD);
                    sl.push(" joined ");

                    // Update the room name in the status bar.
                    client_state.room_name = room.to_string();
                    let mut room_line = Line::default();
                    room_line.pushf(&client_state.room_name, &HIGHLIGHT);
                    screen.set_stat_ur(room_line);
                } else {
                    sl.pushf(name, &HIGHLIGHT);
                    sl.push(" joined ");
                }
                sl.pushf(room, &HIGHLIGHT);
                sl.push(".");
                client_state.enqueue_bytes(&ROSTER_REQUEST);
                screen.push_line(sl);
            }

            "leave" => {
                let (name, message) = match &data[..] {
                    [x, y] => (x, y),
                    _ => {
                        return Err(format!("Incomplete data: {:?}", &msg));
                    }
                };
                let mut sl = Line::default();
                sl.push("* ");
                sl.pushf(name, &HIGHLIGHT);
                sl.push(" left: ");
                sl.push(message);
                client_state.enqueue_bytes(&ROSTER_REQUEST);
                screen.push_line(sl);
            }

            "priv_echo" => {
                let (name, text) = match &data[..] {
                    [x, y] => (x, y),
                    _ => {
                        return Err(format!("Incomplete data: {:?}", &msg));
                    }
                };
                let mut sl = Line::default();
                sl.push("$ ");
                sl.pushf("You", &DIM_BOLD);
                sl.pushf(" @ ", &DIM);
                sl.pushf(name, &HIGHLIGHT);
                sl.push(": ");
                sl.push(text);
                screen.push_line(sl);
            }

            "name" => {
                let (old, new) = match &data[..] {
                    [x, y] => (x, y),
                    _ => {
                        return Err(format!("Incomplete data: {:?}", &msg));
                    }
                };

                let mut sl = Line::default();
                sl.push("* ");
                if old.as_str() == client_state.username.as_str() {
                    sl.pushf("You", &BOLD);
                    sl.push(" are now known as ");
                    client_state.username.clone_from(new);
                    write_mode_line(screen, client_state);
                } else {
                    sl.pushf(old, &HIGHLIGHT);
                    sl.push(" is now known as ");
                }
                sl.pushf(new, &HIGHLIGHT);
                sl.push(".");
                screen.push_line(sl);
                client_state.enqueue_bytes(&ROSTER_REQUEST);
            }

            "new_op" => {
                let (name, room) = match &data[..] {
                    [x, y] => (x, y),
                    _ => {
                        return Err(format!("Incomplete data: {:?}", &msg));
                    }
                };

                let mut sl = Line::default();
                sl.push("* ");
                if name == &client_state.username {
                    sl.pushf("You", &BOLD);
                    sl.push(" are now the operator of ");
                } else {
                    sl.pushf(name, &HIGHLIGHT);
                    sl.push(" is now the operator of ");
                }
                sl.pushf(room, &BOLD);
                sl.push(".");
                screen.push_line(sl);
                client_state.enqueue_bytes(&ROSTER_REQUEST);
            }

            "roster" => {
                if data.is_empty() {
                    return Err(format!("Incomplete data: {:?}", &msg));
                }

                screen.set_roster(data);
            }

            "kick_other" => {
                let (name, room) = match &data[..] {
                    [x, y] => (x, y),
                    _ => {
                        return Err(format!("Incomplete data: {:?}", &msg));
                    }
                };
                let mut sl = Line::default();
                sl.push("* ");
                sl.pushf(name, &HIGHLIGHT);
                sl.push(" has been kicked from ");
                sl.pushf(room, &HIGHLIGHT);
                sl.push(".");
                screen.push_line(sl);
                client_state.enqueue_bytes(&ROSTER_REQUEST);
            }

            "kick_you" => {
                let room = match &data[..] {
                    [x] => x,
                    _ => {
                        return Err(format!("Incomplete data: {:?}", &msg));
                    }
                };
                let mut sl = Line::default();
                sl.push("* ");
                sl.pushf("You", &BOLD);
                sl.push(" have been kicked from ");
                sl.pushf(room, &HIGHLIGHT);
                sl.push(".");
                screen.push_line(sl);
            }

            "addr" => match data.get(0) {
                None => {
                    return Err(format!("Incomplete data: {:?}", &msg));
                }
                Some(addr) => {
                    client_state.local_address.clone_from(addr);
                    write_mode_line(screen, client_state);
                }
            },

            _ => {
                let mut sl = Line::default();
                sl.push("* ");
                sl.push(alt);
                screen.push_line(sl)
            }
        },

        msg => {
            let msgs = format!("{:?}", msg);
            let s: String = msgs
                .chars()
                .map(|c| match c {
                    '\n' => SPACE,
                    x => x,
                })
                .collect();
            let mut sl = Line::default();
            sl.push("# Unsupported Rcvr: ");
            sl.push(&s);
            screen.push_line(sl);
        }
    }
    Ok(())
}

/// In input mode, when the user hits return, this processes processes the
/// content of the input line and decides what to do.
pub fn respond_to_user_input(
    input: Vec<char>,
    screen: &mut Screen,
    client_state: &mut ClientState,
) {
    if let Some(char) = input.first() {
        if *char == client_state.cmd {
            if input.len() == 1 {
                return;
            }

            let cmd_line: String = input[1..]
                .iter()
                .map(|c| if *c == RETURN { SPACE } else { *c })
                .collect();

            debug!("cmd_line: {:?}", cmd_line);

            let cmd_toks = cmd_line.split_whitespace().collect::<Vec<&str>>();
            let cmd = cmd_toks[0].to_lowercase();

            match cmd.as_str() {
                "help" => {
                    let mut sl = Line::default();
                    sl.pushf("# ", &DIM_BOLD);
                    sl.pushf("Commands:", &DIM);
                    screen.push_line(sl);
                    sl = Line::default();
                    sl.pushf("# ", &DIM_BOLD);
                    sl.pushf("  /quit", &DIM);
                    sl.push(" - quit the program");
                    screen.push_line(sl);
                    sl = Line::default();
                    sl.pushf("# ", &DIM_BOLD);
                    sl.pushf("  /name <name>", &DIM);
                    sl.push(" - change your name");
                    screen.push_line(sl);
                    sl = Line::default();
                    sl.pushf("# ", &DIM_BOLD);
                    sl.pushf("  /priv <name> <text>", &DIM);
                    sl.push(" - send a private message");
                    screen.push_line(sl);
                    sl = Line::default();
                    sl.pushf("# ", &DIM_BOLD);
                    sl.pushf("  /join <room>", &DIM);
                    sl.push(" - join a room");
                    screen.push_line(sl);
                }
                "quit" => match split_command_tokens(&cmd_toks, 1) {
                    Ok((_, arg)) => {
                        client_state.enqueue(&Sndr::Logout(&arg));
                    }
                    Err(_) => {
                        return;
                    }
                },

                "priv" => match split_command_tokens(&cmd_toks, 2) {
                    Ok((cmds, arg)) => {
                        client_state.enqueue(&Sndr::Priv {
                            who: cmds[1],
                            text: &arg,
                        });
                    }
                    Err(_) => {
                        let mut sl = Line::default();
                        sl.pushf(
                            "# You must specify a recipient for a private message.",
                            &DIM,
                        );
                        screen.push_line(sl);
                    }
                },

                "name" => match split_command_tokens(&cmd_toks, 1) {
                    Ok((_, arg)) => {
                        client_state.enqueue(&Sndr::Name(&arg));
                    }
                    Err(_) => {
                        return;
                    }
                },

                "join" => match split_command_tokens(&cmd_toks, 1) {
                    Ok((_, arg)) => {
                        client_state.enqueue(&Sndr::Join(&arg));
                    }
                    Err(_) => {
                        return;
                    }
                },

                "who" | "rooms" => match split_command_tokens(&cmd_toks, 1) {
                    Ok((_, arg)) => {
                        client_state.enqueue(&Sndr::Query {
                            what: &cmd,
                            arg: &arg,
                        });
                    }
                    Err(_) => {
                        return;
                    }
                },

                "block" => match split_command_tokens(&cmd_toks, 1) {
                    Ok((_, arg)) => {
                        client_state.enqueue(&Sndr::Block(&arg));
                    }
                    Err(_) => {
                        return;
                    }
                },

                "unblock" => match split_command_tokens(&cmd_toks, 1) {
                    Ok((_, arg)) => {
                        client_state.enqueue(&Sndr::Unblock(&arg));
                    }
                    Err(_) => {
                        return;
                    }
                },

                "op" => match split_command_tokens(&cmd_toks, 2) {
                    Err(_) => {
                        let mut sl = Line::default();
                        sl.pushf(OP_ERROR, &DIM);
                        screen.push_line(sl);
                    }
                    Ok((cmds, arg)) => {
                        let msg: Option<Sndr> = match cmds[1].to_lowercase().as_str() {
                            "open" => Some(Sndr::Op(SndOp::Open)),
                            "close" => Some(Sndr::Op(SndOp::Close)),
                            "ban" | "kick" => Some(Sndr::Op(SndOp::Kick(&arg))),
                            "invite" => Some(Sndr::Op(SndOp::Invite(&arg))),
                            "give" => Some(Sndr::Op(SndOp::Give(&arg))),
                            _ => {
                                let mut sl = Line::default();
                                sl.pushf(OP_ERROR, &DIM);
                                screen.push_line(sl);
                                None
                            }
                        };
                        if let Some(m) = msg {
                            client_state.enqueue(&m);
                        }
                    }
                },

                x => {
                    let mut sl = Line::default();
                    sl.pushf("# Unknown command ", &DIM);
                    sl.pushf(x, &DIM_BOLD);
                    screen.push_line(sl);
                }
            }
            return;
        }
    }

    let input_str: String = input.into_iter().collect();
    let lines: Vec<String> = input_str.lines().map(|line| line.to_string()).collect();
    let lineref: Vec<&str> = lines.iter().map(|x| x.as_str()).collect();

    client_state.enqueue(&Sndr::Text {
        who: "",
        lines: &lineref,
    });
}

/// Split a vector of &str into a vector of commands and a single argument.
fn split_command_tokens<'a>(toks: &'a [&str], n_cmds: usize) -> Result<(Vec<&'a str>, String), ()> {
    if n_cmds == 0 || toks.len() < n_cmds {
        return Err(());
    }

    let (cmds, args) = toks.split_at(n_cmds);
    let cmds: Vec<&'a str> = cmds.to_vec();
    let arg: String = args.to_vec().concat();

    Ok((cmds, arg))
}
