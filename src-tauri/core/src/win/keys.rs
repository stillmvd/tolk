use super::hook::MARK;
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

const MODIFIERS: [VIRTUAL_KEY; 8] = [VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN, VK_LSHIFT, VK_RSHIFT];
const NOOP: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: MARK } },
    }
}

fn tap(out: &mut Vec<INPUT>, vk: VIRTUAL_KEY) {
    out.push(key(vk, 0, KEYBD_EVENT_FLAGS(0)));
    out.push(key(vk, 0, KEYEVENTF_KEYUP));
}

fn send(inputs: &[INPUT]) -> bool {
    inputs.is_empty() || unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) } as usize == inputs.len()
}

fn held_modifiers() -> Vec<VIRTUAL_KEY> {
    MODIFIERS.into_iter().filter(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0).collect()
}

pub fn wait_release(limit: Duration) -> bool {
    let start = Instant::now();
    while !held_modifiers().is_empty() {
        if start.elapsed() > limit {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    true
}

fn clean(body: Vec<INPUT>) -> bool {
    wait_release(Duration::from_millis(1500));
    let held = held_modifiers();
    let menu = held.iter().any(|vk| matches!(*vk, VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN));
    let mut out: Vec<INPUT> = Vec::with_capacity(body.len() + held.len() + 2);
    if menu {
        tap(&mut out, NOOP);
    }
    out.extend(held.iter().map(|&vk| key(vk, 0, KEYEVENTF_KEYUP)));
    out.extend(body);
    send(&out)
}

fn unicode(out: &mut Vec<INPUT>, text: &str) {
    for c in text.chars() {
        if c == '\n' {
            out.push(key(VK_SHIFT, 0, KEYBD_EVENT_FLAGS(0)));
            tap(out, VK_RETURN);
            out.push(key(VK_SHIFT, 0, KEYEVENTF_KEYUP));
            continue;
        }
        if c == '\r' {
            continue;
        }
        let mut buf = [0u16; 2];
        for &unit in c.encode_utf16(&mut buf).iter() {
            out.push(key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
            out.push(key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
    }
}

pub fn replace(backspaces: usize, text: &str) -> bool {
    let mut out = Vec::with_capacity(backspaces * 2 + text.len() * 4);
    for _ in 0..backspaces {
        tap(&mut out, VK_BACK);
    }
    unicode(&mut out, text);
    clean(out)
}

pub fn select_back(n: usize) -> bool {
    let mut out = vec![key(VK_LSHIFT, 0, KEYBD_EVENT_FLAGS(0))];
    for _ in 0..n {
        tap(&mut out, VK_LEFT);
    }
    out.push(key(VK_LSHIFT, 0, KEYEVENTF_KEYUP));
    clean(out)
}

pub fn press(vk: VIRTUAL_KEY) -> bool {
    let mut out = Vec::new();
    tap(&mut out, vk);
    clean(out)
}

pub fn chord(mods: &[VIRTUAL_KEY], vk: VIRTUAL_KEY) -> bool {
    let mut out: Vec<INPUT> = mods.iter().map(|&m| key(m, 0, KEYBD_EVENT_FLAGS(0))).collect();
    tap(&mut out, vk);
    out.extend(mods.iter().rev().map(|&m| key(m, 0, KEYEVENTF_KEYUP)));
    clean(out)
}

pub fn suppress_menu() -> bool {
    let mut out = Vec::new();
    tap(&mut out, NOOP);
    send(&out)
}

pub fn replay(code: u32) -> bool {
    use crate::config::{MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_WIN};
    let mods: Vec<VIRTUAL_KEY> = [(MOD_CTRL, VK_LCONTROL), (MOD_ALT, VK_LMENU), (MOD_SHIFT, VK_LSHIFT), (MOD_WIN, VK_LWIN)]
        .into_iter()
        .filter(|(m, _)| code & m != 0)
        .map(|(_, vk)| vk)
        .collect();
    let mut out: Vec<INPUT> = mods.iter().map(|&m| key(m, 0, KEYBD_EVENT_FLAGS(0))).collect();
    tap(&mut out, VIRTUAL_KEY((code & 0xFFFF) as u16));
    out.extend(mods.iter().rev().map(|&m| key(m, 0, KEYEVENTF_KEYUP)));
    clean(out)
}

pub fn units(text: &str) -> usize {
    text.chars().filter(|&c| c != '\r').count()
}
