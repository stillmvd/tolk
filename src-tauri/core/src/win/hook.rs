use crate::config::{MOD_ALT, MOD_CTRL, MOD_SHIFT, MOD_WIN};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::SeqCst};
use std::sync::OnceLock;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub const MARK: usize = 0x544F_4C4B;
pub const GHBDTN_MARK: usize = 0x4748_4244;
pub const TEST_MARK: usize = 0x544F_5453;
const WM_REINSTALL: u32 = WM_APP + 1;
const WM_QUIT_HOOK: u32 = WM_APP + 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawKey {
    pub vk: u16,
    pub scan: u16,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub win: bool,
    pub caps: bool,
    pub unicode: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookEvent {
    Key(RawKey),
    Translate,
    Area,
    Window,
    Undo,
    Pick(u8),
    Click,
    Foreign,
    Focus,
}

pub static ENABLED: AtomicBool = AtomicBool::new(true);
pub static BLOCKED: AtomicBool = AtomicBool::new(false);
pub static ARMED_UNDO: AtomicBool = AtomicBool::new(false);
pub static HINT_OPEN: AtomicBool = AtomicBool::new(false);
pub static HK_TRANSLATE: AtomicU32 = AtomicU32::new(0);
pub static HK_AREA: AtomicU32 = AtomicU32::new(0);
pub static HK_WINDOW: AtomicU32 = AtomicU32::new(0);
pub static SEQ: AtomicU64 = AtomicU64::new(0);
static ACCEPT_TEST: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);
static EATEN: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static SENDER: OnceLock<Sender<HookEvent>> = OnceLock::new();

fn eaten_slot(vk: u16) -> (&'static AtomicU64, u64) {
    (&EATEN[(vk as usize >> 6) & 3], 1 << (vk & 63))
}

fn set_eaten(vk: u16) {
    let (slot, bit) = eaten_slot(vk);
    slot.fetch_or(bit, SeqCst);
}

fn take_eaten(vk: u16) -> bool {
    let (slot, bit) = eaten_slot(vk);
    slot.fetch_and(!bit, SeqCst) & bit != 0
}

fn down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

fn push(ev: HookEvent) {
    if let Some(tx) = SENDER.get() {
        let _ = tx.try_send(ev);
    }
}

fn touch() {
    SEQ.fetch_add(1, SeqCst);
    ARMED_UNDO.store(false, SeqCst);
}

pub fn is_modifier(vk: u16) -> bool {
    matches!(
        VIRTUAL_KEY(vk),
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU
            | VK_LWIN | VK_RWIN | VK_CAPITAL
    )
}

fn hotkey_matches(code: u32, k: &RawKey) -> bool {
    code != 0
        && (code & 0xFFFF) as u16 == k.vk
        && (code & MOD_SHIFT != 0) == k.shift
        && (code & MOD_CTRL != 0) == k.ctrl
        && (code & MOD_ALT != 0) == k.alt
        && (code & MOD_WIN != 0) == k.win
}

pub fn decide(k: &RawKey, enabled: bool, blocked: bool, armed: bool, hint: bool) -> Option<HookEvent> {
    let plain = !k.shift && !k.win;
    if armed && k.vk == VK_Z.0 && k.ctrl && !k.alt && plain {
        return Some(HookEvent::Undo);
    }
    if hint && k.alt && !k.ctrl && plain && (b'1' as u16..=b'5' as u16).contains(&k.vk) {
        return Some(HookEvent::Pick((k.vk - b'0' as u16) as u8));
    }
    if !enabled {
        return None;
    }
    if hotkey_matches(HK_AREA.load(SeqCst), k) {
        return Some(HookEvent::Area);
    }
    if hotkey_matches(HK_WINDOW.load(SeqCst), k) {
        return Some(HookEvent::Window);
    }
    if !blocked && hotkey_matches(HK_TRANSLATE.load(SeqCst), k) {
        return Some(HookEvent::Translate);
    }
    None
}

