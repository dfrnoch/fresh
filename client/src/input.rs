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

fn command_key(evt: event::KeyEvent, scrn: &mut Screen, gv: &mut Globals) {
  match gv.mode {
    Mode::Command => match evt.code {
      KeyCode::Char(' ') | KeyCode::Enter => gv.mode = Mode::Insert,

      KeyCode::Up | KeyCode::Char('k') => scrn.scroll_lines(1),
      KeyCode::Down | KeyCode::Char('j') => scrn.scroll_lines(-1),

      KeyCode::Left | KeyCode::Char('h') => scrn.input_skip_chars(-1),
      KeyCode::Right | KeyCode::Char('l') => scrn.input_skip_chars(1),

      KeyCode::Char('w') => scrn.input_skip_words(1),
      KeyCode::Char('b') => scrn.input_skip_words(-1),

      KeyCode::Char('0') => {
        let delta = scrn.get_input_length() as i16;
        scrn.input_skip_chars(-delta);
      }
      KeyCode::Char('$') => {
        let delta = scrn.get_input_length() as i16;
        scrn.input_skip_chars(delta);
      }

      KeyCode::Char('a') => {
        scrn.input_skip_chars(1);
        gv.mode = Mode::Insert;
      }
      KeyCode::Char('i') => gv.mode = Mode::Insert,

      KeyCode::PageUp => {
        let jump = (scrn.get_main_height() as i16) - 1;
        scrn.scroll_lines(jump);
      }
      KeyCode::PageDown => {
        let jump = 1 - (scrn.get_main_height() as i16);
        scrn.scroll_lines(jump);
      }
      KeyCode::Char('q') => gv.enqueue(&Sndr::Logout("[ client quit ]")),
      KeyCode::Char('d') => gv.mode = Mode::Delete,
      _ => {}
    },
    Mode::Delete => match evt.code {
      KeyCode::Char('h') => {
        scrn.input_skip_chars(-1);
        // scrn.delete_char();
        gv.mode = Mode::Command;
      }
      KeyCode::Char('l') => {
        scrn.input_skip_chars(1);
        // scrn.delete_char();
        gv.mode = Mode::Command;
      }
      KeyCode::Char('d') => {
        scrn.pop_input();
        gv.mode = Mode::Command;
      }
      KeyCode::Char('w') => {
        scrn.input_delete_words(1);
        gv.mode = Mode::Command;
      }
      KeyCode::Char('b') => {
        scrn.input_delete_words(-1);
        gv.mode = Mode::Command;
      }
      _ => {
        gv.mode = Mode::Command;
      }
    },
    _ => {}
  }
}

fn input_key(evt: event::KeyEvent, scrn: &mut Screen, gv: &mut Globals) {
  match evt.code {
    KeyCode::Enter => respond_to_user_input(scrn.pop_input(), scrn, gv),

    KeyCode::Backspace => {
      if scrn.get_input_length() == 0 {
        gv.mode = Mode::Command;
      } else {
        scrn.input_backspace();
      }
    }

    KeyCode::Left => scrn.input_skip_chars(-1),

    KeyCode::Right => scrn.input_skip_chars(1),

    KeyCode::Esc => {
      gv.mode = Mode::Command;
    }
    KeyCode::Char(c) => {
      scrn.input_char(c);
    }
    _ => {}
  }
}

pub fn process_user_typing(scrn: &mut Screen, gv: &mut Globals) -> crossterm::Result<bool> {
  let mut should_refresh: bool = false;

  while event::poll(Duration::from_millis(0))? {
    let cur_mode = gv.mode;

    match event::read()? {
      Event::Key(evt) => {
        trace!("event: {:?}", evt);
        match gv.mode {
          Mode::Command | Mode::Delete => command_key(evt, scrn, gv),
          Mode::Insert => input_key(evt, scrn, gv),
        }
      }
      Event::Resize(w, h) => scrn.resize(w, h),
      _ => {}
    }

    if cur_mode != gv.mode {
      write_mode_line(scrn, gv);
    }
    should_refresh = true;
  }

  Ok(should_refresh)
}

/// Write the mode line to the screen.
pub fn write_mode_line(scrn: &mut Screen, gv: &Globals) {
  let mut mode_line = Line::default();
  let mch: &str = match gv.mode {
    Mode::Insert => "Ins",
    Mode::Command => "Com",
    Mode::Delete => "Del",
  };
  mode_line.pushf(mch, &HIGHLIGHT);
  mode_line.pushf(" │ ", &DIM);
  mode_line.pushf(&(gv.uname), &HIGHLIGHT);
  mode_line.push(" @ ");
  mode_line.pushf(&(gv.local_addr), &HIGHLIGHT);
  scrn.set_stat_ll(mode_line);
}
