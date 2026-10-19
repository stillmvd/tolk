use crate::text::is_word_char;
use std::time::{Duration, Instant};

const LIMIT: usize = 200;
const IDLE: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct WordBuffer {
    text: String,
    owner: isize,
    at: Option<Instant>,
}

impl WordBuffer {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn owner(&self) -> isize {
        self.owner
    }

    pub fn reset(&mut self) {
        self.text.clear();
        self.at = None;
    }

    pub fn set(&mut self, owner: isize, text: String) {
        self.owner = owner;
        self.text = text;
        self.at = Some(Instant::now());
    }

    pub fn push(&mut self, owner: isize, c: char) {
        if owner != self.owner || self.at.is_some_and(|t| t.elapsed() > IDLE) {
            self.text.clear();
            self.owner = owner;
        }
        if matches!(c, '\n' | '\r') {
            self.text.clear();
        } else {
            self.text.push(c);
        }
        if self.text.chars().count() > LIMIT {
            let cut = self.text.char_indices().nth(self.text.chars().count() - LIMIT).map_or(0, |(i, _)| i);
            self.text.drain(..cut);
        }
        self.at = Some(Instant::now());
    }

    pub fn back(&mut self) {
        self.text.pop();
        self.at = Some(Instant::now());
    }

    pub fn back_word(&mut self) {
        while self.text.ends_with(|c: char| !is_word_char(c)) {
            self.text.pop();
        }
        while self.text.ends_with(is_word_char) {
            self.text.pop();
        }
        self.at = Some(Instant::now());
    }

    pub fn has_word(&self, owner: isize) -> bool {
        owner == self.owner && self.text.chars().any(|c| c.is_alphanumeric()) && self.at.is_none_or(|t| t.elapsed() <= IDLE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_typing_per_window() {
        let mut b = WordBuffer::default();
        "открыть замоа".chars().for_each(|c| b.push(1, c));
        b.back();
        b.push(1, 'к');
        assert_eq!(b.text(), "открыть замок");
        assert!(b.has_word(1) && !b.has_word(2));
        b.back_word();
        assert_eq!(b.text(), "открыть ");
        b.push(2, 'x');
        assert_eq!(b.text(), "x");
        b.push(2, '\n');
        assert!(!b.has_word(2));
    }

    #[test]
    fn keeps_last_limit_chars() {
        let mut b = WordBuffer::default();
        (0..LIMIT + 50).for_each(|_| b.push(1, 'я'));
        assert_eq!(b.text().chars().count(), LIMIT);
    }
}
