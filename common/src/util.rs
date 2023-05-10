use unicode_normalization::UnicodeNormalization;

pub fn collapse(s: &str) -> String {
    let trimmed = s.trim();
    let lowercase = trimmed.to_lowercase();
    let without_diacritics: String = lowercase
        .nfd()
        .filter(|c| c.is_ascii() || !c.is_alphabetic())
        .collect();
    let collapsed: String = without_diacritics
        .split_whitespace()
        .collect::<Vec<&str>>()
        .concat();
    collapsed
}
