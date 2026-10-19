use crate::text::Lang;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, GetKeyboardLayoutList, ToUnicodeEx, HKL, VK_CAPITAL, VK_CONTROL, VK_MENU, VK_SHIFT};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, PostMessageW, GUITHREADINFO, WM_INPUTLANGCHANGEREQUEST};

const NO_STATE_CHANGE: u32 = 4;

fn langid(hkl: HKL) -> u16 {
    (hkl.0 as usize & 0xFFFF) as u16
}

fn is_ime(hkl: HKL) -> bool {
    (hkl.0 as usize >> 28) & 0xF == 0xE
}

pub fn focus_window() -> HWND {
    unsafe {
        let hwnd = GetForegroundWindow();
        let thread = GetWindowThreadProcessId(hwnd, None);
        let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        match GetGUIThreadInfo(thread, &mut info) {
            Ok(()) if !info.hwndFocus.is_invalid() => info.hwndFocus,
            _ => hwnd,
        }
    }
}

fn input_thread() -> (HWND, u32) {
    let focus = focus_window();
    (focus, unsafe { GetWindowThreadProcessId(focus, None) })
}

pub fn current_hkl() -> HKL {
    unsafe { GetKeyboardLayout(input_thread().1) }
}

pub fn current() -> Option<Lang> {
    let hkl = current_hkl();
    (!is_ime(hkl)).then(|| Lang::from_langid(langid(hkl))).flatten()
}

fn hkl_for(lang: Lang) -> Option<HKL> {
    unsafe {
        let n = GetKeyboardLayoutList(None);
        let mut list = vec![HKL::default(); n.max(0) as usize];
        let got = GetKeyboardLayoutList(Some(&mut list));
        list.truncate(got.max(0) as usize);
        list.into_iter().find(|&h| !is_ime(h) && Lang::from_langid(langid(h)) == Some(lang))
    }
}

pub fn switch(lang: Lang) -> bool {
    let Some(hkl) = hkl_for(lang) else { return false };
    let (focus, thread) = input_thread();
    if unsafe { PostMessageW(Some(focus), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize)) }.is_err() {
        return false;
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(80) {
        if unsafe { GetKeyboardLayout(thread) } == hkl {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

pub fn char_of(vk: u16, scan: u16, shift: bool, caps: bool, hkl: HKL) -> Option<char> {
    let mut state = [0u8; 256];
    if shift {
        state[VK_SHIFT.0 as usize] = 0x80;
    }
    if caps {
        state[VK_CAPITAL.0 as usize] = 0x01;
    }
    state[VK_CONTROL.0 as usize] = 0;
    state[VK_MENU.0 as usize] = 0;
    let mut buf = [0u16; 4];
    let n = unsafe { ToUnicodeEx(vk as u32, scan as u32, &state, &mut buf, NO_STATE_CHANGE, Some(hkl)) };
    (n == 1).then(|| char::from_u32(buf[0] as u32)).flatten().filter(|c| !c.is_control())
}
