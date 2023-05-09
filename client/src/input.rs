use crate::{
  connection::Globals,
  message::respond_to_user_input,
  screen::Screen,
  util::styles::{DIM, HIGHLIGHT},
};
use common::{line::Line, proto::Sndr};
use crossterm::{event, event::Event, event::KeyCode};
use log::trace;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
  Insert,
  Command,
  Delete,
}

fn command_key(event: event::KeyEvent, screen: &mut Screen, global: &mut Globals) {
  match global.mode {
    Mode::Command => match event.code {
      KeyCode::Char(' ') | KeyCode::Enter => global.mode = Mode::Insert,

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
        global.mode = Mode::Insert;
      }
      KeyCode::Char('i') => global.mode = Mode::Insert,

      KeyCode::PageUp => {
        let jump = (screen.get_main_height() as i16) - 1;
        screen.scroll_lines(jump);
      }
      KeyCode::PageDown => {
        let jump = 1 - (screen.get_main_height() as i16);
        screen.scroll_lines(jump);
      }
      KeyCode::Char('q') => global.enqueue(&Sndr::Logout("[ client quit ]")),
      KeyCode::Char('d') => global.mode = Mode::Delete,
      _ => {}
    },
    Mode::Delete => match event.code {
      KeyCode::Char('h') => {
        screen.input_skip_chars(-1);
        // scrn.delete_char();
        global.mode = Mode::Command;
      }
      KeyCode::Char('l') => {
        screen.input_skip_chars(1);
        // scrn.delete_char();
        global.mode = Mode::Command;
      }
      KeyCode::Char('d') => {
        screen.pop_input();
        global.mode = Mode::Command;
      }
      KeyCode::Char('w') => {
        screen.input_delete_words(1);
        global.mode = Mode::Command;
      }
      KeyCode::Char('b') => {
        screen.input_delete_words(-1);
        global.mode = Mode::Command;
      }
      _ => {
        global.mode = Mode::Command;
      }
    },
    _ => {}
  }
}

fn input_key(event: event::KeyEvent, screen: &mut Screen, global: &mut Globals) {
  match event.code {
    KeyCode::Enter => respond_to_user_input(screen.pop_input(), screen, global),

    KeyCode::Backspace => {
      if screen.get_input_length() == 0 {
        global.mode = Mode::Command;
      } else {
        screen.input_backspace();
      }
    }

    KeyCode::Left => screen.input_skip_chars(-1),

    KeyCode::Right => screen.input_skip_chars(1),

    KeyCode::Esc => {
      global.mode = Mode::Command;
    }
    KeyCode::Char(c) => {
      screen.input_char(c);
    }
    _ => {}
  }
}

pub fn process_user_typing(screen: &mut Screen, global: &mut Globals) -> crossterm::Result<bool> {
  let mut should_refresh: bool = false;

  while event::poll(Duration::from_millis(0))? {
    let cur_mode = global.mode;

    match event::read()? {
      Event::Key(event) => {
        trace!("event: {:?}", event);
        match global.mode {
          Mode::Command | Mode::Delete => command_key(event, screen, global),
          Mode::Insert => input_key(event, screen, global),
        }
      }
      Event::Resize(w, h) => screen.resize(w, h),
      _ => {}
    }

    if cur_mode != global.mode {
      write_mode_line(screen, global);
    }
    should_refresh = true;
  }

  Ok(should_refresh)
}

/// Write the mode line to the screen.
pub fn write_mode_line(screen: &mut Screen, global: &Globals) {
  let mut mode_line = Line::default();
  let mch: &str = match global.mode {
    Mode::Insert => "Ins",
    Mode::Command => "Com",
    Mode::Delete => "Del",
  };
  mode_line.pushf(mch, &HIGHLIGHT);
  mode_line.pushf(" │ ", &DIM);
  mode_line.pushf(&(global.uname), &HIGHLIGHT);
  mode_line.push(" @ ");
  mode_line.pushf(&(global.local_addr), &HIGHLIGHT);
  screen.set_stat_ll(mode_line);
}