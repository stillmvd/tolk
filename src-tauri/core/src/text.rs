#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    RuEn,
    EnRu,
}

pub fn direction(text: &str) -> Option<Dir> {
    let (mut ru, mut en) = (0usize, 0usize);
    for c in text.chars() {
        match c {
            'а'..='я' | 'А'..='Я' | 'ё' | 'Ё' => ru += 1,
            'a'..='z' | 'A'..='Z' => en += 1,
            _ => {}
        }
    }
    match (ru, en) {
        (0, 0) => None,
        _ if ru >= en => Some(Dir::RuEn),
        _ => Some(Dir::EnRu),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_direction_by_majority() {
        assert_eq!(direction("замок"), Some(Dir::RuEn));
        assert_eq!(direction("hello"), Some(Dir::EnRu));
        assert_eq!(direction("открыть API"), Some(Dir::RuEn));
        assert_eq!(direction("123 :)"), None);
    }
}
