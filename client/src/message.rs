use crate::{
  connection::Globals,
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

pub fn process_msg(msg: Rcvr, screen: &mut Screen, global: &mut Globals) -> Result<(), String> {
  debug!("process_msg(...): rec'd: {:?}", &msg);
  match msg {
    Rcvr::Ping => {
      global.socket.enqueue(&PING);
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
      global.messages.push(s);
      global.run = false;
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
        if name.as_str() == global.uname.as_str() {
          sl.pushf("You", &BOLD);
          sl.push(" joined ");

          // Update the room name in the status bar.
          global.rname = room.to_string();
          let mut room_line = Line::default();
          room_line.pushf(&global.rname, &HIGHLIGHT);
          screen.set_stat_ur(room_line);
        } else {
          sl.pushf(name, &HIGHLIGHT);
          sl.push(" joins ");
        }
        sl.pushf(room, &HIGHLIGHT);
        sl.push(".");
        global.enqueue_bytes(&ROSTER_REQUEST);
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
        sl.push(" leaves: ");
        sl.push(message);
        global.enqueue_bytes(&ROSTER_REQUEST);
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
        if old.as_str() == global.uname.as_str() {
          sl.pushf("You", &BOLD);
          sl.push(" are now known as ");
          global.uname.clone_from(new);
          write_mode_line(screen, global);
        } else {
          sl.pushf(old, &HIGHLIGHT);
          sl.push(" is now known as ");
        }
        sl.pushf(new, &HIGHLIGHT);
        sl.push(".");
        screen.push_line(sl);
        global.enqueue_bytes(&ROSTER_REQUEST);
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
        if name == &global.uname {
          sl.pushf("You", &BOLD);
          sl.push(" are now the operator of ");
        } else {
          sl.pushf(name, &HIGHLIGHT);
          sl.push(" is now the operator of ");
        }
        sl.pushf(room, &BOLD);
        sl.push(".");
        screen.push_line(sl);
        global.enqueue_bytes(&ROSTER_REQUEST);
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
        global.enqueue_bytes(&ROSTER_REQUEST);
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
          global.local_addr.clone_from(addr);
          write_mode_line(screen, global);
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
pub fn respond_to_user_input(input: Vec<char>, screen: &mut Screen, global: &mut Globals) {
  if let Some(c) = input.first() {
    if *c == global.cmd {
      if input.len() == 1 {
        return;
      }

      let cmd_line: String = input[1..]
        .iter()
        .map(|c| if *c == RETURN { SPACE } else { *c })
        .collect();

      let cmd_toks = tokenize_whitespace(&cmd_line);
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
            global.enqueue(&Sndr::Logout(&arg));
          }
          Err(_) => {
            return;
          }
        },

        "priv" => match split_command_tokens(&cmd_toks, 2) {
          Ok((cmds, arg)) => {
            global.enqueue(&Sndr::Priv {
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
            global.enqueue(&Sndr::Name(&arg));
          }
          Err(_) => {
            return;
          }
        },

        "join" => match split_command_tokens(&cmd_toks, 1) {
          Ok((_, arg)) => {
            global.enqueue(&Sndr::Join(&arg));
          }
          Err(_) => {
            return;
          }
        },

        "who" | "rooms" => match split_command_tokens(&cmd_toks, 1) {
          Ok((_, arg)) => {
            global.enqueue(&Sndr::Query {
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
            global.enqueue(&Sndr::Block(&arg));
          }
          Err(_) => {
            return;
          }
        },

        "unblock" => match split_command_tokens(&cmd_toks, 1) {
          Ok((_, arg)) => {
            global.enqueue(&Sndr::Unblock(&arg));
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
              global.enqueue(&m);
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

  let mut lines: Vec<String> = Vec::new();
  let mut cur_line = String::default();
  for c in input.into_iter() {
    if c == '\n' {
      lines.push(cur_line);
      cur_line = String::default();
    } else {
      cur_line.push(c);
    }
  }
  lines.push(cur_line);
  let lineref: Vec<&str> = lines.iter().map(|x| x.as_str()).collect();
  global.enqueue(&Sndr::Text {
    who: "",
    lines: &lineref,
  });
}

/// Split a vector of &str into a vector of commands and a single argument.
fn split_command_tokens<'a>(toks: &'a [&str], n_cmds: usize) -> Result<(Vec<&'a str>, String), ()> {
  if n_cmds == 0 || toks.len() < (2 * n_cmds) - 1 {
    return Err(());
  }

  let cmds: Vec<&'a str> = toks.iter().take(n_cmds * 2).step_by(2).copied().collect();
  let arg: String = toks
    .iter()
    .skip(n_cmds * 2)
    .cloned()
    .collect::<Vec<&str>>()
    .join("");

  Ok((cmds, arg))
}

/// Split a string into a vector of &str, splitting on whitespace.
fn tokenize_whitespace(s: &str) -> Vec<&str> {
  let mut vec: Vec<&str> = Vec::new();

  let mut change: usize = 0;
  let mut s_iter = s.chars();
  let mut in_ws = match s_iter.next() {
    None => {
      return vec;
    }
    Some(c) => c.is_whitespace(),
  };

  let s_iter = s.char_indices();
  for (i, c) in s_iter {
    if in_ws {
      if !c.is_whitespace() {
        vec.push(&s[change..i]);
        change = i;
        in_ws = false;
      }
    } else if c.is_whitespace() {
      vec.push(&s[change..i]);
      change = i;
      in_ws = true;
    }
  }
  vec.push(&s[change..(s.len())]);

  vec
}
