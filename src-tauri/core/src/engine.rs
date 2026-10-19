use crate::config::{parse_hotkey, Config};
use crate::translate::Translator;
use crate::win::context::{self, Context};
use crate::win::hook::{self, HookEvent};
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum CoreEvent {
    Area,
    Window,
}

pub struct Engine {
    config: Arc<Mutex<Config>>,
    pub translator: Arc<Translator>,
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
        context::run(tx);
        let cfg = config.clone();
        std::thread::Builder::new().name("tolk-engine".into()).spawn(move || work(rx, cfg, events)).expect("engine thread");
        Engine { config, translator }
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    pub fn set_config(&self, cfg: Config) {
        apply(&cfg);
        *self.config.lock().unwrap() = cfg;
    }
}

fn work(rx: Receiver<HookEvent>, config: Arc<Mutex<Config>>, events: Sender<CoreEvent>) {
    for ev in rx {
        match ev {
            HookEvent::Focus => {
                context::focus_taken();
                hook::BLOCKED.store(blocked(&context::current(), &config.lock().unwrap()), SeqCst);
            }
            HookEvent::Area => {
                let _ = events.send(CoreEvent::Area);
            }
            HookEvent::Window => {
                let _ = events.send(CoreEvent::Window);
            }
            _ => {}
        }
    }
}
