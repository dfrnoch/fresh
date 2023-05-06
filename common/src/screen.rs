use crossterm::{
  cursor,
  style::{self, Stylize},
  terminal, QueueableCommand,
};
use lazy_static::lazy_static;
use log::trace;
use std::io::{Stdout, Write};

use super::line::*;

const SPACE: char = ' ';
const VBAR: char = '│';
const HBAR: char = '—';

lazy_static! {
  static ref DEFAULT_DIM: Style = Style::new(Some(style::Color::AnsiValue(239)), None, None);
  static ref DEFAULT_DIM_BOLD: Style = Style::new(
    Some(style::Color::AnsiValue(239)),
    None,
    Some(&[style::Attribute::Bold])
  );
  static ref DEFAULT_BOLD: Style = Style::new(None, None, Some(&[style::Attribute::Bold]));
  static ref DEFAULT_HIGHLIGHT: Style = Style::new(Some(style::Color::White), None, None);
  static ref DEFAULT_HIGHLIGHT_BOLD: Style = Style::new(
    Some(style::Color::White),
    None,
    Some(&[style::Attribute::Bold])
  );
  static ref DEFAULT_REVERSE: Style = Style::new(None, None, Some(&[style::Attribute::Reverse]));
  static ref VBARSTR: String = {
    let mut s = String::new();
    s.push(VBAR);
    s
  };
  static ref RESET_ALL: Style = Style::new(
    Some(style::Color::Reset),
    Some(style::Color::Reset),
    Some(&[style::Attribute::Reset])
  );
}

/** This struct holds the different styles used for text shown by the client.
This helps maintain a theme, instead of just setting whatever colors and
attributes wherever.
*/
pub struct Styles {
  pub dim: Style,
  pub dim_bold: Style,
  pub bold: Style,
  pub high: Style,
  pub high_bold: Style,
}

impl std::default::Default for Styles {
  fn default() -> Self {
    Styles {
      dim: DEFAULT_DIM.clone(),
      dim_bold: DEFAULT_DIM_BOLD.clone(),
      bold: DEFAULT_BOLD.clone(),
      high: DEFAULT_HIGHLIGHT.clone(),
      high_bold: DEFAULT_HIGHLIGHT_BOLD.clone(),
    }
  }
}

/** The `Bits` struct holds prerendered bits of stuff that must be
repeatedly written to the screen, like a full-width horizontal separator
and the starting and ending borders of status text on the status lines.
*/
struct Bits {
  stat_begin: String,
  stat_begin_chars: usize,
  stat_end: String,
  stat_end_chars: usize,
  full_hline: String,
}

impl Bits {
  fn new(sty: &Styles, width: u16) -> Bits {
    let mut start = Line::default();
    let mut end = Line::default();
    start.pushf(VBARSTR.as_str(), &sty.dim);
    start.push(" ");
    end.push(" ");
    end.pushf(VBARSTR.as_str(), &sty.dim);

    let mut hline = Line::default();
    {
      let mut s = String::with_capacity(width as usize);
      for _ in 0..width {
        s.push(HBAR);
      }
      hline.pushf(&s, &sty.dim);
    }

    let start_len = start.len();
    let end_len = end.len();

    Bits {
      stat_begin: start.first_n_chars(start_len).to_string(),
      stat_end: end.first_n_chars(end_len).to_string(),
      stat_begin_chars: start_len,
      stat_end_chars: end_len,
      full_hline: hline.first_n_chars((width + 1) as usize).to_string(),
    }
  }
}

/** The `Screen` represents all the state required to display the `fresh`
client UI to the user.
*/
pub struct Screen {
  lines: Vec<Line>,
  input: Vec<char>,
  input_ip: u16,
  roster: Vec<Line>,
  roster_width: u16,
  stat_ul: Line,
  stat_ur: Line,
  stat_ll: Line,
  #[allow(dead_code)]
  stat_lr: Line,
  lines_dirty: bool,
  input_dirty: bool,
  roster_dirty: bool,
  stat_dirty: bool,
  styles: Styles,
  bits: Bits,

  lines_scroll: u16,
  roster_scroll: u16,
  last_x_size: u16,
  last_y_size: u16,
}

