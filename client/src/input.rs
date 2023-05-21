use crate::{
    connection::State,
    line::Line,
    message::respond_to_user_input,
    screen::Screen,
    util::styles::{DIM, HIGHLIGHT},
};
use common::proto::Sndr;
use crossterm::{event, event::Event, event::KeyCode};
use log::trace;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Insert,
    Command,
    Delete,
}

fn command_key(event: event::KeyEvent, screen: &mut Screen, state: &mut State) {
    match state.mode {
        Mode::Command => match event.code {
            KeyCode::Char(' ') | KeyCode::Enter => state.mode = Mode::Insert,

            KeyCode::Up | KeyCode::Char('k') => screen.scroll_lines(1),
            KeyCode::Down | KeyCode::Char('j') => screen.scroll_lines(-1),

            KeyCode::Left | KeyCode::Char('h') => screen.input_skip_chars(-1),
            KeyCode::Right | KeyCode::Char('l') => screen.input_skip_chars(1),

            KeyCode::Char('w') => screen.input_skip_words(1),
            KeyCode::Char('b') => screen.input_skip_words(-1),

            KeyCode::Char('0') => {
                let delta = screen.get_input_length() as i16;
                screen.input_skip_chars(-delta);
            }
            KeyCode::Char('$') => {
                let delta = screen.get_input_length() as i16;
                screen.input_skip_chars(delta);
            }

            KeyCode::Char('a') => {
                screen.input_skip_chars(1);
                state.mode = Mode::Insert;
            }
            KeyCode::Char('i') => state.mode = Mode::Insert,

            KeyCode::PageUp => {
                let jump = (screen.get_main_height() as i16) - 1;
                screen.scroll_lines(jump);
            }
            KeyCode::PageDown => {
                let jump = 1 - (screen.get_main_height() as i16);
                screen.scroll_lines(jump);
            }
            KeyCode::Char('q') => state.enqueue(&Sndr::Logout("[ client quit ]")),
            KeyCode::Char('d') => state.mode = Mode::Delete,
            _ => {}
        },
        Mode::Delete => match event.code {
            KeyCode::Char('h') => {
                screen.input_skip_chars(-1);
                // screen.delete_char();
                state.mode = Mode::Command;
            }
            KeyCode::Char('l') => {
                screen.input_skip_chars(1);
                // scrn.delete_char();
                state.mode = Mode::Command;
            }
            KeyCode::Char('d') => {
                screen.pop_input();
                state.mode = Mode::Command;
            }
            KeyCode::Char('w') => {
                screen.input_delete_words(1);
                state.mode = Mode::Command;
            }
            KeyCode::Char('b') => {
                screen.input_delete_words(-1);
                state.mode = Mode::Command;
            }
            _ => {
                state.mode = Mode::Command;
            }
        },
        _ => {}
    }
}

fn input_key(event: event::KeyEvent, screen: &mut Screen, state: &mut State) {
    match event.code {
        KeyCode::Enter => respond_to_user_input(screen.pop_input(), screen, state),
        KeyCode::Backspace => {
            if screen.get_input_length() == 0 {
                state.mode = Mode::Command;
            } else {
                screen.input_backspace();
            }
        }
        KeyCode::Left => screen.input_skip_chars(-1),
        KeyCode::Right => screen.input_skip_chars(1),
        KeyCode::Esc => {
            state.mode = Mode::Command;
        }
        KeyCode::Char(c) => {
            screen.input_char(c);
        }
        _ => {}
    }
}

pub fn process_user_typing(screen: &mut Screen, state: &mut State) -> crossterm::Result<bool> {
    let mut should_refresh = false;

    while event::poll(Duration::default())? {
        let prev_mode = state.mode;

        if let Ok(Event::Key(event)) = event::read() {
            trace!("event: {:?}", event);
            match state.mode {
                Mode::Command | Mode::Delete => command_key(event, screen, state),
                Mode::Insert => input_key(event, screen, state),
            }
        } else if let Ok(Event::Resize(w, h)) = event::read() {
            screen.resize(w, h);
        }

        if prev_mode != state.mode {
            write_mode_line(screen, state);
        }
        should_refresh = true;
    }

    Ok(should_refresh)
}

/// Write the mode line to the screen.
pub fn write_mode_line(screen: &mut Screen, state: &State) {
    let mut mode_line = Line::default();
    let mch: &str = match state.mode {
        Mode::Insert => "Ins",
        Mode::Command => "Com",
        Mode::Delete => "Del",
    };
    mode_line.pushf(mch, &HIGHLIGHT);
    mode_line.pushf(" │ ", &DIM);
    mode_line.pushf(&(state.username), &HIGHLIGHT);
    mode_line.push(" @ ");
    mode_line.pushf(&(state.local_address), &HIGHLIGHT);
    screen.set_stat_ll(mode_line);
}
