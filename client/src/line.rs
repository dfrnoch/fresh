use lazy_static::lazy_static;
use log::trace;

use crossterm::{style, ExecutableCommand};

lazy_static! {
    static ref RESET: Style = {
        use crossterm::{style, ExecutableCommand};
        let mut buff: Vec<u8> = Vec::new();
        let cols = style::Colors::new(style::Color::Reset, style::Color::Reset);
        buff.execute(style::SetColors(cols)).unwrap();
        buff.execute(style::SetAttribute(style::Attribute::Reset))
            .unwrap();
        Style(String::from_utf8(buff).unwrap())
    };
}

/// A wrapper around a string that can be formatted with ANSI escape codes.
#[derive(Clone)]
pub struct Style(String);

impl Style {
    pub fn new(
        foreground: Option<style::Color>,
        background: Option<style::Color>,
        attributes: Option<&[style::Attribute]>,
    ) -> Style {
        let mut buff = Vec::new();
        let colors = style::Colors {
            foreground,
            background,
        };

        buff.execute(style::SetColors(colors)).unwrap();

        if let Some(attrs) = attributes {
            attrs
                .iter()
                .try_for_each(|attr| buff.execute(style::SetAttribute(*attr)).map(|_| ()))
                .unwrap();
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

/// This struct is used in the `Line` internals to store formatting info.
#[derive(Clone)]
struct Fmtr {
    index: usize,
    code: Style,
}

impl Fmtr {
    fn new(index: usize, from: &Style) -> Fmtr {
        Fmtr {
            index,
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
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn push<T: AsRef<str>>(&mut self, string: T) {
        self.width = None;
        self.nchars = None;
        for c in string.as_ref().chars() {
            self.chars.push(c);
        }
    }

    /// Add a chunk of _formatted_ text to the end of the `Line`.
    pub fn pushf<T: AsRef<str>>(&mut self, string: T, style: &Style) {
        self.width = None;
        self.nchars = None;

        let start_index = self.chars.len();
        self.fdirs.push(Fmtr::new(start_index, style));

        self.chars.extend(string.as_ref().chars());

        let end_index = self.chars.len();
        self.fdirs.push(Fmtr::new(end_index, &RESET));
    }

    fn wrap(&mut self, width: usize) {
        let mut wraps: Vec<usize> = Vec::with_capacity(1 + self.chars.len() / width);
        let mut current_pos: usize = 0;
        let mut last_whitespace_pos: usize = 0;
        let mut write_leading_ws: bool = true;

        trace!("chars: {}", &(self.chars.iter().collect::<String>()));

        for (i, c) in self.chars.iter().enumerate() {
            if current_pos == width {
                wraps.push(if i - width >= last_whitespace_pos {
                    i
                } else {
                    last_whitespace_pos
                });
                current_pos = i - last_whitespace_pos;
                write_leading_ws = false;
            }

            if (current_pos > 0 || write_leading_ws) || !c.is_whitespace() {
                current_pos += 1;
            }

            if c.is_whitespace() {
                last_whitespace_pos = i;
            }
        }

        trace!("wraps at: {:?}", &wraps);

        self.render = Vec::with_capacity(wraps.len() + 1);
        let mut fmt_iter = self.fdirs.iter().peekable();
        let mut cur_line = String::with_capacity(width);
        write_leading_ws = true;
        let mut wrap_idx: usize = 0;
        let mut line_len: usize = 0;

        for (i, c) in self.chars.iter().enumerate() {
            if wrap_idx < wraps.len() && wraps[wrap_idx] == i {
                self.render.push(cur_line.clone());
                cur_line.clear();
                write_leading_ws = false;
                wrap_idx += 1;
                line_len = 0;
            }

            while let Some(f) = fmt_iter.peek() {
                if f.index == i {
                    cur_line.push_str(&f.code);
                    fmt_iter.next();
                } else {
                    break;
                }
            }

            if line_len > 0 || write_leading_ws || !c.is_whitespace() {
                cur_line.push(*c);
                line_len += 1;
            }
        }

        for f in fmt_iter {
            cur_line.push_str(&f.code);
        }

        self.render.push(cur_line);
        self.width = Some(width);
    }

    pub fn lines(&mut self, width: usize) -> &[String] {
        if self.width.map_or(true, |n| n != width) {
            self.wrap(width);
        }

        &self.render
    }

    fn render_n_chars(&mut self, n: usize) {
        let mut rendered_string = String::default();
        let mut format_iter = self.fdirs.iter().peekable();

        for (i, &c) in self.chars[..n].iter().enumerate() {
            while let Some(format) = format_iter.peek() {
                if format.index == i {
                    rendered_string.push_str(&format.code);
                    format_iter.next();
                } else {
                    break;
                }
            }
            rendered_string.push(c);
        }

        for format in format_iter {
            rendered_string.push_str(&format.code);
        }

        self.nchars = Some(n);
        self.nchars_render = rendered_string;
    }

    pub fn first_n_chars(&mut self, n: usize) -> &str {
        let target = n.min(self.chars.len());

        if self.nchars.map_or(true, |i| target != i) {
            self.render_n_chars(target);
        }

        &self.nchars_render
    }
}