impl Screen {
  pub fn new(term: &mut Stdout, roster_chars: u16) -> crossterm::Result<Screen> {
    terminal::enable_raw_mode()?;
    let (x, y): (u16, u16) = terminal::size()?;
    term.queue(cursor::Hide)?.queue(terminal::DisableLineWrap)?;
    term.queue(terminal::SetTitle("Fresh Client"))?;
    term.flush()?;

    let stylez = Styles::default();
    let bitz = Bits::new(&stylez, x);

    Ok(Screen {
      lines: Vec::new(),
      input: Vec::new(),
      roster: Vec::new(),
      roster_width: roster_chars,
      input_ip: 0,
      stat_ul: Line::default(),
      stat_ur: Line::default(),
      stat_ll: Line::default(),
      stat_lr: Line::default(),
      lines_dirty: true,
      input_dirty: true,
      roster_dirty: true,
      stat_dirty: true,
      lines_scroll: 0,
      roster_scroll: 0,
      last_x_size: x,
      last_y_size: y,
      styles: stylez,
      bits: bitz,
    })
  }

  /** Return a reference to the `Styles` struct that contains the styles
  used by this `Screen`. These should be used in calls to
  `ctline::Line::pushf()`.
  */
  pub fn styles(&self) -> &Styles {
    &(self.styles)
  }

  /** Set the color scheme for the terminal. `u8`s are ANSI color numbers;
  setting `underline` true specifies using underlining in place of bold
  text.
  */
  pub fn set_styles(
    &mut self,
    dim_fg: Option<u8>,
    dim_bg: Option<u8>,
    high_fg: Option<u8>,
    high_bg: Option<u8>,
    underline: bool,
  ) {
    let dfg = match dim_fg {
      None => None,
      Some(n) => Some(style::Color::AnsiValue(n)),
    };
    let dbg = match dim_bg {
      None => None,
      Some(n) => Some(style::Color::AnsiValue(n)),
    };
    let hfg = match high_fg {
      None => None,
      Some(n) => Some(style::Color::AnsiValue(n)),
    };
    let hbg = match high_bg {
      None => None,
      Some(n) => Some(style::Color::AnsiValue(n)),
    };
    let attr = match underline {
      true => style::Attribute::Underlined,
      false => style::Attribute::Bold,
    };

    let new_styles = Styles {
      dim: Style::new(dfg, dbg, None),
      dim_bold: Style::new(dfg, dbg, Some(&[attr])),
      bold: Style::new(None, None, Some(&[attr])),
      high: Style::new(hfg, hbg, None),
      high_bold: Style::new(hfg, hbg, Some(&[attr])),
    };

    self.styles = new_styles;
    self.bits = Bits::new(&self.styles, self.last_x_size);
  }

  /** Return the height of the main scrollback window. */
  pub fn get_main_height(&self) -> u16 {
    self.last_y_size - 2
  }

  /** Return the number of `Line`s in the scrollback buffer. */
  pub fn get_scrollback_length(&self) -> usize {
    self.lines.len()
  }

  /** Trim the scrollback buffer to the latest `n` lines. */
  pub fn prune_scrollback(&mut self, n: usize) {
    if n >= self.lines.len() {
      return;
    }
    let new_zero = self.lines.len() - n;

    let temp: Vec<Line> = self.lines.split_off(new_zero);
    self.lines = temp;

    self.lines_dirty = true;
  }

  /** Push the supplied line onto the end of the scrollback buffer. */
  pub fn push_line(&mut self, l: Line) {
    self.lines.push(l);
    self.lines_dirty = true;
  }

  /** Populate the roster with the given slice of strings. */
  pub fn set_roster<T: AsRef<str>>(&mut self, items: &[T]) {
    self.roster = Vec::new();
    for s in items.iter() {
      let mut l: Line = Line::default();
      l.push(s.as_ref());
      self.roster.push(l);
    }
    self.roster_dirty = true;
  }

  /** Get number of characters in the input line. */
  pub fn get_input_length(&self) -> usize {
    self.input.len()
  }

  /** Add a `char` to the input line. */
  pub fn input_char(&mut self, ch: char) {
    if (self.input_ip as usize) >= self.input.len() {
      self.input.push(ch);
      self.input_ip = self.input.len() as u16;
    } else {
      self.input.insert(self.input_ip as usize, ch);
      self.input_ip += 1;
    }
    self.input_dirty = true;
  }

  /** Delete the character on the input line before the cursor.

  Obviously, this does nothing if the cursor is at the beginning.
  */
  pub fn input_backspace(&mut self) {
    let ilen = self.input.len() as u16;
    if ilen == 0 || self.input_ip == 0 {
      return;
    }

    if self.input_ip >= ilen {
      let _ = self.input.pop();
      self.input_ip = ilen - 1;
    } else {
      self.input_ip -= 1;
      let _ = self.input.remove(self.input_ip as usize);
    }
    self.input_dirty = true;
  }

