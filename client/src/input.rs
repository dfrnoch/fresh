use crate::{
  connection::{Globals, Mode},
  message::respond_to_user_input,
  screen::Screen,
  util::styles::{DIM, HIGHLIGHT},
};
use common::{line::Line, proto::Sndr};
use crossterm::{event, event::Event, event::KeyCode};
use log::trace;
use std::time::Duration;

fn command_key(evt: event::KeyEvent, scrn: &mut Screen, gv: &mut Globals) {
  match evt.code {
    KeyCode::Char(' ') | KeyCode::Enter => {
      gv.mode = Mode::Input;
    }
    KeyCode::Up | KeyCode::Char('k') => {
      scrn.scroll_lines(1);
    }
    KeyCode::Down | KeyCode::Char('j') => {
      scrn.scroll_lines(-1);
    }
    KeyCode::PageUp => {
      let jump = (scrn.get_main_height() as i16) - 1;
      scrn.scroll_lines(jump);
    }
    KeyCode::PageDown => {
      let jump = 1 - (scrn.get_main_height() as i16);
      scrn.scroll_lines(jump);
    }
    KeyCode::Char('q') => {
      gv.enqueue(&Sndr::Logout("[ client quit  ]"));
    }
    _ => {}
  }
}

fn input_key(evt: event::KeyEvent, scrn: &mut Screen, gv: &mut Globals) {
  match evt.code {
    KeyCode::Enter => {
      let cv = scrn.pop_input();
      respond_to_user_input(cv, scrn, gv);
    }
    KeyCode::Backspace => {
      if scrn.get_input_length() == 0 {
        gv.mode = Mode::Command;
      } else {
        scrn.input_backspace();
      }
    }
    KeyCode::Left => {
      if evt.modifiers.contains(event::KeyModifiers::ALT) {
        scrn.input_skip_backword();
      } else {
        scrn.input_skip_chars(-1);
      }
    }
    KeyCode::Right => {
      if evt.modifiers.contains(event::KeyModifiers::ALT) {
        scrn.input_skip_foreword();
      } else {
        scrn.input_skip_chars(1);
      }
    }
    KeyCode::Home => {
      let delta = scrn.get_input_length() as i16;
      scrn.input_skip_chars(-delta);
    }
    KeyCode::End => {
      let delta = scrn.get_input_length() as i16;
      scrn.input_skip_chars(delta);
    }
    KeyCode::Esc => {
      gv.mode = Mode::Command;
    }
    KeyCode::Char('\u{1b}') => {
      if evt.modifiers.contains(event::KeyModifiers::ALT) {
        gv.mode = Mode::Command;
      }
    }
    KeyCode::Char(c) => {
      scrn.input_char(c);
    }
    _ => { /* */ }
  }
}

/** While the terminal polls that events are available, read them and
act accordingly.

Returns `true` if an event was read, so the calling code can know whether
to redraw (some portion of) the screen.
*/
pub fn process_user_typing(scrn: &mut Screen, gv: &mut Globals) -> crossterm::Result<bool> {
  let mut should_refresh: bool = false;

  while event::poll(Duration::from_millis(0))? {
    let cur_mode = gv.mode;

    match event::read()? {
      Event::Key(evt) => {
        trace!("event: {:?}", evt);
        match gv.mode {
          Mode::Command => command_key(evt, scrn, gv),
          Mode::Input => input_key(evt, scrn, gv),
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

/** When the mode line (in the lower-left-hand corner) should change,
this updates it.
*/
pub fn write_mode_line(scrn: &mut Screen, gv: &Globals) {
  let mut mode_line = Line::default();
  let mch: &str = match gv.mode {
    Mode::Command => "Com",
    Mode::Input => "Ipt",
  };
  mode_line.pushf(mch, &HIGHLIGHT);
  mode_line.pushf(" | ", &DIM);
  mode_line.pushf(&(gv.uname), &HIGHLIGHT);
  mode_line.push(" @ ");
  mode_line.pushf(&(gv.local_addr), &HIGHLIGHT);
  scrn.set_stat_ll(mode_line);
}
