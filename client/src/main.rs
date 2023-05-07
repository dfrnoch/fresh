mod connection;
mod input;
mod message;
mod screen;
mod util;

use clap::Parser;
use connection::{Globals, Mode};
use lazy_static::lazy_static;
use log::{debug, error};
use std::io::stdout;
use std::time::Instant;
use util::styles::*;

use crate::connection::connect;
use crate::input::{process_user_typing, write_mode_line};
use crate::message::process_msg;
use crate::screen::Screen;
use common::config::ClientConfig;
use common::line::Line;
use common::proto::Sndr;
use common::socket::Socket;

lazy_static! {
  static ref PING: Vec<u8> = Sndr::Ping.bytes();
  static ref ROSTER_REQUEST: Vec<u8> = Sndr::Query {
    what: "roster",
    arg: "",
  }
  .bytes();
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct ClapOpts {
  #[arg(short = 'c', long = "config")]
  config: Option<String>,

  #[arg(short = 'n', long = "name")]
  name: Option<String>,

  #[arg(short = 'a', long = "address")]
  address: Option<String>,

  #[arg(
    short = 'g',
    long = "generate-default",
    default_value = "false",
    default_missing_value = "true"
  )]
  write: bool,
}

fn configure() -> ClientConfig {
  let opts = ClapOpts::parse();

  if opts.write {
    match ClientConfig::generate() {
      Ok(dir) => {
        println!("Default configuration file written to {}", &dir);
        std::process::exit(0);
      }
      Err(e) => {
        println!("{}", e);
        std::process::exit(2);
      }
    }
  }

  let mut cfg = match ClientConfig::configure(opts.config) {
    Ok(x) => x,
    Err(e) => {
      println!("Configuration error: {}", e);
      std::process::exit(1);
    }
  };

  if let Some(n) = opts.name {
    cfg.name = n;
  }
  if let Some(a) = opts.address {
    cfg.address = a;
  }

  cfg
}

fn main() {
  let cfg: ClientConfig = configure();
  #[cfg(release)]
  let the_log_level = simplelog::LevelFilter::None;

  simplelog::WriteLogger::init(
    simplelog::LevelFilter::Trace,
    simplelog::Config::default(),
    std::fs::File::create("fresh.log").unwrap(),
  )
  .unwrap();

  debug!("{:?}", &cfg);
  println!("Attempting to connect to {}...", &cfg.address);
  let mut sck: Socket = match connect(&cfg) {
    Err(e) => {
      println!("{}", e);
      std::process::exit(2);
    }
    Ok(x) => x,
  };
  sck.set_read_buffer_size(cfg.read_size);
  println!("...success. Negotiating initial protocol...");

  {
    let b = Sndr::Query {
      what: "addr",
      arg: "",
    }
    .bytes();
    sck.enqueue(&b);
  }
  println!("...success. Initializing terminal.");

  let mut gv: Globals = Globals {
    uname: cfg.name.clone(),
    rname: String::from("Lobby"),
    mode: Mode::Input,
    local_addr: String::default(),
    messages: Vec::new(),
    server_addr: sck.get_addr().unwrap(),
    socket: sck,
    cmd: cfg.cmd_char,
    run: true,
  };

  {
    let mut term = stdout();
    let mut scrn: Screen = match Screen::new(&mut term, cfg.roster_width) {
      Ok(x) => x,
      Err(e) => {
        println!("Error setting up terminal: {}", e);
        std::process::exit(1);
      }
    };

    let mut addr_line = Line::default();
    addr_line.pushf(&gv.server_addr, &HIGHLIGHT);
    scrn.set_stat_ul(addr_line);

    let mut room_line = Line::default();
    room_line.pushf(&gv.rname, &HIGHLIGHT);
    scrn.set_stat_ur(room_line);
    write_mode_line(&mut scrn, &gv);

    'main_loop: loop {
      let loop_start = Instant::now();

      'input_loop: loop {
        match process_user_typing(&mut scrn, &mut gv) {
          Err(e) => {
            gv.messages
              .push(format!("Error getting event from keyboard: {}", e));
            break 'main_loop;
          }
          Ok(true) => {
            if let Err(e) = scrn.refresh(&mut term) {
              gv.messages.push(format!("Error refreshing screen: {}", e));
              break 'main_loop;
            } else if !gv.run {
              break 'main_loop;
            }
          }
          Ok(false) => {
            break 'input_loop;
          }
        }
      }

      let outgoing_bytes = gv.socket.send_buff_size();
      match gv.socket.blow() {
        Err(e) => {
          gv.messages.push(format!("{}", e));
          break 'main_loop;
        }
        Ok(n) => {
          let sent = outgoing_bytes - n;
          if sent > 0 {
            debug!("Sock::blow() wrote {} bytes.", sent);
          }
        }
      }

      /* Try to suck from the byte stream incoming from the server.

      If there's anything there, attempt to decode `Msg`s from the
      `Sock` and process them until there's nothing left. */
      let suck_res = gv.socket.suck();
      match suck_res {
        Err(e) => {
          gv.messages.push(format!("{}", e));
          break 'main_loop;
        }
        Ok(0) => {}
        Ok(n) => {
          debug!("Sock::suck() huffed {} bytes.", n);
          'msg_loop: loop {
            let get_res = gv.socket.try_get();
            match get_res {
              Err(e) => {
                gv.messages.push(format!("{}", e));
                break 'main_loop;
              }
              Ok(None) => {
                break 'msg_loop;
              }
              Ok(Some(msg)) => {
                match process_msg(msg, &mut scrn, &mut gv) {
                  Ok(()) => {
                    if !gv.run {
                      break 'main_loop;
                    }
                  }
                  Err(e) => {
                    error!("process_msg(...) returned error: {}", e);
                  }
                };
              }
            }
          }
        }
      }

      if scrn.get_scrollback_length() > cfg.max_scrollback {
        scrn.prune_scrollback(cfg.min_scrollback);
      }

      if let Err(e) = scrn.refresh(&mut term) {
        gv.messages.push(format!("Error refreshing screen: {}", e));
        break 'main_loop;
      }

      let loop_time = Instant::now().duration_since(loop_start);
      if loop_time < cfg.tick {
        std::thread::sleep(cfg.tick - loop_time);
      }
    }
  }

  for m in &gv.messages {
    println!("{}", &m);
  }
}