  /** Move the input cursor forward (or backward, for negative values)
  `n_chars`, or to the end (or beginning), if the new position would
  be out of range.
  */
  pub fn input_skip_chars(&mut self, n_chars: i16) {
    let cur = self.input_ip as i16;
    let new = cur + n_chars;
    if new < 0 {
      self.input_ip = 0;
    } else {
      let new: u16 = new as u16;
      let ilen = self.input.len() as u16;
      if new > ilen {
        self.input_ip = ilen;
      } else {
        self.input_ip = new;
      }
    }
    self.input_dirty = true;
  }

  /* Move the input cursor forward to the next word-end location. */
  pub fn input_skip_foreword(&mut self) {
    let uip = self.input_ip as usize;
    if uip == self.input.len() {
      return;
    }

    self.input_dirty = true;
    let mut in_ws = self.input[uip].is_whitespace();

    for (i, c) in self.input[uip..].iter().enumerate() {
      if in_ws {
        if !c.is_whitespace() {
          in_ws = false;
        }
      } else if c.is_whitespace() {
        self.input_ip = (uip + i) as u16;
        return;
      }
    }
    self.input_ip = self.input.len() as u16;
  }

  /* Move the input cursor backward to the previous word-beginning
  location. */
  pub fn input_skip_backword(&mut self) {
    let uip = self.input_ip as usize;
    if uip == 0 {
      return;
    }

    self.input_dirty = true;
    let mut in_ws = false;
    if uip < self.input.len() && self.input[uip - 1].is_whitespace() {
      in_ws = true;
    }

    for (i, c) in self.input[..uip].iter().rev().enumerate() {
      if in_ws {
        if !c.is_whitespace() {
          in_ws = false;
        }
      } else if c.is_whitespace() {
        self.input_ip = (uip - i) as u16;
        return;
      }
    }
    self.input_ip = 0;
  }

  /** Scroll the main display up (or down, for negative values) `n_chars`,
  or to the end (or beginning) if the new position would be out of range.
  */
  pub fn scroll_lines(&mut self, n_chars: i16) {
    let cur = self.lines_scroll as i16;
    let mut new = cur + n_chars;
    if new < 0 {
      new = 0;
    }
    self.lines_scroll = new as u16;
    self.lines_dirty = true;
  }

  /** Scroll the roster up (or down, for negative values) `n_chars`,
  or to the end (or beginning) if the new position would be out of range.
  */
  pub fn scroll_roster(&mut self, n_chars: i16) {
    let rost_vsize = self.last_y_size - 3;
    if rost_vsize as usize >= self.roster.len() {
      if self.roster_scroll != 0 {
        self.roster_dirty = true;
      }
      self.roster_scroll = 0;
      return;
    }
    let max = (self.roster.len() - (rost_vsize as usize)) as i16;
    let new = self.roster_scroll as i16 + n_chars;
    if new < 0 {
      if self.roster_scroll != 0 {
        self.roster_dirty = true;
      }
      self.roster_scroll = 0;
    } else if new > max {
      self.roster_scroll = max as u16;
      self.roster_dirty = true;
    } else {
      self.roster_scroll = new as u16;
      self.roster_dirty = true;
    }
  }

  /** Return the contents of the input line as a String and clear
  the input line.
  */
  pub fn pop_input(&mut self) -> Vec<char> {
    let mut new_v: Vec<char> = Vec::new();
    std::mem::swap(&mut new_v, &mut self.input);
    self.input_ip = 0;
    self.input_dirty = true;
    new_v
  }

  pub fn set_stat_ll(&mut self, new_stat: Line) {
    self.stat_ll = new_stat;
    self.stat_dirty = true;
  }
  pub fn set_stat_ul(&mut self, new_stat: Line) {
    self.stat_ul = new_stat;
    self.stat_dirty = true;
  }
  pub fn set_stat_ur(&mut self, new_stat: Line) {
    self.stat_ur = new_stat;
    self.stat_dirty = true;
  }

  /** Set the size at which the `Screen` should be rendered. This is
  intended to be the entire terminal window.

  If the terminal changes size, this should be called before the next
  call to `.refresh()`, or it probably won't look right.
  */
  pub fn resize(&mut self, cols: u16, rows: u16) {
    if cols != self.last_x_size {
      let mut s = String::with_capacity(cols as usize);
      for _ in 0..cols {
        s.push(HBAR);
      }
      let mut hl = Line::default();
      hl.pushf(&s, &self.styles.dim);
      self.bits.full_hline = hl.first_n_chars(cols as usize).to_string();
    }
    if (cols != self.last_x_size) || (rows != self.last_y_size) {
      self.lines_dirty = true;
      self.input_dirty = true;
      self.roster_dirty = true;
      self.stat_dirty = true;
      self.last_x_size = cols;
      self.last_y_size = rows;
    }
  }

