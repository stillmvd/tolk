use super::{Engine, Translation};
use serde::Deserialize;

pub const GTX: &str = "https://translate.googleapis.com/translate_a/single";
pub const CLIENTS5: &str = "https://clients5.google.com/translate_a/t";
pub const MAX_VARIANTS: usize = 5;

#[derive(Deserialize)]
struct Gtx {
    #[serde(default)]
    sentences: Vec<Sentence>,
    #[serde(default)]
    dict: Vec<Dict>,
    #[serde(default)]
    alternative_translations: Vec<Alternatives>,
}

#[derive(Deserialize)]
struct Sentence {
    trans: Option<String>,
}

#[derive(Deserialize)]
struct Dict {
    #[serde(default)]
    entry: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    word: String,
    #[serde(default)]
    score: f64,
}

#[derive(Deserialize)]
struct Alternatives {
    #[serde(default)]
    alternative: Vec<Alternative>,
}

#[derive(Deserialize)]
struct Alternative {
    word_postproc: String,
}

pub fn parse_gtx(json: &str, single_word: bool) -> Option<Translation> {
    let g: Gtx = serde_json::from_str(json).ok()?;
    let text = g.sentences.iter().filter_map(|s| s.trans.as_deref()).collect::<String>().trim().to_string();
    if text.is_empty() {
        return None;
    }
    let mut variants = vec![text.clone()];
    if single_word {
        let alts = g.alternative_translations.iter().take(1).flat_map(|a| a.alternative.iter().map(|x| x.word_postproc.clone()));
        let mut entries: Vec<&Entry> = g.dict.iter().flat_map(|d| d.entry.iter()).collect();
        entries.sort_by(|a, b| b.score.total_cmp(&a.score));
        for v in alts.chain(entries.into_iter().map(|e| e.word.clone())) {
            if variants.len() >= MAX_VARIANTS {
                break;
            }
            if !variants.iter().any(|x| x.to_lowercase() == v.to_lowercase()) {
                variants.push(v);
            }
        }
    }
    Some(Translation { text, variants, engine: Engine::Gtx })
}

pub fn parse_clients5(json: &str) -> Option<Translation> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let first = v.as_array()?.first()?;
    let text = match first {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => a.first()?.as_str()?.to_string(),
        _ => return None,
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then(|| Translation { variants: vec![text.clone()], text, engine: Engine::Clients5 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn gtx_word_gives_variants() {
        let t = parse_gtx(&data("gtx-word-dj.json"), true).unwrap();
        assert_eq!(t.text, "lock");
        assert_eq!(t.variants[..2], ["lock", "castle"]);
        assert!(t.variants.len() <= MAX_VARIANTS);
        assert!(t.variants.contains(&"chateau".to_string()));
    }

    #[test]
    fn gtx_phrase_joins_sentences() {
        let t = parse_gtx(&data("gtx-phrase-dj.json"), false).unwrap();
        assert_eq!(t.text, "Доброе утро, друзья.\nКак вы?");
        assert_eq!(t.variants.len(), 1);
    }

    #[test]
    fn gtx_light_caps_variants() {
        let t = parse_gtx(&data("gtx-light-dj.json"), true).unwrap();
        assert_eq!(t.text, "свет");
        assert_eq!(t.variants.len(), MAX_VARIANTS);
    }

    #[test]
    fn clients5_plain() {
        let t = parse_clients5(&data("clients5.json")).unwrap();
        assert_eq!(t.text, "open the lock");
        assert!(parse_clients5("[]").is_none());
        assert_eq!(parse_clients5(r#"[["замок","en"]]"#).unwrap().text, "замок");
    }

    #[test]
    fn garbage_is_none() {
        assert!(parse_gtx("<html>", true).is_none());
        assert!(parse_gtx(r#"{"sentences":[]}"#, true).is_none());
    }
}
