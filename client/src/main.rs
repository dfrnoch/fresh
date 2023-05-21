mod connection;
mod input;
mod line;
mod message;
mod screen;
mod util;

use crate::connection::connect;
use crate::input::{process_user_typing, write_mode_line, Mode};
use crate::line::Line;
use crate::message::process_msg;
use crate::screen::Screen;
use crate::util::styles::HIGHLIGHT;

use clap::Parser;
use common::config::ClientConfig;
use common::proto::Sndr;
use common::socket::Socket;
use connection::State;
use lazy_static::lazy_static;
use log::{debug, error};
use std::io::stdout;
use std::time::Instant;

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
}

fn configure() -> ClientConfig {
    let opts = ClapOpts::parse();

    let mut cfg = match ClientConfig::configure(opts.config) {
        Ok(x) => x,
        Err(e) => {
            println!("Configuration error: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(name) = opts.name {
        cfg.name = name;
    }
    if let Some(address) = opts.address {
        cfg.address = address;
    }

    cfg
}

fn main() {
    let cfg: ClientConfig = configure();

    #[cfg(debug_assertions)]
    simplelog::WriteLogger::init(
        simplelog::LevelFilter::Trace,
        simplelog::Config::default(),
        std::fs::File::create("fresh.log").unwrap(),
    )
    .unwrap();

    debug!("{:?}", &cfg);
    println!("Attempting to connect to {}...", &cfg.address);
    let mut socket: Socket = match connect(&cfg) {
        Err(e) => {
            println!("{}", e);
            std::process::exit(2);
        }
        Ok(x) => x,
    };
    socket.set_read_buffer_size(cfg.read_size);
    println!("...success. Negotiating initial protocol...");

    {
        let bytes = Sndr::Query {
            what: "addr",
            arg: "",
        }
        .bytes();
        socket.enqueue(&bytes);
    }
    println!("...success. Initializing terminal.");

    let mut state = State {
        username: cfg.name.clone(),
        room_name: String::from("Lobby"),
        mode: Mode::Insert,
        local_address: String::default(),
        buffered_messages: Vec::new(),
        server_address: socket.get_addr().unwrap(),
        socket,
        cmd: cfg.cmd_char,
        running: true,
    };

    {
        let mut terminal_handle = stdout();
        let mut screen: Screen = match Screen::new(&mut terminal_handle, cfg.roster_width) {
            Ok(x) => x,
            Err(e) => {
                println!("Error setting up terminal: {}", e);
                std::process::exit(1);
            }
        };

        let mut server_address_line = Line::default();
        server_address_line.pushf(&state.server_address, &HIGHLIGHT);
        screen.set_stat_ul(server_address_line);

        let mut current_room_line = Line::default();
        current_room_line.pushf(&state.room_name, &HIGHLIGHT);
        screen.set_stat_ur(current_room_line);
        write_mode_line(&mut screen, &state);

        'main_loop: loop {
            let loop_timer = Instant::now();

            'input_loop: loop {
                match process_user_typing(&mut screen, &mut state) {
                    Err(e) => {
                        state
                            .buffered_messages
                            .push(format!("Error getting event from keyboard: {}", e));
                        break 'main_loop;
                    }
                    Ok(true) => {
                        if let Err(e) = screen.refresh(&mut terminal_handle) {
                            state
                                .buffered_messages
                                .push(format!("Error refreshing screen: {}", e));
                            break 'main_loop;
                        } else if !state.running {
                            break 'main_loop;
                        }
                    }
                    Ok(false) => {
                        break 'input_loop;
                    }
                }
            }

            let outgoing_bytes = state.socket.send_buff_size();
            match state.socket.send_data() {
                Err(e) => {
                    state.buffered_messages.push(format!("{}", e));
                    break 'main_loop;
                }
                Ok(n) => {
                    let sent = outgoing_bytes - n;
                    if sent > 0 {
                        debug!("Socket::send_data() wrote {} bytes.", sent);
                    }
                }
            }

            // Try to read from the byte stream incoming from the server.
            let read_result = state.socket.read_data();
            match read_result {
                Err(e) => {
                    state.buffered_messages.push(format!("{}", e));
                    break 'main_loop;
                }
                Ok(0) => {}
                Ok(n) => {
                    debug!("Socket::read_data() huffed {} bytes.", n);
                    'msg_loop: loop {
                        let message_result = state.socket.try_get();
                        match message_result {
                            Err(e) => {
                                state.buffered_messages.push(format!("{}", e));
                                break 'main_loop;
                            }
                            Ok(None) => {
                                break 'msg_loop;
                            }
                            Ok(Some(message)) => {
                                match process_msg(message, &mut screen, &mut state) {
                                    Ok(()) => {
                                        if !state.running {
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

            if screen.get_scrollback_length() > cfg.max_scrollback {
                screen.prune_scrollback(cfg.min_scrollback);
            }

            if let Err(e) = screen.refresh(&mut terminal_handle) {
                state
                    .buffered_messages
                    .push(format!("Error refreshing screen: {}", e));
                break 'main_loop;
            }

            let loop_time = Instant::now().duration_since(loop_timer);
            if loop_time < cfg.tick {
                std::thread::sleep(cfg.tick - loop_time);
            }
        }
    }

    for message in &state.buffered_messages {
        println!("{}", &message);
    }
}
