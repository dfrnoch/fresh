use crossterm::style;
use lazy_static::lazy_static;

use crate::line::Style;

lazy_static! {
  pub static ref DIM: Style = Style::new(Some(style::Color::AnsiValue(239)), None, None);
  pub static ref DIM_BOLD: Style = Style::new(
    Some(style::Color::AnsiValue(239)),
    None,
    Some(&[style::Attribute::Bold])
  );
  pub static ref BOLD: Style = Style::new(None, None, Some(&[style::Attribute::Bold]));
  pub static ref HIGHLIGHT: Style = Style::new(Some(style::Color::White), None, None);
  pub static ref HIGHLIGHT_BOLD: Style = Style::new(
    Some(style::Color::White),
    None,
    Some(&[style::Attribute::Bold])
  );
  pub static ref REVERSE: Style = Style::new(None, None, Some(&[style::Attribute::Reverse]));
  pub static ref RESET_ALL: Style = Style::new(
    Some(style::Color::Reset),
    Some(style::Color::Reset),
    Some(&[style::Attribute::Reset])
  );
}
