use ct2rs::sys::{ComputeType, Config, TranslationOptions, Translator};
use ct2rs::tokenizers::sentencepiece::Tokenizer as Spm;
use ct2rs::Tokenizer;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::Path;

const DIRS: [&str; 2] = ["ru-en", "en-ru"];

#[derive(Deserialize)]
struct Request {
    id: u64,
    dir: String,
    text: String,
    #[serde(default = "one")]
    n: usize,
}

fn one() -> usize {
    1
}

struct Model {
    translator: Translator,
    spm: Spm,
}

impl Model {
    fn load(dir: &Path) -> anyhow::Result<Self> {
        let config = Config { compute_type: ComputeType::INT8, ..Default::default() };
        Ok(Model { translator: Translator::new(dir, &config)?, spm: Spm::from_file(dir.join("source.spm"), dir.join("target.spm"))? })
    }

    fn run(&self, sentences: &[String], n: usize) -> anyhow::Result<Vec<Vec<String>>> {
        let tokens = sentences.iter().map(|s| self.spm.encode(s)).collect::<anyhow::Result<Vec<_>>>()?;
        let options = TranslationOptions { beam_size: n.max(4), num_hypotheses: n, max_decoding_length: 512, ..Default::default() };
        self.translator
            .translate_batch(&tokens, &options, None)?
            .into_iter()
            .map(|r| r.hypotheses.into_iter().map(|h| self.spm.decode(h)).collect())
            .collect()
    }

    fn translate(&self, text: &str, n: usize) -> anyhow::Result<(String, Vec<String>)> {
        let parts = split(text);
        let sentences: Vec<String> = parts.iter().flatten().cloned().collect();
        if sentences.is_empty() {
            return Ok((text.to_string(), Vec::new()));
        }
        let single = sentences.len() == 1;
        let mut out = self.run(&sentences, if single { n.max(1) } else { 1 })?.into_iter();
        if single {
            let hyps = out.next().unwrap_or_default();
            let mut variants: Vec<String> = Vec::new();
            for h in hyps {
                if !variants.iter().any(|v| v.to_lowercase() == h.to_lowercase()) {
                    variants.push(h);
                }
            }
            let main = variants.first().cloned().unwrap_or_default();
            return Ok((rejoin(&parts, &mut std::iter::once(main)), variants));
        }
        let mut firsts = out.map(|h| h.into_iter().next().unwrap_or_default());
        Ok((rejoin(&parts, &mut firsts), Vec::new()))
    }
}

fn split(text: &str) -> Vec<Vec<String>> {
    text.split('\n').map(sentences).collect()
}

fn sentences(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        cur.push(c);
        let end = matches!(c, '.' | '!' | '?' | '…');
        let next_space = chars.get(i + 1).is_some_and(|n| n.is_whitespace());
        let next_upper = chars.get(i + 2).is_some_and(|n| n.is_uppercase() || matches!(n, '"' | '«' | '—'));
        if end && next_space && next_upper {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn rejoin(parts: &[Vec<String>], translated: &mut dyn Iterator<Item = String>) -> String {
    parts.iter().map(|line| line.iter().map(|_| translated.next().unwrap_or_default()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n")
}

fn answer(models: &HashMap<&str, Model>, line: &str) -> Value {
    let req: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return json!({ "error": format!("bad request: {e}") }),
    };
    let Some(model) = models.get(req.dir.as_str()) else { return json!({ "id": req.id, "error": format!("model not found: {}", req.dir) }) };
    match model.translate(&req.text, req.n) {
        Ok((text, variants)) => json!({ "id": req.id, "text": text, "variants": variants }),
        Err(e) => json!({ "id": req.id, "error": e.to_string() }),
    }
}

fn main() {
    let root = std::env::args().nth(1).unwrap_or_default();
    let models: HashMap<&str, Model> = DIRS
        .into_iter()
        .filter_map(|d| match Model::load(&Path::new(&root).join(d)) {
            Ok(m) => Some((d, m)),
            Err(e) => {
                eprintln!("tolk-mt: {d}: {e}");
                None
            }
        })
        .collect();
    if models.is_empty() {
        std::process::exit(1);
    }
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", json!({ "ready": true }));
    let _ = out.flush();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let _ = writeln!(out, "{}", answer(&models, &line));
        let _ = out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences_and_lines() {
        assert_eq!(split("Hello. How are you?\nFine"), vec![vec!["Hello.".to_string(), "How are you?".to_string()], vec!["Fine".to_string()]]);
        assert_eq!(sentences("e.g. this one"), vec!["e.g. this one".to_string()]);
        assert_eq!(split("a\n\nb"), vec![vec!["a".to_string()], vec![], vec!["b".to_string()]]);
    }

    #[test]
    fn rejoins_in_order() {
        let parts = split("Раз. Два.\n\nТри");
        let mut it = ["One.", "Two.", "Three"].into_iter().map(String::from);
        assert_eq!(rejoin(&parts, &mut it), "One. Two.\n\nThree");
    }
}