  fn refresh_lines(&mut self, term: &mut Stdout, width: u16, height: u16) -> crossterm::Result<()> {
    trace!("Screen::refresh_lines(..., {}, {}) called", &width, &height);
    let blank: String = {
      let mut s = String::new();
      for _ in 0..width {
        s.push(SPACE);
      }
      s
    };
    let mut y = height - 1;
    let w = width as usize;
    let mut count_back: u16 = 0;
    for aline in self.lines.iter_mut().rev() {
      for row in aline.lines(w).iter().rev() {
        if y == 0 {
          break;
        }
        if count_back >= self.lines_scroll {
          term
            .queue(cursor::MoveTo(0, y))?
            .queue(style::Print(&blank))?
            .queue(cursor::MoveToColumn(0))?
            .queue(style::Print(&row))?;
          y -= 1;
        }
        count_back += 1;
      }
      if y == 0 {
        break;
      }
    }

    /* Check to see if we've scrolled past the end of the scrollback,
    and if so, scroll us forward a little bit and keep
    `self.lines_dirty == true` */
    if y > 1 && self.lines_scroll > 0 {
      let adjust: i16 = (y - 1) as i16;
      self.scroll_lines(-adjust);
    } else {
      while y > 0 {
        term
          .queue(cursor::MoveTo(0, y))?
          .queue(style::Print(&blank))?;
        y -= 1;
      }
      self.lines_dirty = false;
    }
    Ok(())
  }

  fn refresh_roster(
    &mut self,
    term: &mut Stdout,
    xstart: u16,
    height: u16,
  ) -> crossterm::Result<()> {
    trace!(
      "Screen::refresh_roster(..., {}, {}) called",
      &xstart,
      &height
    );
    let rrw: usize = (self.roster_width as usize) + 1;
    let urw: usize = self.roster_width as usize;

    let blank: String = {
      let mut s = String::new();
      for _ in 0..self.roster_width {
        s.push(SPACE);
      }
      let mut l = Line::default();
      l.pushf(VBARSTR.as_str(), &self.styles.dim);
      l.push(&s);
      l.first_n_chars(rrw).to_string()
    };
    let mut y: u16 = 1;
    let targ_y = height;
    let us_scroll = self.roster_scroll as usize;
    for (i, aline) in self.roster.iter_mut().enumerate() {
      if y == targ_y {
        break;
      }
      if i >= us_scroll {
        term
          .queue(cursor::MoveTo(xstart, y))?
          .queue(style::Print(&blank))?
          .queue(cursor::MoveTo(xstart + 1, y))?
          .queue(style::Print(aline.first_n_chars(urw)))?;
        y += 1;
      }
    }
    while y < height {
      term
        .queue(cursor::MoveTo(xstart, y))?
        .queue(style::Print(&blank))?;
      y += 1;
    }
    self.roster_dirty = false;
    Ok(())
  }

  fn refresh_input(&mut self, term: &mut Stdout) -> crossterm::Result<()> {
    term
      .queue(cursor::MoveTo(0, self.last_y_size - 1))?
      .queue(terminal::Clear(terminal::ClearType::CurrentLine))?
      .queue(cursor::MoveToColumn(0))?;

    let third = self.last_x_size / 3;
    let maxpos = self.last_x_size - third;
    let startpos = {
      if self.input.len() < self.last_x_size as usize {
        0
      } else if self.input_ip < third {
        0
      } else if self.input_ip > maxpos {
        self.input_ip - maxpos
      } else {
        self.input_ip - third
      }
    };
    let endpos = {
      if startpos + self.last_x_size > (self.input.len() as u16) {
        self.input.len() as u16
      } else {
        startpos + self.last_x_size
      }
    };

    let input_ip_us = self.input_ip as usize;
    for i in (startpos as usize)..(endpos as usize) {
      let c = self.input[i];
      if i == input_ip_us {
        let cch = style::style(c).attribute(style::Attribute::Reverse);
        term.queue(style::PrintStyledContent(cch))?;
      } else {
        term.queue(style::Print(c))?;
      }
    }
    if input_ip_us == self.input.len() {
      let cch = style::style(SPACE).attribute(style::Attribute::Reverse);
      term.queue(style::PrintStyledContent(cch))?;
    }

    self.input_dirty = false;
    Ok(())
  }

