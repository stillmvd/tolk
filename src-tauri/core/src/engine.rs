use crate::buffer::WordBuffer;
use crate::config::{parse_hotkey, Config, Mode};
use crate::text::{direction, dress, tail_words, Dir, Lang};
use crate::translate::{Failure, Translation, Translator, MAX_TEXT};
use crate::win::context::{self, Context};
use crate::win::hook::{self, HookEvent, RawKey};
use crate::win::uia::{self, Selection};
use crate::win::{clipboard, keys, lang};
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RECT> for Rect {
    fn from(r: RECT) -> Self {
        Rect { left: r.left, top: r.top, right: r.right, bottom: r.bottom }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CoreEvent {
    Area,
    Window,
    Notice { text: String, error: bool, rect: Option<Rect> },
    Translated { from: String, to: String, dir: Dir },
    Show { original: String, translation: String, rect: Option<Rect> },
    Variants { list: Vec<String>, current: usize, rect: Option<Rect> },
    Waiting { rect: Option<Rect> },
    HideHint,
}

pub struct Engine {
    config: Arc<Mutex<Config>>,
    pub translator: Arc<Translator>,
    tx: Sender<HookEvent>,
}

pub fn apply(cfg: &Config) {
    hook::ENABLED.store(cfg.enabled, SeqCst);
    hook::HK_TRANSLATE.store(parse_hotkey(&cfg.hotkey).unwrap_or(0), SeqCst);
    hook::HK_AREA.store(parse_hotkey(&cfg.area_hotkey).unwrap_or(0), SeqCst);
    hook::HK_WINDOW.store(cfg.window_hotkey.as_deref().and_then(parse_hotkey).unwrap_or(0), SeqCst);
}

fn blocked(ctx: &Context, cfg: &Config) -> bool {
    ctx.own || ctx.password || cfg.is_excluded(&ctx.exe)
}

impl Engine {
    pub fn start(config: Config, events: Sender<CoreEvent>) -> Engine {
        apply(&config);
        let config = Arc::new(Mutex::new(config));
        let translator = Arc::new(Translator::default());
        let (tx, rx) = crossbeam_channel::bounded(4096);
        hook::run(tx.clone());
        context::run(tx.clone());
        let mut worker = Worker { config: config.clone(), translator: translator.clone(), events, buffer: WordBuffer::default(), done: None };
        std::thread::Builder::new().name("tolk-engine".into()).spawn(move || worker.run(rx)).expect("engine thread");
        Engine { config, translator, tx }
    }

    pub fn pick(&self, n: usize) {
        let _ = self.tx.send(HookEvent::Pick(n as u8));
    }

    pub fn translate_now(&self) {
        let _ = self.tx.send(HookEvent::Translate);
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    pub fn set_config(&self, cfg: Config) {
        apply(&cfg);
        *self.config.lock().unwrap() = cfg;
    }
}

struct Typed {
    before: String,
    start: usize,
    words: usize,
}

struct Done {
    hwnd: isize,
    seq: u64,
    at: Instant,
    inserted: String,
    original: String,
    layout: Option<Lang>,
    typed: Option<Typed>,
    variants: Vec<String>,
}

struct Worker {
    config: Arc<Mutex<Config>>,
    translator: Arc<Translator>,
    events: Sender<CoreEvent>,
    buffer: WordBuffer,
    done: Option<Done>,
}

const RESETS: [VIRTUAL_KEY; 14] = [
    VK_RETURN, VK_TAB, VK_ESCAPE, VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_HOME, VK_END, VK_PRIOR, VK_NEXT, VK_DELETE, VK_INSERT, VK_APPS,
];
const TYPE_LIMIT: usize = 400;

const UNDO_FOR: Duration = Duration::from_secs(60);

fn put(backspaces: usize, text: &str, console: bool) -> bool {
    if text.contains('\n') || keys::units(text) > TYPE_LIMIT {
        (backspaces == 0 || keys::select_back(backspaces)) && clipboard::paste(text, console)
    } else {
        keys::replace(backspaces, text)
    }
}

impl Worker {
    fn run(&mut self, rx: Receiver<HookEvent>) {
        for ev in rx {
            match ev {
                HookEvent::Focus => {
                    context::focus_taken();
                    let ctx = context::current();
                    hook::BLOCKED.store(blocked(&ctx, &self.config.lock().unwrap()), SeqCst);
                    if ctx.hwnd != self.buffer.owner() {
                        self.buffer.reset();
                    }
                }
                HookEvent::Key(k) => {
                    self.hide_hint();
                    self.key(&k)
                }
                HookEvent::Click | HookEvent::Foreign => {
                    self.hide_hint();
                    self.buffer.reset()
                }
                HookEvent::Translate => self.translate(),
                HookEvent::Undo => self.undo(),
                HookEvent::Area => self.emit(CoreEvent::Area),
                HookEvent::Window => self.emit(CoreEvent::Window),
                HookEvent::Pick(n) => self.pick(n as usize),
            }
        }
    }

    fn emit(&self, ev: CoreEvent) {
        let _ = self.events.send(ev);
    }

    fn notice(&self, text: &str) {
        self.emit(CoreEvent::Notice { text: text.into(), error: false, rect: uia::caret_rect().map(Rect::from) });
    }

    fn error(&self, text: &str) {
        self.emit(CoreEvent::Notice { text: text.into(), error: true, rect: uia::caret_rect().map(Rect::from) });
    }

    fn offline(&self) -> bool {
        self.config.lock().unwrap().mode == Mode::Offline
    }

    fn key(&mut self, k: &RawKey) {
        if hook::BLOCKED.load(SeqCst) {
            return self.buffer.reset();
        }
        let owner = context::current().hwnd;
        if let Some(u) = k.unicode {
            if let Some(c) = char::from_u32(u as u32) {
                self.buffer.push(owner, c);
            }
            return;
        }
        let vk = VIRTUAL_KEY(k.vk);
        if vk == VK_BACK {
            if k.ctrl {
                self.buffer.back_word();
            } else {
                self.buffer.back();
            }
            return;
        }
        if k.ctrl || k.alt || k.win || RESETS.contains(&vk) {
            self.buffer.reset();
            return;
        }
        if let Some(c) = lang::char_of(k.vk, k.scan, k.shift, k.caps, lang::current_hkl()) {
            self.buffer.push(owner, c);
        }
    }

    fn translate(&mut self) {
        self.hide_hint();
        keys::suppress_menu();
        let ctx = context::current();
        let seq = hook::SEQ.load(SeqCst);
        let repeat = Duration::from_millis(self.config.lock().unwrap().repeat_ms as u64);
        if let Some(d) = self.done.take() {
            if d.hwnd == ctx.hwnd && d.seq == seq && d.at.elapsed() < repeat {
                return self.extend(&ctx, d, seq);
            }
        }
        if ctx.elevated {
            return self.error("Не могу менять текст в окне администратора");
        }
        let typed = self.buffer.has_word(ctx.hwnd);
        match uia::selection() {
            Selection::Text { text, editable, rect } => {
                self.replace_selection(&ctx, text, editable, rect.map(Rect::from), seq, false);
                return;
            }
            _ if typed => {}
            Selection::Empty { editable: true } => {
                if let Some(word) = uia::select_word_before_caret() {
                    self.replace_selection(&ctx, word, true, None, seq, true);
                    return;
                }
            }
            Selection::Empty { editable: false } => {}
            Selection::Unknown => {
                if let Some(text) = clipboard::copy_selection(ctx.console).filter(|t| !t.ends_with('\n')) {
                    self.replace_selection(&ctx, text, uia::has_caret(), None, seq, false);
                    return;
                }
            }
        }
        if typed {
            let before = self.buffer.text().to_string();
            if let Some(start) = tail_words(&before, 1) {
                self.replace_typed(&ctx, before, start, 1, None, seq);
                return;
            }
        }
        keys::replay(hook::HK_TRANSLATE.load(SeqCst));
    }

    fn keep(&mut self, d: Done) {
        self.done = Some(Done { at: Instant::now(), ..d });
        hook::ARMED_UNDO.store(true, SeqCst);
    }

    fn extend(&mut self, ctx: &Context, d: Done, seq: u64) {
        let next = d.typed.as_ref().and_then(|t| tail_words(&t.before, t.words + 1).filter(|&s| s < t.start).map(|s| (t.before.clone(), s, t.words + 1)));
        let Some((before, start, words)) = next else {
            self.notice(if d.typed.is_some() { "Это вся фраза" } else { "Уже переведено — Ctrl+Z вернёт как было" });
            return self.keep(d);
        };
        if !self.replace_typed(ctx, before, start, words, Some(&d), seq) && self.done.is_none() {
            self.keep(d);
        }
    }

    fn fetch(&self, text: &str, dir: Dir) -> Option<Translation> {
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let (done, shown, events) = (done.clone(), shown.clone(), self.events.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150));
                if !done.load(SeqCst) {
                    let rect = uia::caret_rect().map(Rect::from);
                    if !done.load(SeqCst) && !shown.swap(true, SeqCst) {
                        let _ = events.send(CoreEvent::Waiting { rect });
                    }
                }
            });
        }
        let got = self.translator.translate_long(text, dir, self.offline());
        done.store(true, SeqCst);
        if shown.swap(true, SeqCst) {
            self.emit(CoreEvent::HideHint);
        }
        match got {
            Ok(t) => Some(t),
            Err(f) => {
                self.fail(f);
                None
            }
        }
    }

    fn still_here(&self, hwnd: isize, seq: u64) -> bool {
        keys::wait_release(Duration::from_millis(1500));
        hook::SEQ.load(SeqCst) == seq && context::current().hwnd == hwnd
    }

    fn replace_typed(&mut self, ctx: &Context, before: String, start: usize, words: usize, prev: Option<&Done>, seq: u64) -> bool {
        let fragment = &before[start..];
        let Some(dir) = direction(fragment) else {
            self.notice("Нечего переводить");
            return false;
        };
        let Some(t) = self.fetch(fragment.trim(), dir) else { return false };
        let dressed = dress(fragment, &t.text);
        if !self.still_here(ctx.hwnd, seq) {
            self.emit(CoreEvent::Show { original: fragment.to_string(), translation: dressed, rect: None });
            return false;
        }
        let backspaces = match prev.and_then(|p| p.typed.as_ref().map(|t| (p, t))) {
            Some((p, t)) => keys::units(&p.inserted) + keys::units(&before[start..t.start]),
            None => keys::units(fragment),
        };
        let layout = prev.map_or_else(lang::current, |p| p.layout);
        hook::ARMED_UNDO.store(true, SeqCst);
        if !put(backspaces, &dressed, ctx.console) {
            hook::ARMED_UNDO.store(false, SeqCst);
            self.error("Не получилось заменить текст");
            return false;
        }
        self.buffer.set(ctx.hwnd, format!("{}{}", &before[..start], dressed));
        self.emit(CoreEvent::Translated { from: fragment.to_string(), to: dressed.clone(), dir });
        let original = fragment.to_string();
        let variants: Vec<String> = if words == 1 { t.variants.iter().map(|v| dress(fragment, v)).collect() } else { Vec::new() };
        self.done = Some(Done { hwnd: ctx.hwnd, seq, at: Instant::now(), inserted: dressed, original, layout, typed: Some(Typed { before, start, words }), variants });
        if lang::current() != Some(dir.to()) {
            lang::switch(dir.to());
        }
        self.show_variants(0);
        true
    }

    fn replace_selection(&mut self, ctx: &Context, text: String, editable: bool, rect: Option<Rect>, seq: u64, ours: bool) {
        let collapse = |ok: bool| {
            if ours && !ok && hook::SEQ.load(SeqCst) == seq {
                keys::press(VK_RIGHT);
            }
        };
        if text.chars().count() > MAX_TEXT {
            self.error("Слишком длинный текст — выдели меньше 20 000 символов");
            return collapse(false);
        }
        let Some(dir) = direction(&text) else {
            self.notice("Нечего переводить");
            return collapse(false);
        };
        let Some(t) = self.fetch(text.trim(), dir) else { return collapse(false) };
        let dressed = dress(&text, &t.text);
        if !editable || !self.still_here(ctx.hwnd, seq) {
            collapse(false);
            return self.emit(CoreEvent::Show { original: text, translation: dressed, rect });
        }
        hook::ARMED_UNDO.store(true, SeqCst);
        if !put(0, &dressed, ctx.console) {
            hook::ARMED_UNDO.store(false, SeqCst);
            collapse(false);
            return self.emit(CoreEvent::Show { original: text, translation: dressed, rect });
        }
        self.buffer.reset();
        self.emit(CoreEvent::Translated { from: text.clone(), to: dressed.clone(), dir });
        let variants: Vec<String> = if crate::text::word_count(&text) == 1 { t.variants.iter().map(|v| dress(&text, v)).collect() } else { Vec::new() };
        self.done = Some(Done { hwnd: ctx.hwnd, seq, at: Instant::now(), inserted: dressed, original: text, layout: None, typed: None, variants });
        self.show_variants(0);
    }

    fn undo(&mut self) {
        hook::ARMED_UNDO.store(false, SeqCst);
        let fresh = self.done.as_ref().is_some_and(|d| d.at.elapsed() < UNDO_FOR && d.seq == hook::SEQ.load(SeqCst));
        let Some(d) = self.done.take().filter(|_| fresh) else {
            keys::replay(crate::config::MOD_CTRL | VK_Z.0 as u32);
            return;
        };
        if !self.still_here(d.hwnd, d.seq) {
            return;
        }
        let ctx = context::current();
        if !put(keys::units(&d.inserted), &d.original, ctx.console) {
            self.error("Не получилось вернуть текст");
            return;
        }
        if let Some(l) = d.layout {
            if lang::current() != Some(l) {
                lang::switch(l);
            }
        }
        match d.typed {
            Some(t) => self.buffer.set(d.hwnd, t.before),
            None => self.buffer.reset(),
        }
    }

    fn show_variants(&self, current: usize) {
        let Some(d) = &self.done else { return };
        if d.variants.len() < 2 {
            return;
        }
        hook::HINT_OPEN.store(true, SeqCst);
        self.emit(CoreEvent::Variants { list: d.variants.clone(), current, rect: uia::caret_rect().map(Rect::from) });
    }

    fn hide_hint(&self) {
        if hook::HINT_OPEN.swap(false, SeqCst) {
            self.emit(CoreEvent::HideHint);
        }
    }

    fn pick(&mut self, n: usize) {
        keys::suppress_menu();
        let seq = hook::SEQ.load(SeqCst);
        let Some(d) = self.done.as_mut().filter(|d| d.seq == seq) else {
            return self.hide_hint();
        };
        hook::ARMED_UNDO.store(true, SeqCst);
        if n < 1 || n > d.variants.len() {
            return self.hide_hint();
        }
        let next = d.variants[n - 1].clone();
        if next == d.inserted {
            return;
        }
        let ctx = context::current();
        if ctx.hwnd != d.hwnd || !put(keys::units(&d.inserted), &next, ctx.console) {
            hook::ARMED_UNDO.store(false, SeqCst);
            return self.hide_hint();
        }
        if let Some(t) = &d.typed {
            self.buffer.set(d.hwnd, format!("{}{}", &t.before[..t.start], next));
        }
        d.inserted = next;
        d.at = Instant::now();
        hook::ARMED_UNDO.store(true, SeqCst);
        self.show_variants(n - 1);
    }

    fn fail(&self, f: Failure) {
        self.error(f.message());
    }
}