fn on_key(kb: &KBDLLHOOKSTRUCT) -> bool {
    let up = kb.flags.contains(LLKHF_UP);
    let vk = kb.vkCode as u16;
    if kb.flags.contains(LLKHF_INJECTED) && kb.dwExtraInfo == GHBDTN_MARK {
        if !up && (vk == VK_PACKET.0 || vk == VK_BACK.0) {
            SEQ.fetch_add(1, SeqCst);
            let unicode = (vk == VK_PACKET.0).then_some(kb.scanCode as u16);
            push(HookEvent::Key(RawKey { vk, scan: 0, shift: false, ctrl: false, alt: false, win: false, caps: false, unicode }));
        }
        return false;
    }
    let test = kb.dwExtraInfo == TEST_MARK && ACCEPT_TEST.load(SeqCst);
    if kb.flags.contains(LLKHF_INJECTED) && !test {
        if !up && !is_modifier(vk) {
            touch();
            push(HookEvent::Foreign);
        }
        return false;
    }
    if up {
        return take_eaten(vk);
    }
    let (slot, bit) = eaten_slot(vk);
    if slot.load(SeqCst) & bit != 0 {
        return true;
    }
    let k = RawKey {
        vk,
        scan: kb.scanCode as u16,
        shift: down(VK_SHIFT),
        ctrl: down(VK_CONTROL),
        alt: down(VK_MENU),
        win: down(VK_LWIN) || down(VK_RWIN),
        caps: unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0,
        unicode: (vk == VK_PACKET.0).then_some(kb.scanCode as u16),
    };
    if let Some(ev) = decide(&k, ENABLED.load(SeqCst), BLOCKED.load(SeqCst), ARMED_UNDO.load(SeqCst), HINT_OPEN.load(SeqCst)) {
        if ev != HookEvent::Undo {
            ARMED_UNDO.store(false, SeqCst);
        }
        set_eaten(vk);
        push(ev);
        return true;
    }
    if !is_modifier(vk) {
        touch();
        push(HookEvent::Key(k));
    }
    false
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if kb.dwExtraInfo != MARK && std::panic::catch_unwind(|| on_key(kb)).unwrap_or(false) {
            return LRESULT(1);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && matches!(wparam.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) {
        touch();
        push(HookEvent::Click);
    }
    CallNextHookEx(None, code, wparam, lparam)
}

type Hooks = (Option<HHOOK>, Option<HHOOK>);

fn install() -> Hooks {
    unsafe {
        (
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0).ok(),
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0).ok(),
        )
    }
}

fn uninstall(hooks: Hooks) {
    unsafe {
        for h in [hooks.0, hooks.1].into_iter().flatten() {
            let _ = UnhookWindowsHookEx(h);
        }
    }
}

pub fn run(tx: Sender<HookEvent>) -> std::thread::JoinHandle<()> {
    let _ = SENDER.set(tx);
    ACCEPT_TEST.store(std::env::var_os("TOLK_ACCEPT_TEST").is_some_and(|v| v == "1"), SeqCst);
    std::thread::Builder::new()
        .name("tolk-hook".into())
        .spawn(|| unsafe {
            HOOK_THREAD.store(GetCurrentThreadId(), SeqCst);
            let mut hooks = install();
            let watch = watch_window();
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                match msg.message {
                    WM_REINSTALL => {
                        uninstall(hooks);
                        hooks = install();
                    }
                    WM_QUIT_HOOK => break,
                    _ => {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
            if let Some(hwnd) = watch {
                let _ = WTSUnRegisterSessionNotification(hwnd);
                let _ = DestroyWindow(hwnd);
            }
            uninstall(hooks);
        })
        .expect("hook thread")
}

fn resumed(msg: u32, wparam: WPARAM) -> bool {
    let code = wparam.0 as u32;
    match msg {
        WM_WTSSESSION_CHANGE => matches!(code, WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT | WTS_SESSION_LOGON | WTS_SESSION_UNLOCK),
        WM_POWERBROADCAST => matches!(code, PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND),
        _ => false,
    }
}

unsafe extern "system" fn watch_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if resumed(msg, wparam) {
        touch();
        reinstall();
        push(HookEvent::Focus);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn watch_window() -> Option<HWND> {
    let instance = GetModuleHandleW(None).ok()?.into();
    let class = WNDCLASSW { lpfnWndProc: Some(watch_proc), hInstance: instance, lpszClassName: w!("TolkSessionWatch"), ..Default::default() };
    RegisterClassW(&class);
    let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), w!("TolkSessionWatch"), w!(""), WS_OVERLAPPED, 0, 0, 0, 0, None, None, Some(instance), None).ok()?;
    let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
    Some(hwnd)
}

fn post(msg: u32) {
    let id = HOOK_THREAD.load(SeqCst);
    if id != 0 {
        unsafe {
            let _ = PostThreadMessageW(id, msg, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn reinstall() {
    post(WM_REINSTALL);
}

pub fn stop() {
    post(WM_QUIT_HOOK);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(vk: u8, ctrl: bool, alt: bool, shift: bool) -> RawKey {
        RawKey { vk: vk as u16, scan: 0, shift, ctrl, alt, win: false, caps: false, unicode: None }
    }

    #[test]
    fn decides_hotkeys_by_flags() {
        HK_TRANSLATE.store(MOD_ALT | b'Q' as u32, SeqCst);
        HK_AREA.store(MOD_ALT | MOD_SHIFT | b'Q' as u32, SeqCst);
        let alt_q = key(b'Q', false, true, false);
        assert_eq!(decide(&alt_q, true, false, false, false), Some(HookEvent::Translate));
        assert_eq!(decide(&alt_q, true, true, false, false), None);
        assert_eq!(decide(&alt_q, false, false, false, false), None);
        assert_eq!(decide(&key(b'Q', false, true, true), true, false, false, false), Some(HookEvent::Area));
        assert_eq!(decide(&key(b'Z', true, false, false), true, false, true, false), Some(HookEvent::Undo));
        assert_eq!(decide(&key(b'Z', true, false, false), true, false, false, false), None);
        assert_eq!(decide(&key(b'2', false, true, false), true, false, false, true), Some(HookEvent::Pick(2)));
        assert_eq!(decide(&key(b'2', false, false, false), true, false, false, true), None);
        assert_eq!(decide(&key(b'6', false, true, false), true, false, false, true), None);
    }
}
