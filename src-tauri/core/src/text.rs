#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
        }
    }

    pub fn langid(self) -> u16 {
        match self {
            Lang::Ru => 0x0419,
            Lang::En => 0x0409,
        }
    }

    pub fn from_langid(id: u16) -> Option<Lang> {
        match id & 0x3FF {
            0x19 => Some(Lang::Ru),
            0x09 => Some(Lang::En),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Dir {
    RuEn,
    EnRu,
}

impl Dir {
    pub fn from(self) -> Lang {
        match self {
            Dir::RuEn => Lang::Ru,
            Dir::EnRu => Lang::En,
        }
    }

    pub fn to(self) -> Lang {
        match self {
            Dir::RuEn => Lang::En,
            Dir::EnRu => Lang::Ru,
        }
    }

    pub fn flip(self) -> Dir {
        match self {
            Dir::RuEn => Dir::EnRu,
            Dir::EnRu => Dir::RuEn,
        }
    }
}

pub fn is_ru(c: char) -> bool {
    matches!(c, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '\'' | '’')
}

pub fn direction(text: &str) -> Option<Dir> {
    let (mut ru, mut en) = (0usize, 0usize);
    for c in text.chars() {
        if is_ru(c) {
            ru += 1;
        } else if c.is_ascii_alphabetic() {
            en += 1;
        }
    }
    match (ru, en) {
        (0, 0) => None,
        _ if ru >= en => Some(Dir::RuEn),
        _ => Some(Dir::EnRu),
    }
}

fn in_word(chars: &[(usize, char)], i: usize) -> bool {
    let c = chars[i].1;
    c.is_alphanumeric()
        || (is_word_char(c) && i > 0 && i + 1 < chars.len() && chars[i - 1].1.is_alphanumeric() && chars[i + 1].1.is_alphanumeric())
}

pub fn tail_words(text: &str, n: usize) -> Option<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = chars.len();
    while i > 0 && !in_word(&chars, i - 1) {
        i -= 1;
    }
    let mut found = 0;
    let mut start = None;
    while found < n {
        if i == 0 || !in_word(&chars, i - 1) {
            return start;
        }
        while i > 0 && in_word(&chars, i - 1) {
            i -= 1;
        }
        found += 1;
        start = Some(chars[i].0);
        while i > 0 && !in_word(&chars, i - 1) && !matches!(chars[i - 1].1, '.' | '!' | '?' | '\n' | '…') {
            i -= 1;
        }
    }
    start
}

pub fn word_count(text: &str) -> usize {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    (0..chars.len()).filter(|&i| in_word(&chars, i) && (i == 0 || !in_word(&chars, i - 1))).count()
}

pub fn chunks(text: &str, max: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while rest.chars().count() > max {
        let limit = rest.char_indices().nth(max).map_or(rest.len(), |(i, _)| i);
        let head = &rest[..limit];
        let after = |i: usize| i + head[i..].chars().next().map_or(1, char::len_utf8);
        let cut = head
            .rfind('\n')
            .or_else(|| head.rfind(['.', '!', '?', '…']))
            .or_else(|| head.rfind(' '))
            .map(after)
            .unwrap_or(limit);
        out.push(&rest[..cut]);
        rest = &rest[cut..];
    }
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

const END_PUNCT: [char; 7] = ['.', '!', '?', ',', ';', ':', '…'];

pub fn dress(src: &str, translated: &str) -> String {
    let core = src.trim();
    let lead = &src[..src.len() - src.trim_start().len()];
    let trail = &src[src.trim_end().len()..];
    let mut out = translated.trim().to_string();
    let letters: Vec<char> = core.chars().filter(|c| c.is_alphabetic()).collect();
    let shouting = letters.len() > 1 && letters.iter().all(|c| c.is_uppercase());
    if shouting {
        out = out.to_uppercase();
    } else if let (Some(s), Some(t)) = (letters.first(), out.chars().next()) {
        if s.is_uppercase() && t.is_lowercase() {
            out = t.to_uppercase().collect::<String>() + &out[t.len_utf8()..];
        } else if s.is_lowercase() && t.is_uppercase() && word_count(core) == 1 {
            out = t.to_lowercase().collect::<String>() + &out[t.len_utf8()..];
        }
    }
    let src_end = core.chars().last().filter(|c| END_PUNCT.contains(c));
    let out_end = out.chars().last().filter(|c| END_PUNCT.contains(c));
    match (src_end, out_end) {
        (None, Some('.')) => {
            out.pop();
        }
        (Some(s), None) => out.push(s),
        (Some(s), Some(o)) if s != o && matches!(s, '?' | '!') => {
            out.pop();
            out.push(s);
        }
        _ => {}
    }
    format!("{lead}{out}{trail}")
}

pub fn dress_variants(src: &str, variants: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for v in variants.iter().map(|v| dress(src, v)) {
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_are_unique_after_dressing() {
        let v = ["Привет".to_string(), "привет".to_string(), "Привет!".to_string()];
        let got = dress_variants("Hello", &v);
        let unique: std::collections::HashSet<_> = got.iter().collect();
        assert_eq!(unique.len(), got.len());
        assert!(got.len() < v.len());
    }

    #[test]
    fn picks_direction_by_majority() {
        assert_eq!(direction("замок"), Some(Dir::RuEn));
        assert_eq!(direction("hello"), Some(Dir::EnRu));
        assert_eq!(direction("открыть API"), Some(Dir::RuEn));
        assert_eq!(direction("123 :)"), None);
    }

    #[test]
    fn tail_words_extends_back_within_sentence() {
        let t = "Привет. Надо открыть замок ";
        assert_eq!(&t[tail_words(t, 1).unwrap()..], "замок ");
        assert_eq!(&t[tail_words(t, 2).unwrap()..], "открыть замок ");
        assert_eq!(&t[tail_words(t, 3).unwrap()..], "Надо открыть замок ");
        assert_eq!(&t[tail_words(t, 4).unwrap()..], "Надо открыть замок ");
        assert_eq!(tail_words("  ", 1), None);
        assert_eq!(&"it's done"[tail_words("it's done", 2).unwrap()..], "it's done");
        let q = "say 'hello'";
        assert_eq!(&q[tail_words(q, 1).unwrap()..], "hello'");
        assert_eq!(word_count("a - b"), 2);
        assert_eq!(word_count("кто-то пришёл"), 2);
    }

    #[test]
    fn chunks_split_on_boundaries_and_rejoin() {
        let text = "Первое предложение. Второе предложение!\nТретья строка без точки и ещё слова";
        for max in [10, 25, 40, 1000] {
            let parts = chunks(text, max);
            assert_eq!(parts.concat(), text);
            assert!(parts.iter().all(|p| p.chars().count() <= max), "{max}: {parts:?}");
        }
        assert_eq!(chunks(text, 45)[0], "Первое предложение. Второе предложение!\n");
        assert_eq!(chunks("", 10), Vec::<&str>::new());
    }

    #[test]
    fn dress_keeps_case_punctuation_and_spaces() {
        assert_eq!(dress("Замок.", "lock"), "Lock.");
        assert_eq!(dress("замок", "Lock."), "lock");
        assert_eq!(dress(" hello ", "привет"), " привет ");
        assert_eq!(dress("Как дела?", "How are you?"), "How are you?");
        assert_eq!(dress("NASA", "НАСА"), "НАСА");
        assert_eq!(dress("ПРИВЕТ", "Hello"), "HELLO");
        assert_eq!(dress("Ты где?", "Where are you."), "Where are you?");
        assert_eq!(dress("good morning, friends", "Доброе утро, друзья"), "Доброе утро, друзья");
    }
}
