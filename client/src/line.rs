use lazy_static::lazy_static;
use log::trace;

use crossterm::{style, ExecutableCommand};

lazy_static! {
    static ref RESET: Style = {
        use crossterm::{style, ExecutableCommand};
        let mut buff: Vec<u8> = Vec::new();
        let color_codes = style::Colors::new(style::Color::Reset, style::Color::Reset);
        buff.execute(style::SetColors(color_codes)).unwrap();
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
    max_line_width: Option<usize>,
    num_characters: Option<usize>,
    format_directions: Vec<Fmtr>,
    rendered_lines: Vec<String>,
    rendered_substring: String,
}

impl Line {
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn push<T: AsRef<str>>(&mut self, string: T) {
        self.max_line_width = None;
        self.num_characters = None;
        for c in string.as_ref().chars() {
            self.chars.push(c);
        }
    }

    /// Add a chunk of _formatted_ text to the end of the `Line`.
    pub fn pushf<T: AsRef<str>>(&mut self, string: T, style: &Style) {
        self.max_line_width = None;
        self.num_characters = None;

        let start_index = self.chars.len();
        self.format_directions.push(Fmtr::new(start_index, style));

        self.chars.extend(string.as_ref().chars());

        let end_index = self.chars.len();
        self.format_directions.push(Fmtr::new(end_index, &RESET));
    }

    fn wrap(&mut self, width: usize) {
        let mut wraps: Vec<usize> = Vec::with_capacity(1 + self.chars.len() / width);
        let mut current_pos: usize = 0;
        let mut last_whitespace_pos: usize = 0;
        let mut include_leading_whitespace: bool = true;

        trace!("chars: {}", &(self.chars.iter().collect::<String>()));

        for (i, c) in self.chars.iter().enumerate() {
            if current_pos == width {
                wraps.push(if i - width >= last_whitespace_pos {
                    i
                } else {
                    last_whitespace_pos
                });
                current_pos = i - last_whitespace_pos;
                include_leading_whitespace = false;
            }

            if (current_pos > 0 || include_leading_whitespace) || !c.is_whitespace() {
                current_pos += 1;
            }

            if c.is_whitespace() {
                last_whitespace_pos = i;
            }
        }

        trace!("wraps at: {:?}", &wraps);

        self.rendered_lines = Vec::with_capacity(wraps.len() + 1);
        let mut fmt_iter = self.format_directions.iter().peekable();
        let mut current_line = String::with_capacity(width);
        include_leading_whitespace = true;
        let mut line_wrap_index: usize = 0;
        let mut current_line_length: usize = 0;

        for (i, c) in self.chars.iter().enumerate() {
            if line_wrap_index < wraps.len() && wraps[line_wrap_index] == i {
                self.rendered_lines.push(current_line.clone());
                current_line.clear();
                include_leading_whitespace = false;
                line_wrap_index += 1;
                current_line_length = 0;
            }

            while let Some(f) = fmt_iter.peek() {
                if f.index == i {
                    current_line.push_str(&f.code);
                    fmt_iter.next();
                } else {
                    break;
                }
            }

            if current_line_length > 0 || include_leading_whitespace || !c.is_whitespace() {
                current_line.push(*c);
                current_line_length += 1;
            }
        }

        for f in fmt_iter {
            current_line.push_str(&f.code);
        }

        self.rendered_lines.push(current_line);
        self.max_line_width = Some(width);
    }

    pub fn lines(&mut self, width: usize) -> &[String] {
        if self.max_line_width.map_or(true, |n| n != width) {
            self.wrap(width);
        }

        &self.rendered_lines
    }

    fn render_n_chars(&mut self, n: usize) {
        let mut rendered_string = String::default();
        let mut format_iter = self.format_directions.iter().peekable();

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

        self.num_characters = Some(n);
        self.rendered_substring = rendered_string;
    }

    pub fn first_n_chars(&mut self, n: usize) -> &str {
        let substring_length = n.min(self.chars.len());

        if self.num_characters.map_or(true, |i| substring_length != i) {
            self.render_n_chars(substring_length);
        }

        &self.rendered_substring
    }
}
