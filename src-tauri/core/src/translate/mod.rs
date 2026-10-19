pub mod google;
pub mod models;
pub mod offline;

use crate::text::{chunks, word_count, Dir};

pub const CHUNK: usize = 1500;
pub const MAX_TEXT: usize = 20_000;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Gtx,
    Clients5,
    Offline,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Translation {
    pub text: String,
    pub variants: Vec<String>,
    pub engine: Engine,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Failure {
    NoNetwork,
    NoModel,
    Service,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Failure::NoNetwork => "Нет сети — скачай офлайн-модель в настройках",
            Failure::NoModel => "Офлайн-модель не скачана — скачай её в настройках",
            Failure::Service => "Переводчик не ответил — попробуй ещё раз",
        }
    }
}

const CACHE: usize = 200;
const GAP: Duration = Duration::from_millis(250);
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0 Safari/537.36";

type Key = (Dir, String);

pub struct Translator {
    agent: ureq::Agent,
    cache: Mutex<(HashMap<Key, Translation>, VecDeque<Key>)>,
    last: Mutex<Option<Instant>>,
    pub sidecar: offline::Sidecar,
}

impl Default for Translator {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder().http_status_as_error(false).user_agent(UA).build().into();
        Translator { agent, cache: Mutex::new((HashMap::new(), VecDeque::new())), last: Mutex::new(None), sidecar: offline::Sidecar::default() }
    }
}

enum Got {
    Ok(String),
    Net,
    Bad,
}

impl Translator {
    pub fn translate(&self, text: &str, dir: Dir, offline: bool) -> Result<Translation, Failure> {
        let key = (dir, text.to_string());
        if let Some(t) = self.cache.lock().unwrap().0.get(&key) {
            return Ok(t.clone());
        }
        if offline {
            return self.local(key, text, dir);
        }
        let single = word_count(text) == 1;
        let mut net_down = true;
        let gtx = self.get(google::GTX, &[("client", "gtx"), ("sl", dir.from().code()), ("tl", dir.to().code()), ("dt", "t"), ("dt", "at"), ("dt", "bd"), ("dj", "1"), ("q", text)], Duration::from_millis(1200));
        let mut found = match gtx {
            Got::Ok(body) => {
                net_down = false;
                google::parse_gtx(&body, single)
            }
            Got::Bad => {
                net_down = false;
                None
            }
            Got::Net => None,
        };
        if found.is_none() {
            match self.get(google::CLIENTS5, &[("client", "dict-chrome-ex"), ("sl", dir.from().code()), ("tl", dir.to().code()), ("q", text)], Duration::from_millis(800)) {
                Got::Ok(body) => {
                    net_down = false;
                    found = google::parse_clients5(&body);
                }
                Got::Bad => net_down = false,
                Got::Net => {}
            }
        }
        let Some(t) = found else {
            let fail = if net_down { Failure::NoNetwork } else { Failure::Service };
            return if models::ready() { self.local(key, text, dir).map_err(|_| fail) } else { Err(fail) };
        };
        self.remember(key, &t);
        Ok(t)
    }

    fn local(&self, key: Key, text: &str, dir: Dir) -> Result<Translation, Failure> {
        let single = word_count(text) == 1;
        let (main, variants) = self.sidecar.translate(text, dir, if single { 3 } else { 1 })?;
        let t = Translation { text: main, variants: if single { variants } else { Vec::new() }, engine: Engine::Offline };
        self.remember(key, &t);
        Ok(t)
    }

    fn remember(&self, key: Key, t: &Translation) {
        let mut cache = self.cache.lock().unwrap();
        if cache.0.insert(key.clone(), t.clone()).is_none() {
            cache.1.push_back(key);
            if cache.1.len() > CACHE {
                if let Some(old) = cache.1.pop_front() {
                    cache.0.remove(&old);
                }
            }
        }
    }

    pub fn translate_long(&self, text: &str, dir: Dir, offline: bool) -> Result<Translation, Failure> {
        if text.chars().count() <= CHUNK {
            return self.translate(text, dir, offline);
        }
        let mut out = String::new();
        let mut engine = Engine::Gtx;
        for part in chunks(text, CHUNK) {
            let core = part.trim();
            if core.is_empty() {
                out.push_str(part);
                continue;
            }
            let t = self.translate(core, dir, offline)?;
            engine = t.engine;
            out.push_str(&part[..part.len() - part.trim_start().len()]);
            out.push_str(&t.text);
            out.push_str(&part[part.trim_end().len()..]);
        }
        Ok(Translation { variants: vec![out.clone()], text: out, engine })
    }

    pub fn forget(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.0.clear();
        cache.1.clear();
    }

    fn get(&self, url: &str, query: &[(&str, &str)], timeout: Duration) -> Got {
        {
            let mut last = self.last.lock().unwrap();
            if let Some(wait) = last.and_then(|l| GAP.checked_sub(l.elapsed())) {
                std::thread::sleep(wait);
            }
            *last = Some(Instant::now());
        }
        let req = self.agent.get(url).query_pairs(query.iter().copied()).config().timeout_global(Some(timeout)).build();
        match req.call() {
            Ok(mut resp) if resp.status().is_success() => resp.body_mut().read_to_string().map_or(Got::Bad, Got::Ok),
            Ok(_) => Got::Bad,
            Err(_) => Got::Net,
        }
    }
}