  fn refresh_stat(&mut self, term: &mut Stdout) -> crossterm::Result<()> {
    trace!("Screen::refresh_stat(...) called");

    /* Lower left corner (there is no lower-right as of yet). */
    let stat_pad = 2 + self.bits.stat_begin_chars + self.bits.stat_end_chars;
    let stat_room = (self.last_x_size as usize) - stat_pad;
    let ll_y = self.last_y_size - 2;

    term
      .queue(cursor::MoveTo(0, ll_y))?
      .queue(style::Print(&self.bits.full_hline))?
      .queue(cursor::MoveTo(1, ll_y))?
      .queue(style::Print(&self.bits.stat_begin))?
      .queue(style::Print(self.stat_ll.first_n_chars(stat_room)))?
      .queue(style::Print(&self.bits.stat_end))?;

    let bits_sum = (3 + (self.bits.stat_begin_chars * 2) + (self.bits.stat_end_chars * 2)) as u16;
    let tot_space = self.last_x_size - bits_sum;
    let space_each: usize = (tot_space / 2) as usize;
    let abbrev_space = space_each - 3;

    term
      .queue(cursor::MoveTo(0, 0))?
      .queue(style::Print(&self.bits.full_hline))?
      .queue(cursor::MoveTo(1, 0))?
      .queue(style::Print(&self.bits.stat_begin))?;
    if self.stat_ul.len() > space_each {
      term
        .queue(style::Print(self.stat_ul.first_n_chars(abbrev_space)))?
        .queue(style::Print("..."))?;
    } else {
      term.queue(style::Print(self.stat_ul.first_n_chars(space_each)))?;
    }
    term.queue(style::Print(&self.bits.stat_end))?;

    let ur_offs: u16 = match self.stat_ur.len() > space_each {
      true => {
        self.last_x_size
          - (2 + self.bits.stat_begin_chars + self.bits.stat_end_chars + space_each) as u16
      }
      false => {
        self.last_x_size
          - (2 + self.bits.stat_begin_chars + self.bits.stat_end_chars + self.stat_ur.len()) as u16
      }
    };

    term
      .queue(cursor::MoveTo(ur_offs, 0))?
      .queue(style::Print(&self.bits.stat_begin))?;
    if self.stat_ur.len() > space_each {
      term
        .queue(style::Print(self.stat_ur.first_n_chars(abbrev_space)))?
        .queue(style::Print("..."))?;
    } else {
      term.queue(style::Print(self.stat_ur.first_n_chars(space_each)))?;
    }
    term.queue(style::Print(&self.bits.stat_end))?;

    self.stat_dirty = false;
    Ok(())
  }

  fn announce_term_too_small(&self, term: &mut Stdout) -> crossterm::Result<()> {
    term
      .queue(terminal::Clear(terminal::ClearType::All))?
      .queue(cursor::MoveTo(0, 0))?
      .queue(style::Print(
        "The terminal window is too small. Please make it larger.",
      ))?;
    term.flush()?;

    Ok(())
  }

  /** Redraw any parts of the `Screen` that have changed since the last
  call to `.refresh()`.
  */
  pub fn refresh(&mut self, term: &mut Stdout) -> Result<(), String> {
    trace!("Screen::refresh(...) called");
    if !(self.lines_dirty || self.input_dirty || self.roster_dirty || self.stat_dirty) {
      return Ok(());
    }

    let rost_w = self.roster_width + 1;
    let main_w = self.last_x_size - rost_w;
    let main_h = self.last_y_size - 2;

    if (main_w < 20) || (main_h < 5) {
      match self.announce_term_too_small(term) {
        Err(e) => {
          return Err(format!("{}", e));
        }
        Ok(_) => {
          return Ok(());
        }
      }
    }

    if self.input_dirty {
      if let Err(e) = self.refresh_input(term) {
        return Err(format!("{}", e));
      }
    }
    if self.lines_dirty {
      if let Err(e) = self.refresh_lines(term, main_w, main_h) {
        return Err(format!("{}", e));
      }
    }
    if self.roster_dirty {
      if let Err(e) = self.refresh_roster(term, main_w, main_h) {
        return Err(format!("{}", e));
      }
    }
    if self.stat_dirty {
      if let Err(e) = self.refresh_stat(term) {
        return Err(format!("{}", e));
      }
    }

    if let Err(e) = term.flush() {
      return Err(format!("{}", e));
    }

    Ok(())
  }
}

impl Drop for Screen {
  fn drop(&mut self) {
    let mut term = std::io::stdout();
    term
      .queue(cursor::Show)
      .unwrap()
      .queue(terminal::EnableLineWrap)
      .unwrap()
      .queue(terminal::Clear(terminal::ClearType::All))
      .unwrap()
      .queue(cursor::MoveTo(0, 0))
      .unwrap();
    term.flush().unwrap();
    terminal::disable_raw_mode().unwrap();
  }
}
