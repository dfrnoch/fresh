use lazy_static::lazy_static;
use log::trace;

use crossterm::{style, ExecutableCommand};

lazy_static! {
  static ref RESET: Style = {
    use crossterm::{style, ExecutableCommand};
    let mut buff: Vec<u8> = Vec::new();
    let cols = style::Colors::new(style::Color::Reset, style::Color::Reset);
    buff.execute(style::SetColors(cols)).unwrap();
    buff
      .execute(style::SetAttribute(style::Attribute::Reset))
      .unwrap();
    Style(String::from_utf8(buff).unwrap())
  };
}

/** A `Style` is just a wrapper for a string containing the ANSI codes to
write text in a given style to the terminal.
*/
#[derive(Clone)]
pub struct Style(String);

impl Style {
  pub fn new(
    fg: Option<style::Color>,
    bg: Option<style::Color>,
    attrs: Option<&[style::Attribute]>,
  ) -> Style {
    let mut buff = Vec::new();
    let cols = style::Colors {
      foreground: fg,
      background: bg,
    };
    buff.execute(style::SetColors(cols)).unwrap();
    if let Some(x) = attrs {
      for attr in x.iter() {
        buff.execute(style::SetAttribute(*attr)).unwrap();
      }
    }

    Style(String::from_utf8(buff).unwrap())
  }
}

impl std::ops::Deref for Style {
  type Target = str;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

/** This struct is used in the `Line` internals to store formatting info. */
#[derive(Clone)]
struct Fmtr {
  idx: usize,
  code: Style,
}

impl Fmtr {
  fn new(i: usize, from: &Style) -> Fmtr {
    Fmtr {
      idx: i,
      code: from.clone(),
    }
  }
}

#[derive(Default)]
pub struct Line {
  chars: Vec<char>,
  width: Option<usize>,
  nchars: Option<usize>,
  fdirs: Vec<Fmtr>,
  render: Vec<String>,
  nchars_render: String,
}

impl Line {
  /** Return the number of characters in the `Line`. */
  pub fn len(&self) -> usize {
    self.chars.len()
  }

  /** Add a chunk of unformatted text to the end of the `Line`. */
  pub fn push<T: AsRef<str>>(&mut self, s: T) {
    self.width = None;
    self.nchars = None;
    for c in s.as_ref().chars() {
      self.chars.push(c);
    }
  }

  /** Add a chunk of _formatted_ text to the end of the `Line`. */
  pub fn pushf<T: AsRef<str>>(&mut self, s: T, styl: &Style) {
    self.width = None;
    self.nchars = None;

    let mut n: usize = self.chars.len();
    self.fdirs.push(Fmtr::new(n, styl));

    for c in s.as_ref().chars() {
      self.chars.push(c);
    }

    n = self.chars.len();
    self.fdirs.push(Fmtr::new(n, &RESET));
  }

  /** Append a copy of the contents of `other` to `self`. */
  pub fn append(&mut self, other: &Self) {
    self.width = None;
    self.nchars = None;

    let base = self.chars.len();
    for c in other.chars.iter() {
      self.chars.push(*c);
    }
    for f in other.fdirs.iter() {
      self.fdirs.push(Fmtr::new(base + f.idx, &f.code));
    }
  }

  fn wrap(&mut self, tgt: usize) {
    let mut wraps: Vec<usize> = Vec::with_capacity(1 + self.chars.len() / tgt);
    let mut x: usize = 0;
    let mut lws: usize = 0;
    let mut write_leading_ws: bool = true;

    trace!("chars: {}", &(self.chars.iter().collect::<String>()));

    for (i, c) in self.chars.iter().enumerate() {
      if x == tgt {
        if i - tgt >= lws {
          wraps.push(i);
          x = 0;
        } else {
          wraps.push(lws);
          x = i - lws;
        }
        write_leading_ws = false;
      }

      if c.is_whitespace() {
        lws = i;
        if x > 0 || write_leading_ws {
          x += 1;
        }
      } else {
        x += 1;
      }
    }

    trace!("wraps at: {:?}", &wraps);

    self.render = Vec::with_capacity(wraps.len() + 1);
    let mut fmt_iter = self.fdirs.iter();
    let mut nextf = fmt_iter.next();
    let mut cur_line = String::with_capacity(tgt);
    write_leading_ws = true;
    let mut wrap_idx: usize = 0;
    let mut line_len: usize = 0;

    for (i, c) in self.chars.iter().enumerate() {
      if wrap_idx < wraps.len() && wraps[wrap_idx] == i {
        self.render.push(cur_line);
        cur_line = String::with_capacity(tgt);
        write_leading_ws = false;
        wrap_idx += 1;
        line_len = 0;
      }

      while match nextf {
        None => false,
        Some(f) => {
          if f.idx == i {
            cur_line.push_str(&f.code);
            nextf = fmt_iter.next();
            true
          } else {
            false
          }
        }
      } {}

      if line_len > 0 || write_leading_ws || !c.is_whitespace() {
        cur_line.push(*c);
        line_len += 1
      }
    }

    while let Some(f) = nextf {
      cur_line.push_str(&f.code);
      nextf = fmt_iter.next();
    }

    self.render.push(cur_line);

    self.width = Some(tgt);
  }

  pub fn lines(&mut self, width: usize) -> &[String] {
    if self.width.map_or(true, |n| n != width) {
      self.wrap(width);
    }

    &self.render
  }

  fn render_n_chars(&mut self, n: usize) {
    let mut s = String::default();
    let mut fmt_iter = self.fdirs.iter().peekable();

    for (i, c) in self.chars[..n].iter().enumerate() {
      while let Some(f) = fmt_iter.peek() {
        if f.idx == i {
          s.push_str(&f.code);
          fmt_iter.next();
        } else {
          break;
        }
      }
      s.push(*c);
    }

    while let Some(f) = fmt_iter.next() {
      s.push_str(&f.code);
    }

    self.nchars = Some(n);
    self.nchars_render = s;
  }

  pub fn first_n_chars(&mut self, n: usize) -> &str {
    let tgt = n.min(self.chars.len());

    if self.nchars.map_or(true, |i| tgt != i) {
      self.render_n_chars(tgt);
    }

    &self.nchars_render
  }
}
